//! Recursive-descent code discovery, guided by the per-game config.

use crate::config::{Config, Table, TableKind};
use crate::disasm::{decode, Flags, Flow, Instr};
use crate::rom::Rom;
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Debug, Default)]
pub struct Discovery {
    pub instrs: BTreeMap<u32, Instr>,
    /// Indirect jumps/calls reached with no table configured: candidates for the config.
    pub unresolved: BTreeSet<u32>,
}

fn next_addr(addr: u32, len: u8) -> u32 {
    (addr & 0xFF_0000) | ((addr + len as u32) & 0xFFFF)
}

/// Read the pointers of `table`, whose first entry is at `at`; `bank` is used for 16-bit pointers.
fn table_targets(rom: &Rom, bank: u32, at: u32, table: &Table) -> Vec<u32> {
    let size = table.size as u32;
    (0..table.count as u32)
        .filter_map(|i| {
            let p = (at & 0xFF_0000) | ((at + i * size) & 0xFFFF);
            let lo = rom.read16(p)? as u32;
            if size == 2 {
                Some(bank | ((lo as i32 + table.bias) as u32 & 0xFFFF))
            } else {
                let hi = rom.read8((p & 0xFF_0000) | ((p + 2) & 0xFFFF))? as u32;
                Some(((lo | hi << 16) as i32 + table.bias) as u32 & 0xFF_FFFF)
            }
        })
        .collect()
}

/// Targets of a `split` table: low, high and (optionally) bank bytes in parallel arrays.
fn split_targets(rom: &Rom, site_bank: u32, table: &Table) -> Vec<u32> {
    let split = table.split.expect("validated by Config::parse");
    let at = |base: u32, i: u32| (base & 0xFF_0000) | ((base + i) & 0xFFFF);
    (0..table.count as u32)
        .filter_map(|i| {
            let lo = rom.read8(at(split.lo, i))? as u32;
            let hi = rom.read8(at(split.hi, i))? as u32;
            let bank = match split.bank {
                Some(b) => (rom.read8(at(b, i))? as u32) << 16,
                None => site_bank,
            };
            Some(bank | ((((hi << 8) | lo) as i32 + table.bias) as u32 & 0xFFFF))
        })
        .collect()
}

/// Longest run of entries that `inline_helper_targets` will accept: helpers index with an 8-bit value.
const MAX_INLINE_ENTRIES: u32 = 256;

/// Targets of the inline table that follows a call to an `inline_helper`; `start` is the first entry.
/// The length is not known, so entries are taken while they look like code pointers and stop at the
/// lowest same-bank target above the table, which is where the code after the table begins.
fn inline_helper_targets(rom: &Rom, start: u32, size: u8) -> Vec<u32> {
    let bank = start & 0xFF_0000;
    let mut limit = 0x1_0000u32;
    let mut out = Vec::new();
    for i in 0..MAX_INLINE_ENTRIES {
        let off = (start & 0xFFFF) + i * size as u32;
        if off >= limit || off > 0xFFFF {
            break;
        }
        let p = bank | off;
        let Some(lo) = rom.read16(p).map(u32::from) else { break };
        if lo == 0 {
            // An unused slot; the table goes on.
            continue;
        }
        if lo < 0x8000 {
            break;
        }
        let target = if size == 2 {
            bank | lo
        } else {
            let Some(b) = rom.read8(bank | ((off + 2) & 0xFFFF)) else { break };
            (b as u32) << 16 | lo
        };
        // A bank past the end of the ROM would only wrap around to unrelated data.
        if ((target >> 16) as usize & 0x7F) << 15 >= rom.len() {
            break;
        }
        if target & 0xFF_0000 == bank && (target & 0xFFFF) > (start & 0xFFFF) {
            limit = limit.min(target & 0xFFFF);
        }
        out.push(target);
    }
    out
}

