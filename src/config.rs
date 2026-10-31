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
//!
//! [game]                   # identity of the profile; optional for `discover`/`codegen --config`, required by snes-port
//! id = "alttp_usa"
//! name = "The Legend of Zelda: A Link to the Past (USA)"
//! crc32 = "777AAC2F"       # of the ROM without copier header
//!
//! [[hook]]                 # a C function of the game (games/<id>/*.c) called from a recompiled function
//! addr = "06:9271"         # entry of a function of the graph
//! kind = "before"          # before = on entry, after = before each return, replace = instead of the body
//! call = "my_hook"         # void my_hook(Cpu *c)
//! ret = "rts"              # only with replace: how the function returns (rts or rtl)
//!
//! [resume]                 # where the runtime can save and load states: a block of the main loop, with the 65816 stack at its base
//! addr = "00:8034"         # start of a block of the graph; the generator calls rt_resume(c) there
//!
//! [[option]]               # a setting of the game for the player: config.ini `game.<key> = <name>` and a row in the F1 menu
//! key = "language"         # letters, digits or `_`, up to 31 characters; the C of the game reads it with game_opt("language", 0)
//! label = "Language"       # the row of the menu
//! names = ["en", "es"]     # two or more different names; game_opt returns the index of the chosen one
//! default = "en"           # one of the names
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
    game: Option<RawGame>,
    #[serde(default)]
    hook: Vec<RawHook>,
    #[serde(default)]
    option: Vec<RawOption>,
    resume: Option<RawResume>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawResume {
    addr: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOption {
    key: String,
    label: String,
    names: Vec<String>,
    default: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGame {
    id: String,
    name: String,
    crc32: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHook {
    addr: String,
    kind: HookKind,
    call: String,
    ret: Option<HookReturn>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookKind {
    Before,
    After,
    Replace,
}

/// How a function whose body a hook replaces returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookReturn {
    Rts,
    Rtl,
}

/// A call to a C function of the game from the recompiled function that starts at `addr`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hook {
    pub addr: u32,
    pub kind: HookKind,
    pub call: String,
    /// Set for `Replace`, and only then.
    pub ret: Option<HookReturn>,
}

/// A setting of the game that the player chooses from a list: the `[[option]]` sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameOption {
    pub key: String,
    pub label: String,
    pub names: Vec<String>,
    /// Index into `names`.
    pub default: usize,
}

/// Longest `key` of a game option (the runtime keeps it in 32 bytes with the terminator).
pub const MAX_OPTION_KEY: usize = 31;
/// Most options a profile can declare (the runtime keeps a fixed table).
pub const MAX_OPTIONS: usize = 16;

/// Who a profile is: the `[game]` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameId {
    pub id: String,
    pub name: String,
    pub crc32: u32,
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
    pub game: Option<GameId>,
    pub hooks: Vec<Hook>,
    pub options: Vec<GameOption>,
    /// The block of the main loop where the runtime may save and load states (`[resume]`).
    pub resume: Option<u32>,
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

        if let Some(g) = raw.game {
            let hex = g.crc32.trim().trim_start_matches("0x");
            let crc32 = u32::from_str_radix(hex, 16).ok().filter(|_| hex.len() == 8).ok_or_else(|| format!("[game]: crc32 must be 8 hex digits, got {:?}", g.crc32))?;
            if g.id.is_empty() || !g.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                return Err(format!("[game]: id must be letters, digits, `_` or `-`, got {:?}", g.id));
            }
            cfg.game = Some(GameId { id: g.id, name: g.name, crc32 });
        }

        for h in raw.hook {
            let addr = parse_addr(&h.addr)?;
            let ident = h.call.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && h.call.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            if !ident {
                return Err(format!("hook at {}: call must be a C identifier, got {:?}", h.addr, h.call));
            }
            match (h.kind, h.ret) {
                (HookKind::Replace, None) => return Err(format!("hook at {}: replace needs `ret` (rts or rtl)", h.addr)),
                (HookKind::Before | HookKind::After, Some(_)) => return Err(format!("hook at {}: only replace can set `ret`", h.addr)),
                _ => {}
            }
            if cfg.hooks.iter().any(|o| o.addr == addr && o.kind == h.kind) {
                return Err(format!("duplicate {:?} hook for {}", h.kind, h.addr));
            }
            cfg.hooks.push(Hook { addr, kind: h.kind, call: h.call, ret: h.ret });
        }

        if let Some(r) = raw.resume {
            cfg.resume = Some(parse_addr(&r.addr)?);
        }

        for o in raw.option {
            let key = &o.key;
            if key.is_empty() || key.len() > MAX_OPTION_KEY || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return Err(format!("option {key:?}: key must be 1 to {MAX_OPTION_KEY} letters, digits or `_`"));
            }
            if cfg.options.len() == MAX_OPTIONS {
                return Err(format!("option {key:?}: a profile can declare at most {MAX_OPTIONS} options"));
            }
            if o.label.is_empty() || !o.label.chars().all(|c| (' '..='~').contains(&c)) {
                return Err(format!("option {key:?}: label must be printable ASCII (the menu font has no more), got {:?}", o.label));
            }
            if cfg.options.iter().any(|x| &x.key == key) {
                return Err(format!("option {key:?}: duplicate key"));
            }
            if o.names.len() < 2 {
                return Err(format!("option {key:?}: names needs at least two entries"));
            }
            if o.names.iter().any(|n| n.is_empty() || !n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')) {
                return Err(format!("option {key:?}: names are letters, digits, `_`, `-` or `.`, got {:?}", o.names));
            }
            if (1..o.names.len()).any(|i| o.names[..i].contains(&o.names[i])) {
                return Err(format!("option {key:?}: names must be different, got {:?}", o.names));
            }
            let Some(default) = o.names.iter().position(|n| *n == o.default) else {
                return Err(format!("option {key:?}: default {:?} is not one of {:?}", o.default, o.names));
            };
            cfg.options.push(GameOption { key: o.key, label: o.label, names: o.names, default });
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
    fn the_real_game_profile_parses() {
        let Some(game) = crate::cctest::real_game() else { return };
        let cfg = Config::parse(&game.profile.config).unwrap();
        assert!(!cfg.entries.is_empty());
        assert_eq!(cfg.game.as_ref().map(|g| g.crc32), Some(game.profile.crc32));
    }

    #[test]
    fn parses_resume() {
        assert_eq!(Config::parse("").unwrap().resume, None);
        assert_eq!(Config::parse("[resume]
addr = \"00:8034\"").unwrap().resume, Some(0x8034));
        assert!(Config::parse("[resume]
addr = \"zz\"").is_err());
        assert!(Config::parse("[resume]
addr = \"00:8034\"
foo = 1").is_err());
    }

    #[test]
    fn parses_game_and_hooks() {
        let cfg = Config::parse(
            "[game]
id=\"g1\"
name=\"Game One\"
crc32=\"0000ABCD\"
[[hook]]
addr=\"00:8000\"
kind=\"before\"
call=\"on_reset\"
[[hook]]
addr=\"00:8000\"
kind=\"replace\"
call=\"f2\"
ret=\"rtl\"",
        )
        .unwrap();
        assert_eq!(cfg.game, Some(GameId { id: "g1".into(), name: "Game One".into(), crc32: 0xABCD }));
        assert_eq!(cfg.hooks.len(), 2);
        assert_eq!(cfg.hooks[1], Hook { addr: 0x8000, kind: HookKind::Replace, call: "f2".into(), ret: Some(HookReturn::Rtl) });
        assert!(Config::parse("[[entry]]
addr=\"00:8000\"").unwrap().game.is_none());
    }

    #[test]
    fn rejects_bad_game_and_hooks() {
        let hook = |extra: &str| format!("[[hook]]
addr=\"00:8000\"
call=\"f\"
{extra}");
        assert!(Config::parse(&hook("kind=\"replace\"")).is_err(), "replace without ret");
        assert!(Config::parse(&hook("kind=\"before\"
ret=\"rts\"")).is_err(), "ret without replace");
        assert!(Config::parse("[[hook]]
addr=\"00:8000\"
kind=\"before\"
call=\"a b\"").is_err(), "not an identifier");
        assert!(Config::parse(&format!("{}
{}", hook("kind=\"before\""), hook("kind=\"before\""))).is_err(), "duplicate");
        assert!(Config::parse("[game]
id=\"g\"
name=\"n\"
crc32=\"12\"").is_err(), "short crc");
        assert!(Config::parse("[game]
id=\"g\"
name=\"n\"").is_err(), "missing crc");
        assert!(Config::parse("[game]
id=\"a/b\"
name=\"n\"
crc32=\"0000ABCD\"").is_err(), "bad id");
    }

    #[test]
    fn parses_game_options() {
        let cfg = Config::parse("[[option]]
key=\"language\"
label=\"Language\"
names=[\"en\",\"es\"]
default=\"es\"
[[option]]
key=\"b\"
label=\"B\"
names=[\"x\",\"y\",\"z\"]
default=\"x\"").unwrap();
        assert_eq!(cfg.options.len(), 2);
        assert_eq!(cfg.options[0], GameOption { key: "language".into(), label: "Language".into(), names: vec!["en".into(), "es".into()], default: 1 });
        assert_eq!(cfg.options[1].default, 0);
        assert!(Config::parse("[[entry]]
addr=\"00:8000\"").unwrap().options.is_empty(), "no options without [[option]]");
    }

    #[test]
    fn rejects_bad_game_options() {
        let opt = |k: &str, names: &str, d: &str| format!("[[option]]
key=\"{k}\"
label=\"L\"
names={names}
default=\"{d}\"");
        assert!(Config::parse(&opt("language", "[\"en\",\"es\"]", "fr")).unwrap_err().contains("language"), "default outside the list");
        assert!(Config::parse(&format!("{}
{}", opt("language", "[\"en\",\"es\"]", "en"), opt("language", "[\"a\",\"b\"]", "a"))).unwrap_err().contains("duplicate"));
        assert!(Config::parse(&opt("language", "[\"en\"]", "en")).is_err(), "one name");
        assert!(Config::parse(&opt("language", "[\"en\",\"en\"]", "en")).is_err(), "repeated names");
        assert!(Config::parse(&opt("bad key", "[\"a\",\"b\"]", "a")).is_err(), "key with a space");
        assert!(Config::parse(&opt(&"k".repeat(32), "[\"a\",\"b\"]", "a")).is_err(), "key too long");
        assert!(Config::parse(&opt(&"k".repeat(31), "[\"a\",\"b\"]", "a")).is_ok());
        assert!(Config::parse(&opt("k", "[\"a b\",\"c\"]", "c")).is_err(), "name with a space");
        assert!(Config::parse("[[option]]\nkey=\"k\"\nlabel=\"Idioma \u{f1}\"\nnames=[\"a\",\"b\"]\ndefault=\"a\"").is_err(), "label with a non-ASCII letter");
        let many: String = (0..17).map(|i| opt(&format!("k{i}"), "[\"a\",\"b\"]", "a") + "\n").collect();
        assert!(Config::parse(&many).unwrap_err().contains("at most 16"), "more than 16 options");
        assert!(Config::parse("[[option]]
key=\"k\"
label=\"L\"
names=[\"a\",\"b\"]
default=\"a\"
typo=1").is_err());
    }

    #[test]
    fn rejects_bad_input() {
        assert!(Config::parse("[[table]]\nkind=\"indirect\"\nsite=\"00:8000\"\ncount=1").is_err());
        assert!(Config::parse("[[entry]]\naddr=\"zz\"").is_err());
        assert!(Config::parse("[[entry]]\naddr=\"00:8000\"\nm=12").is_err());
        assert!(Config::parse("[[entry]]\naddr=\"00:8000\"\ntypo=1").is_err());
    }
}
