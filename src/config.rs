//! Per-game configuration: entry points and hints for what static analysis can't infer.
//!
//! ```toml
//! [[entry]]                # code roots (interrupt handlers, reset, ...)
//! addr = "00:8000"
//! name = "reset"
//! m = 8                    # optional register widths, default 8
//! x = 8
//!
//! [[table]]                # jump table for an indirect jump/call
//! kind = "indirect"        # site is JMP (a,X) / JSR (a,X) / JMP (a) / JML [a]
//! site = "00:8100"
//! at = "00:8110"           # where the pointers live
//! count = 8
//! size = 2                 # 2 = 16-bit pointers (site's bank), 3 = 24-bit
//! bias = 0                 # added to each pointer (1 for RTS-style "addr - 1" tables)
//!
//! [[table]]                # pointer split over parallel byte arrays (LDA lo,Y / hi,Y / bank,Y)
//! kind = "split"
//! site = "00:80C6"
//! lo = "00:8061"
//! hi = "00:807D"
//! bank = "00:8099"         # optional, defaults to the site's bank
//! count = 28
//!
//! [[table]]                # pointers stored right after a call to a dispatch helper
//! kind = "inline"
//! site = "00:8200"         # the JSR/JSL to the helper
//! count = 5
//! falls_through = false    # default: the helper never returns to the call site
//!
//! [[inline_helper]]        # dispatch helper: every call is followed by an inline pointer table
//! addr = "00:8781"         # of unknown length, delimited by the discovery (see discover.rs)
//! size = 2                 # 2 = 16-bit pointers (caller's bank), 3 = 24-bit; the helper never returns
//! exit = "00:8799"         # optional: the helper's own indirect jump; its targets are the union of those of every call
//!
//! [[not_code]]             # padding/data that a table points at: discovery does not decode it
//! addr = "0E:FB64"
//!
//! [[no_return]]            # routines that never return to their caller
//! addr = "00:8300"
//!
//! [[assume]]               # force register widths at an address (after PLP/RTI etc.)
//! addr = "00:8400"
//! m = 16
//! x = 16
//! ```

use crate::disasm::Flags;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