pub fn discover(rom: &Rom, cfg: &Config, roots: &[(u32, Flags)]) -> Discovery {
    let mut out = Discovery::default();
    let mut seen: HashSet<(u32, Flags)> = HashSet::new();
    let mut work: Vec<(u32, Flags)> = roots.to_vec();

    while let Some((mut addr, mut f)) = work.pop() {
        loop {
            if cfg.not_code.contains(&addr) {
                break;
            }
            if let Some(&forced) = cfg.assume.get(&addr) {
                f = forced;
            }
            if !seen.insert((addr, f)) {
                break;
            }
            let Some(ins) = decode(rom, addr, f) else { break };
            let after = next_addr(addr, ins.len);
            let nf = ins.next_flags(f);
            let flow = ins.flow();
            let table = cfg.tables.get(&addr);
            out.instrs.entry(addr).or_insert(ins);

            let mut stop = false;
            match flow {
                Flow::Next | Flow::IndirectCall | Flow::IndirectJump => {}
                Flow::Branch(t) => work.push((t, nf)),
                Flow::Call(t) => {
                    work.push((t, nf));
                    stop = cfg.no_return.contains(&t);
                }
                Flow::Jump(t) => {
                    work.push((t, nf));
                    stop = true;
                }
                Flow::Return | Flow::Stop => stop = true,
            }

            if let (None, Flow::Call(t)) = (table, flow) {
                if let Some(h) = cfg.inline_helpers.get(&t) {
                    for target in inline_helper_targets(rom, after, h.size) {
                        work.push((target, nf));
                    }
                    stop = true;
                }
            }

            match (table, flow) {
                (Some(t), _) if t.kind == TableKind::Inline => {
                    for target in table_targets(rom, addr & 0xFF_0000, after, t) {
                        work.push((target, nf));
                    }
                    stop = stop || !t.falls_through;
                }
                (Some(t), Flow::IndirectJump | Flow::IndirectCall) => {
                    let targets = match t.kind {
                        TableKind::Split => split_targets(rom, addr & 0xFF_0000, t),
                        _ => table_targets(rom, addr & 0xFF_0000, t.at.expect("validated by Config::parse"), t),
                    };
                    for target in targets {
                        work.push((target, nf));
                    }
                    stop = stop || flow == Flow::IndirectJump;
                }
                (None, Flow::IndirectJump) => {
                    if !cfg.helper_exits.contains(&addr) {
                        out.unresolved.insert(addr);
                    }
                    stop = true;
                }
                (None, Flow::IndirectCall) => {
                    out.unresolved.insert(addr);
                }
                _ => {}
            }

            if stop {
                break;
            }
            // An inline table sits between the call and the next instruction.
            addr = match table {
                Some(t) if t.kind == TableKind::Inline => {
                    (after & 0xFF_0000) | ((after + (t.count * t.size as usize) as u32) & 0xFFFF)
                }
                _ => after,
            };
            f = nf;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rom::test_rom;

    fn addrs(d: &Discovery) -> Vec<u32> {
        d.instrs.keys().copied().collect()
    }

    #[test]
    fn follows_calls() {
        // 8000: JSR $8010 ; 8003: BRA $8003 (loop)   8010: LDA #$01 ; RTS
        let rom = test_rom(&[
            (0x8000, &[0x20, 0x10, 0x80, 0x80, 0xFE]),
            (0x8010, &[0xA9, 0x01, 0x60]),
        ]);
        let d = discover(&rom, &Config::default(), &[(0x8000, Flags::RESET)]);
        assert_eq!(addrs(&d), [0x8000, 0x8003, 0x8010, 0x8012]);
    }

    #[test]
    fn resolves_indirect_table() {
        // 8000: JMP ($8100,X)   table at 8100: $8010, $8020
        let rom = test_rom(&[
            (0x8000, &[0x7C, 0x00, 0x81]),
            (0x8100, &[0x10, 0x80, 0x20, 0x80]),
            (0x8010, &[0x60]),
            (0x8020, &[0xEA, 0x60]),
        ]);
        let unresolved = discover(&rom, &Config::default(), &[(0x8000, Flags::RESET)]);
        assert_eq!(addrs(&unresolved), [0x8000]);
        assert!(unresolved.unresolved.contains(&0x8000));

        let cfg = Config::parse(
            "[[table]]\nkind=\"indirect\"\nsite=\"00:8000\"\nat=\"00:8100\"\ncount=2",
        )
        .unwrap();
        let d = discover(&rom, &cfg, &[(0x8000, Flags::RESET)]);
        assert_eq!(addrs(&d), [0x8000, 0x8010, 0x8020, 0x8021]);
        assert!(d.unresolved.is_empty());
    }

    #[test]
    fn resolves_split_table() {
        // 8000: JML [$03] ; lo bytes at 8100, hi bytes at 8110, bank bytes at 8120
        let rom = test_rom(&[
            (0x8000, &[0xDC, 0x03, 0x00]),
            (0x8100, &[0x10, 0x20]),
            (0x8110, &[0x80, 0x80]),
            (0x8120, &[0x00, 0x01]),
            (0x8010, &[0x60]),
            (0x1_8020, &[0x60]),
        ]);
        let cfg = Config::parse(
            "[[table]]
kind=\"split\"
site=\"00:8000\"
lo=\"00:8100\"
hi=\"00:8110\"
bank=\"00:8120\"
count=2",
        )
        .unwrap();
        let d = discover(&rom, &cfg, &[(0x8000, Flags::RESET)]);
        assert_eq!(addrs(&d), [0x8000, 0x8010, 0x01_8020]);
    }

    #[test]
    fn resolves_inline_table() {
        // 8000: JSR $8200 (dispatch helper, never returns) ; inline: $8010, $8020 (bias 0)
        let rom = test_rom(&[
            (0x8000, &[0x20, 0x00, 0x82, 0x10, 0x80, 0x20, 0x80]),
            (0x8200, &[0x60]),
            (0x8010, &[0x60]),
            (0x8020, &[0x60]),
        ]);
        let cfg = Config::parse("[[table]]\nkind=\"inline\"\nsite=\"00:8000\"\ncount=2").unwrap();
        let d = discover(&rom, &cfg, &[(0x8000, Flags::RESET)]);
        assert_eq!(addrs(&d), [0x8000, 0x8010, 0x8020, 0x8200]);
    }

    #[test]
    fn inline_table_with_fallthrough_continues_after_table() {
        // 8000: JSR $8200 ; inline: $8010 ; 8005: RTS
        let rom = test_rom(&[
            (0x8000, &[0x20, 0x00, 0x82, 0x10, 0x80, 0x60]),
            (0x8200, &[0x60]),
            (0x8010, &[0x60]),
        ]);
        let cfg = Config::parse(
            "[[table]]\nkind=\"inline\"\nsite=\"00:8000\"\ncount=1\nfalls_through=true",
        )
        .unwrap();
        let d = discover(&rom, &cfg, &[(0x8000, Flags::RESET)]);
        assert_eq!(addrs(&d), [0x8000, 0x8005, 0x8010, 0x8200]);
    }

    #[test]
    fn inline_helper_stops_at_first_target_above_table() {
        // 8000: JSR $8200 (helper) ; inline: $8005, then code that must not be read as a pointer
        let rom = test_rom(&[
            (0x8000, &[0x20, 0x00, 0x82, 0x05, 0x80, 0xA9, 0x90, 0x60]),
            (0x8200, &[0x60]),
        ]);
        let cfg = Config::parse("[[inline_helper]]
addr=\"00:8200\"").unwrap();
        let d = discover(&rom, &cfg, &[(0x8000, Flags::RESET)]);
        assert_eq!(addrs(&d), [0x8000, 0x8005, 0x8007, 0x8200]);
    }

    #[test]
    fn inline_helper_skips_unused_zero_slots() {
        // 8000: JSR $8200 ; inline: $8010, (unused), $8020
        let rom = test_rom(&[
            (0x8000, &[0x20, 0x00, 0x82, 0x10, 0x80, 0x00, 0x00, 0x20, 0x80]),
            (0x8200, &[0x60]),
            (0x8010, &[0x60]),
            (0x8020, &[0x60]),
        ]);
        let cfg = Config::parse("[[inline_helper]]
addr=\"00:8200\"").unwrap();
        let d = discover(&rom, &cfg, &[(0x8000, Flags::RESET)]);
        assert_eq!(addrs(&d), [0x8000, 0x8010, 0x8020, 0x8200]);
    }

    #[test]
    fn inline_helper_reads_long_pointers() {
        // 8000: JSR $8200 ; inline long pointers: 00:8010, 01:8020, then a non-pointer
        let rom = test_rom(&[
            (0x8000, &[0x20, 0x00, 0x82, 0x10, 0x80, 0x00, 0x20, 0x80, 0x01]),
            (0x8200, &[0x60]),
            (0x8010, &[0x60]),
            (0x1_8020, &[0x60]),
        ]);
        let cfg = Config::parse("[[inline_helper]]
addr=\"00:8200\"
size=3").unwrap();
        let d = discover(&rom, &cfg, &[(0x8000, Flags::RESET)]);
        assert_eq!(addrs(&d), [0x8000, 0x8010, 0x8200, 0x01_8020]);
    }

    #[test]
    fn helper_exit_is_not_reported_as_unresolved() {
        // 8000: JML [$0000] is the helper's own dispatch jump
        let rom = test_rom(&[(0x8000, &[0xDC, 0x00, 0x00])]);
        let d = discover(&rom, &Config::default(), &[(0x8000, Flags::RESET)]);
        assert!(d.unresolved.contains(&0x8000));
        let cfg = Config::parse("[[inline_helper]]
addr=\"00:8000\"
exit=\"00:8000\"").unwrap();
        let d = discover(&rom, &cfg, &[(0x8000, Flags::RESET)]);
        assert!(d.unresolved.is_empty());
    }

    #[test]
    fn not_code_is_not_decoded() {
        // 8000: JMP $8010 ; 8010 is padding a table points at
        let rom = test_rom(&[(0x8000, &[0x4C, 0x10, 0x80]), (0x8010, &[0xEA])]);
        let cfg = Config::parse("[[not_code]]
addr=\"00:8010\"").unwrap();
        let d = discover(&rom, &cfg, &[(0x8000, Flags::RESET)]);
        assert_eq!(addrs(&d), [0x8000]);
    }

    #[test]
    fn no_return_stops_fallthrough() {
        // 8000: JSR $8100 ; 8003: NOP (dead if $8100 never returns)
        let rom = test_rom(&[(0x8000, &[0x20, 0x00, 0x81, 0xEA]), (0x8100, &[0x60])]);
        let cfg = Config::parse("[[no_return]]\naddr=\"00:8100\"").unwrap();
        let d = discover(&rom, &cfg, &[(0x8000, Flags::RESET)]);
        assert_eq!(addrs(&d), [0x8000, 0x8100]);
    }
}