pub fn parse_addr(s: &str) -> Result<u32, String> {
    let clean: String = s.trim().trim_start_matches('$').chars().filter(|&c| c != ':').collect();
    match u32::from_str_radix(&clean, 16) {
        Ok(a) if a <= 0xFF_FFFF => Ok(a),
        _ => Err(format!("bad address: {s:?}")),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default)]
    entry: Vec<RawEntry>,
    #[serde(default)]
    table: Vec<RawTable>,
    #[serde(default)]
    inline_helper: Vec<RawHelper>,
    #[serde(default)]
    not_code: Vec<RawAddr>,
    #[serde(default)]
    no_return: Vec<RawAddr>,
    #[serde(default)]
    assume: Vec<RawAssume>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    addr: String,
    name: Option<String>,
    m: Option<u8>,
    x: Option<u8>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTable {
    kind: TableKind,
    site: String,
    at: Option<String>,
    lo: Option<String>,
    hi: Option<String>,
    bank: Option<String>,
    count: usize,
    size: Option<u8>,
    bias: Option<i32>,
    falls_through: Option<bool>,
    name: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHelper {
    addr: String,
    size: Option<u8>,
    exit: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAddr {
    addr: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAssume {
    addr: String,
    m: Option<u8>,
    x: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TableKind {
    Indirect,
    Inline,
    Split,
}

/// Locations of the byte arrays of a `split` table.
#[derive(Debug, Clone, Copy)]
pub struct Split {
    pub lo: u32,
    pub hi: u32,
    pub bank: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub addr: u32,
    pub name: String,
    pub flags: Flags,
}

#[derive(Debug, Clone)]
pub struct Table {
    pub kind: TableKind,
    /// Explicit location of the pointers. For inline tables this is derived from the site.
    pub at: Option<u32>,
    pub split: Option<Split>,
    pub count: usize,
    pub size: u8,
    pub bias: i32,
    pub falls_through: bool,
    pub name: String,
}

/// A routine that takes its jump table inline after the call and never returns to the caller.
#[derive(Debug, Clone)]
pub struct InlineHelper {
    pub size: u8,
    pub name: String,
    /// The helper's own indirect jump, if declared.
    pub exit: Option<u32>,
}

#[derive(Debug, Default)]
pub struct Config {
    pub entries: Vec<Entry>,
    pub tables: HashMap<u32, Table>,
    pub inline_helpers: HashMap<u32, InlineHelper>,
    /// Indirect jumps inside inline helpers: their targets are given by each call, so they are not unresolved.
    pub helper_exits: HashSet<u32>,
    pub no_return: HashSet<u32>,
    pub not_code: HashSet<u32>,
    pub assume: HashMap<u32, Flags>,
}

fn width(v: Option<u8>, what: &str) -> Result<bool, String> {
    match v {
        None | Some(8) => Ok(true),
        Some(16) => Ok(false),
        Some(n) => Err(format!("{what}: width must be 8 or 16, got {n}")),
    }
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Config, String> {
        let path = path.as_ref();
        let text = fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        Config::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn parse(text: &str) -> Result<Config, String> {
        let raw: RawConfig = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut cfg = Config::default();

        for e in raw.entry {
            let addr = parse_addr(&e.addr)?;
            let flags = Flags { m: width(e.m, &e.addr)?, x: width(e.x, &e.addr)? };
            cfg.entries.push(Entry { addr, name: e.name.unwrap_or_default(), flags });
        }

        for t in raw.table {
            let site = parse_addr(&t.site)?;
            let size = t.size.unwrap_or(2);
            if size != 2 && size != 3 {
                return Err(format!("table at {}: size must be 2 or 3, got {size}", t.site));
            }
            let at = t.at.as_deref().map(parse_addr).transpose()?;
            let split = match (t.kind, &t.lo, &t.hi) {
                (TableKind::Split, Some(lo), Some(hi)) => Some(Split {
                    lo: parse_addr(lo)?,
                    hi: parse_addr(hi)?,
                    bank: t.bank.as_deref().map(parse_addr).transpose()?,
                }),
                (TableKind::Split, _, _) => return Err(format!("table at {}: split tables need `lo` and `hi`", t.site)),
                _ => None,
            };
            match (t.kind, at) {
                (TableKind::Indirect, None) => return Err(format!("table at {}: indirect tables need `at`", t.site)),
                (TableKind::Inline | TableKind::Split, Some(_)) => {
                    return Err(format!("table at {}: only indirect tables can set `at`", t.site))
                }
                _ => {}
            }
            let table = Table {
                kind: t.kind,
                at,
                split,
                count: t.count,
                size,
                bias: t.bias.unwrap_or(0),
                falls_through: t.falls_through.unwrap_or(t.kind != TableKind::Inline),
                name: t.name.unwrap_or_default(),
            };
            if cfg.tables.insert(site, table).is_some() {
                return Err(format!("duplicate table for site {}", t.site));
            }
        }

        for h in raw.inline_helper {
            let addr = parse_addr(&h.addr)?;
            let size = h.size.unwrap_or(2);
            if size != 2 && size != 3 {
                return Err(format!("inline_helper at {}: size must be 2 or 3, got {size}", h.addr));
            }
            let exit = h.exit.as_deref().map(parse_addr).transpose()?;
            if let Some(exit) = exit {
                cfg.helper_exits.insert(exit);
            }
            let helper = InlineHelper { size, name: h.name.unwrap_or_default(), exit };
            if cfg.inline_helpers.insert(addr, helper).is_some() {
                return Err(format!("duplicate inline_helper {}", h.addr));
            }
        }

        for a in raw.not_code {
            cfg.not_code.insert(parse_addr(&a.addr)?);
        }

        for a in raw.no_return {
            cfg.no_return.insert(parse_addr(&a.addr)?);
        }

        for a in raw.assume {
            let addr = parse_addr(&a.addr)?;
            cfg.assume.insert(addr, Flags { m: width(a.m, &a.addr)?, x: width(a.x, &a.addr)? });
        }

        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_sections() {
        let cfg = Config::parse(
            r#"
            [[entry]]
            addr = "00:8000"
            name = "reset"
            [[entry]]
            addr = "00:822C"
            m = 16

            [[table]]
            kind = "indirect"
            site = "00:8100"
            at = "00:8110"
            count = 4
            [[table]]
            kind = "inline"
            site = "00:8200"
            count = 3
            bias = 1

            [[no_return]]
            addr = "00:8300"
            [[assume]]
            addr = "00:8400"
            m = 16
            x = 16
            "#,
        )
        .unwrap();
        assert_eq!(cfg.entries.len(), 2);
        assert_eq!(cfg.entries[0].flags, Flags::RESET);
        assert_eq!(cfg.entries[1].flags, Flags { m: false, x: true });
        assert!(cfg.tables[&0x8100].falls_through);
        assert!(!cfg.tables[&0x8200].falls_through);
        assert_eq!(cfg.tables[&0x8200].bias, 1);
        assert!(cfg.no_return.contains(&0x8300));
        assert_eq!(cfg.assume[&0x8400], Flags { m: false, x: false });
    }

    #[test]
    fn parses_inline_helpers() {
        let cfg = Config::parse(
            "[[inline_helper]]
addr=\"00:8781\"
exit=\"00:8799\"
[[inline_helper]]
addr=\"00:879C\"
size=3",
        )
        .unwrap();
        assert_eq!(cfg.inline_helpers[&0x8781].size, 2);
        assert_eq!(cfg.inline_helpers[&0x879C].size, 3);
        assert!(cfg.helper_exits.contains(&0x8799));
        assert_eq!(cfg.inline_helpers[&0x8781].exit, Some(0x8799));
        assert_eq!(cfg.inline_helpers[&0x879C].exit, None);
        assert!(Config::parse("[[inline_helper]]
addr=\"00:8781\"
size=4").is_err());
        assert!(Config::parse("[[inline_helper]]
addr=\"00:8781\"
[[inline_helper]]
addr=\"00:8781\"").is_err());
    }

    #[test]
    fn bundled_alttp_config_parses() {
        let cfg = Config::load(concat!(env!("CARGO_MANIFEST_DIR"), "/games/alttp_usa.toml")).unwrap();
        assert_eq!(cfg.inline_helpers.len(), 2);
        assert!(cfg.tables.contains_key(&0x00_80C6));
        assert!(cfg.tables.len() > 60);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(Config::parse("[[table]]\nkind=\"indirect\"\nsite=\"00:8000\"\ncount=1").is_err());
        assert!(Config::parse("[[entry]]\naddr=\"zz\"").is_err());
        assert!(Config::parse("[[entry]]\naddr=\"00:8000\"\nm=12").is_err());
        assert!(Config::parse("[[entry]]\naddr=\"00:8000\"\ntypo=1").is_err());
    }
}
