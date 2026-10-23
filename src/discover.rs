//! Recursive-descent code discovery, guided by the per-game config.

use crate::config::{Config, Table, TableKind};
use crate::disasm::{decode, Flags, Flow, Instr};
use crate::rom::Rom;
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdgeKind {
    Branch,
    Jump,
    Call,
    /// Target of a jump table or inline helper table.
    Dispatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Edge {
    pub to: u32,
    pub kind: EdgeKind,
}

#[derive(Debug, Default)]
pub struct Discovery {
    pub instrs: BTreeMap<u32, Instr>,
    /// Control transfers out of each instruction, except the fall-through.
    pub edges: BTreeMap<u32, BTreeSet<Edge>>,
    /// Where execution continues after an instruction, when discovery went on past it.
    pub fallthrough: BTreeMap<u32, u32>,
    /// Register widths in effect before each instruction (the first combination found).
    pub flags: BTreeMap<u32, Flags>,
    /// Addresses reached with more than one combination of widths, with all of them.
    pub flag_conflicts: BTreeMap<u32, BTreeSet<Flags>>,
    /// Indirect jumps/calls reached with no table configured: candidates for the config.
    pub unresolved: BTreeSet<u32>,
    /// Calls (by address) whose callee never returns in the analysis and is not declared `no_return`.
    pub calls_without_return: BTreeSet<u32>,
    /// `PLP` instructions that pop something other than a `PHP` from the same path.
    pub untracked_plp: BTreeSet<u32>,
    /// Deepest abstract stack reached, before the limit is applied.
    pub max_stack: usize,
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

fn add_edge(out: &mut Discovery, cfg: &Config, from: u32, to: u32, kind: EdgeKind) {
    // Data that a table points at is not decoded, so it is not a graph node either.
    if !cfg.not_code.contains(&to) {
        out.edges.entry(from).or_default().insert(Edge { to, kind });
    }
}

/// A function being analysed: its entry and the widths it was entered with.
type Ctx = (u32, Flags);

/// One entry of the abstract stack that pairs `PHP` with `PLP`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Item {
    Flags(Flags),
    Other,
}

/// Entries kept in the abstract stack; the oldest is dropped past this, so the state space is finite.
const MAX_STACK: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Path {
    addr: u32,
    f: Flags,
    stack: Vec<Item>,
    ctx: Ctx,
}

/// A call to an inline helper whose targets wait for the widths at the helper's exit.
struct Deferred {
    helper: u32,
    targets: Vec<u32>,
    /// Widths at the call site, used if the exit is never reached.
    f: Flags,
    stack: Vec<Item>,
    ctx: Ctx,
}

/// What a caller does once the callee returns: continue at `ret` with the widths the callee left.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Cont {
    site: u32,
    before: Flags,
    ret: u32,
    stack: Vec<Item>,
    ctx: Ctx,
}

impl Cont {
    fn resume(&self, f: Flags) -> Path {
        Path { addr: self.ret, f, stack: self.stack.clone(), ctx: self.ctx }
    }
}

/// Return summaries per function context and the continuations waiting on them.
#[derive(Default)]
struct Calls {
    returns: BTreeMap<Ctx, BTreeSet<Flags>>,
    conts: BTreeMap<Ctx, BTreeSet<Cont>>,
}

impl Calls {
    fn add_return(&mut self, ctx: Ctx, f: Flags, work: &mut Vec<Path>) {
        if self.returns.entry(ctx).or_default().insert(f) {
            for c in self.conts.get(&ctx).into_iter().flatten() {
                work.push(c.resume(f));
            }
        }
    }

    fn subscribe(&mut self, callee: Ctx, c: Cont, work: &mut Vec<Path>) {
        if self.conts.entry(callee).or_default().insert(c.clone()) {
            for &f in self.returns.get(&callee).into_iter().flatten() {
                work.push(c.resume(f));
            }
        }
    }

    fn returns_nothing(&self, ctx: &Ctx) -> bool {
        self.returns.get(ctx).map_or(true, |r| r.is_empty())
    }
}

pub fn discover(rom: &Rom, cfg: &Config, roots: &[(u32, Flags)]) -> Discovery {
    let mut out = Discovery::default();
    let mut seen: HashSet<Path> = HashSet::new();
    let mut all_flags: BTreeMap<u32, BTreeSet<Flags>> = BTreeMap::new();
    let mut calls = Calls::default();
    let mut resolved: BTreeSet<(Ctx, Cont)> = BTreeSet::new();
    let mut deferred: Vec<Deferred> = Vec::new();
    let mut helper_targets: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    let mut started: HashSet<(usize, Flags)> = HashSet::new();
    let mut work: Vec<Path> = roots
        .iter()
        .map(|&(a, f)| Path { addr: a, f, stack: Vec::new(), ctx: (a, f) })
        .collect();

    loop {
        while let Some(Path { mut addr, mut f, mut stack, ctx }) = work.pop() {
            loop {
                if cfg.not_code.contains(&addr) {
                    break;
                }
                if let Some(&forced) = cfg.assume.get(&addr) {
                    f = forced;
                }
                if !seen.insert(Path { addr, f, stack: stack.clone(), ctx }) {
                    break;
                }
                out.flags.entry(addr).or_insert(f);
                all_flags.entry(addr).or_default().insert(f);
                let Some(ins) = decode(rom, addr, f) else { break };
                let after = next_addr(addr, ins.len);
                let mut nf = ins.next_flags(f);
                match ins.mnemonic {
                    "PHP" | "PHA" | "PHX" | "PHY" | "PHB" | "PHD" | "PHK" | "PEA" | "PEI" | "PER" => {
                        stack.push(if ins.mnemonic == "PHP" { Item::Flags(f) } else { Item::Other });
                        out.max_stack = out.max_stack.max(stack.len());
                        if stack.len() > MAX_STACK {
                            stack.remove(0);
                        }
                    }
                    "PLP" => match stack.pop() {
                        Some(Item::Flags(saved)) => nf = saved,
                        _ => {
                            out.untracked_plp.insert(addr);
                        }
                    },
                    "PLA" | "PLX" | "PLY" | "PLB" | "PLD" => {
                        stack.pop();
                    }
                    _ => {}
                }
                let flow = ins.flow();
                let table = cfg.tables.get(&addr);
                out.instrs.entry(addr).or_insert(ins);
                let here = |a: u32, stack: &[Item]| Path { addr: a, f: nf, stack: stack.to_vec(), ctx };
                let callee = |t: u32| Path { addr: t, f: nf, stack: Vec::new(), ctx: (t, nf) };

                let mut stop = false;
                // Functions this instruction calls; the code after it resumes when they return.
                let mut called: Vec<u32> = Vec::new();
                match flow {
                    Flow::Next | Flow::IndirectCall | Flow::IndirectJump => {}
                    Flow::Branch(t) => {
                        add_edge(&mut out, cfg, addr, t, EdgeKind::Branch);
                        work.push(here(t, &stack));
                    }
                    Flow::Call(t) => {
                        add_edge(&mut out, cfg, addr, t, EdgeKind::Call);
                        work.push(callee(t));
                        stop = cfg.no_return.contains(&t);
                        if !stop {
                            called.push(t);
                        }
                    }
                    Flow::Jump(t) => {
                        add_edge(&mut out, cfg, addr, t, EdgeKind::Jump);
                        work.push(here(t, &stack));
                        stop = true;
                    }
                    Flow::Return => {
                        calls.add_return(ctx, nf, &mut work);
                        stop = true;
                    }
                    Flow::Stop => stop = true,
                }

                if let (None, Flow::Call(t)) = (table, flow) {
                    if let Some(h) = cfg.inline_helpers.get(&t) {
                        let targets = inline_helper_targets(rom, after, h.size);
                        for &target in &targets {
                            add_edge(&mut out, cfg, addr, target, EdgeKind::Dispatch);
                        }
                        match h.exit {
                            // The targets run with the widths the helper leaves at its exit.
                            Some(_) => deferred.push(Deferred { helper: t, targets, f: nf, stack: stack.clone(), ctx }),
                            None => work.extend(targets.into_iter().map(|target| here(target, &stack))),
                        }
                        called.clear();
                        stop = true;
                    }
                }

                match (table, flow) {
                    (Some(t), _) if t.kind == TableKind::Inline => {
                        for target in table_targets(rom, addr & 0xFF_0000, after, t) {
                            add_edge(&mut out, cfg, addr, target, EdgeKind::Dispatch);
                            work.push(here(target, &stack));
                        }
                        stop = stop || !t.falls_through;
                        if stop {
                            called.clear();
                        }
                    }
                    (Some(t), Flow::IndirectJump | Flow::IndirectCall) => {
                        let targets = match t.kind {
                            TableKind::Split => split_targets(rom, addr & 0xFF_0000, t),
                            _ => table_targets(rom, addr & 0xFF_0000, t.at.expect("validated by Config::parse"), t),
                        };
                        for target in targets {
                            add_edge(&mut out, cfg, addr, target, EdgeKind::Dispatch);
                            if flow == Flow::IndirectCall {
                                work.push(callee(target));
                                if !cfg.no_return.contains(&target) {
                                    called.push(target);
                                }
                            } else {
                                work.push(here(target, &stack));
                            }
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
                let next = match table {
                    Some(t) if t.kind == TableKind::Inline => {
                        (after & 0xFF_0000) | ((after + (t.count * t.size as usize) as u32) & 0xFFFF)
                    }
                    _ => after,
                };
                out.fallthrough.insert(addr, next);
                if !called.is_empty() {
                    for t in called {
                        let c = Cont { site: addr, before: nf, ret: next, stack: stack.clone(), ctx };
                        calls.subscribe((t, nf), c, &mut work);
                    }
                    break;
                }
                addr = next;
                f = nf;
            }
        }

        // Targets of inline helpers go on with the widths found at the helper's exit.
        let mut progress = false;
        for (i, d) in deferred.iter().enumerate() {
            let exit = cfg.inline_helpers[&d.helper].exit.expect("deferred helpers have an exit");
            let widths: Vec<Flags> = match all_flags.get(&exit) {
                Some(w) => w.iter().copied().collect(),
                None => vec![d.f],
            };
            helper_targets.entry(d.helper).or_default().extend(&d.targets);
            for f in widths {
                if started.insert((i, f)) {
                    progress = true;
                    work.extend(d.targets.iter().map(|&t| Path { addr: t, f, stack: d.stack.clone(), ctx: d.ctx }));
                }
            }
        }
        if progress {
            continue;
        }

        // Calls whose callee never returned: go on with the widths from before the call.
        let mut pending: Vec<(Ctx, Cont)> = Vec::new();
        for (callee, cs) in &calls.conts {
            if calls.returns_nothing(callee) {
                for c in cs {
                    if !resolved.contains(&(*callee, c.clone())) {
                        pending.push((*callee, c.clone()));
                    }
                }
            }
        }
        if pending.is_empty() {
            break;
        }
        for (callee, c) in pending {
            out.calls_without_return.insert(c.site);
            work.push(c.resume(c.before));
            resolved.insert((callee, c));
        }
    }
    // The helper's own jump can reach any target of any of its calls.
    for (helper, targets) in &helper_targets {
        let exit = cfg.inline_helpers[helper].exit.expect("deferred helpers have an exit");
        if out.instrs.contains_key(&exit) {
            for &t in targets {
                add_edge(&mut out, cfg, exit, t, EdgeKind::Dispatch);
            }
        }
    }
    out.flag_conflicts = all_flags.into_iter().filter(|(_, v)| v.len() > 1).collect();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rom::test_rom;

    fn addrs(d: &Discovery) -> Vec<u32> {
        d.instrs.keys().copied().collect()
    }

    fn edges_of(d: &Discovery, site: u32) -> Vec<(u32, EdgeKind)> {
        d.edges.get(&site).map_or(vec![], |e| e.iter().map(|e| (e.to, e.kind)).collect())
    }

    #[test]
    fn records_branch_jump_and_call_edges() {
        // 8000: BNE $8004 ; 8002: JSR $8010 ; 8005: JMP $8020  ...
        let rom = test_rom(&[
            (0x8000, &[0xD0, 0x02, 0x20, 0x10, 0x80, 0x4C, 0x20, 0x80]),
            (0x8010, &[0x60]),
            (0x8020, &[0x60]),
        ]);
        let d = discover(&rom, &Config::default(), &[(0x8000, Flags::RESET)]);
        assert_eq!(edges_of(&d, 0x8000), [(0x8004, EdgeKind::Branch)]);
        assert_eq!(edges_of(&d, 0x8002), [(0x8010, EdgeKind::Call)]);
        assert_eq!(edges_of(&d, 0x8005), [(0x8020, EdgeKind::Jump)]);
        assert_eq!(d.fallthrough.get(&0x8000), Some(&0x8002));
        assert_eq!(d.fallthrough.get(&0x8002), Some(&0x8005));
        assert!(!d.fallthrough.contains_key(&0x8005));
        assert!(!d.fallthrough.contains_key(&0x8010));
    }

    #[test]
    fn records_dispatch_edges_and_skips_inline_table() {
        let rom = test_rom(&[
            (0x8000, &[0x7C, 0x00, 0x81]),
            (0x8100, &[0x10, 0x80, 0x20, 0x80]),
            (0x8010, &[0x60]),
            (0x8020, &[0x60]),
            (0x9000, &[0x20, 0x00, 0x82, 0x10, 0x80, 0x60]),
            (0x8200, &[0x60]),
        ]);
        let cfg = Config::parse(
            "[[table]]
kind=\"indirect\"
site=\"00:8000\"
at=\"00:8100\"
count=2
             [[table]]
kind=\"inline\"
site=\"00:9000\"
count=1
falls_through=true",
        )
        .unwrap();
        let d = discover(&rom, &cfg, &[(0x8000, Flags::RESET), (0x9000, Flags::RESET)]);
        assert_eq!(edges_of(&d, 0x8000), [(0x8010, EdgeKind::Dispatch), (0x8020, EdgeKind::Dispatch)]);
        assert!(!d.fallthrough.contains_key(&0x8000));
        assert_eq!(edges_of(&d, 0x9000), [(0x8010, EdgeKind::Dispatch), (0x8200, EdgeKind::Call)]);
        assert_eq!(d.fallthrough.get(&0x9000), Some(&0x9005));
    }

    #[test]
    fn records_register_widths_per_instruction() {
        // 8000: REP #$20 ; 8002: LDA #$1234 ; 8005: SEP #$20 ; 8007: LDA #$12 ; 8009: RTS
        let rom = test_rom(&[(
            0x8000,
            &[0xC2, 0x20, 0xA9, 0x34, 0x12, 0xE2, 0x20, 0xA9, 0x12, 0x60],
        )]);
        let d = discover(&rom, &Config::default(), &[(0x8000, Flags::RESET)]);
        assert_eq!(d.flags[&0x8000], Flags { m: true, x: true });
        assert_eq!(d.flags[&0x8002], Flags { m: false, x: true });
        assert_eq!(d.flags[&0x8007], Flags { m: true, x: true });
        assert!(d.flag_conflicts.is_empty());
    }

    #[test]
    fn reports_addresses_reached_with_different_widths() {
        // 8000: BEQ $8004 ; 8002: REP #$20 ; 8004: NOP (reached with M=1 and M=0) ; 8005: RTS
        let rom = test_rom(&[(0x8000, &[0xF0, 0x02, 0xC2, 0x20, 0xEA, 0x60])]);
        let d = discover(&rom, &Config::default(), &[(0x8000, Flags::RESET)]);
        assert_eq!(
            d.flag_conflicts[&0x8004],
            BTreeSet::from([Flags { m: false, x: true }, Flags { m: true, x: true }])
        );
        assert!(!d.flag_conflicts.contains_key(&0x8000));
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
    fn helper_exit_dispatches_to_the_union_of_its_calls() {
        // Two callers of the helper at 8200, each with its own table; 8200: JML [$0000]
        let rom = test_rom(&[
            (0x8000, &[0x20, 0x00, 0x82, 0x10, 0x80, 0x60, 0x00]),
            (0x9000, &[0x20, 0x00, 0x82, 0x20, 0x80, 0x60, 0x00]),
            (0x8200, &[0xDC, 0x00, 0x00]),
            (0x8010, &[0x60]),
            (0x8020, &[0x60]),
        ]);
        let cfg = width_test_cfg("[[inline_helper]]
addr=\"00:8200\"
exit=\"00:8200\"");
        let d = discover(&rom, &cfg, &[(0x8000, Flags::RESET), (0x9000, Flags::RESET)]);
        assert_eq!(edges_of(&d, 0x8200), [(0x8010, EdgeKind::Dispatch), (0x8020, EdgeKind::Dispatch)]);
        assert_eq!(edges_of(&d, 0x8000), [(0x8010, EdgeKind::Dispatch), (0x8200, EdgeKind::Call)]);
        assert!(d.unresolved.is_empty());
    }

    #[test]
    fn helper_exit_without_calls_has_no_targets() {
        let rom = test_rom(&[(0x8200, &[0xDC, 0x00, 0x00])]);
        let cfg = width_test_cfg("[[inline_helper]]
addr=\"00:8200\"
exit=\"00:8200\"");
        let d = discover(&rom, &cfg, &[(0x8200, Flags::RESET)]);
        assert!(edges_of(&d, 0x8200).is_empty());
        assert!(d.unresolved.is_empty());
    }

    #[test]
    fn helper_targets_take_the_widths_at_the_exit() {
        // 8000: REP #$30 ; JSR $8200 ; inline: $8010   8200: SEP #$30 ; 8202: JML [$0000]
        // 8010: LDA #$12 (8 bits wide only if the exit's widths apply) ; RTS
        let rom = test_rom(&[
            (0x8000, &[0xC2, 0x30, 0x20, 0x00, 0x82, 0x10, 0x80, 0x60, 0x00]),
            (0x8200, &[0xE2, 0x30, 0xDC, 0x00, 0x00]),
            (0x8010, &[0xA9, 0x12, 0x60]),
        ]);
        let cfg = width_test_cfg("[[inline_helper]]
addr=\"00:8200\"
exit=\"00:8202\"");
        let d = discover(&rom, &cfg, &[(0x8000, Flags::RESET)]);
        assert_eq!(d.flags[&0x8010], Flags { m: true, x: true });
        assert_eq!(d.instrs[&0x8010].len, 2);
        assert!(!d.flag_conflicts.contains_key(&0x8010));
        assert_eq!(edges_of(&d, 0x8202), [(0x8010, EdgeKind::Dispatch)]);
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

    const M16X16: Flags = Flags { m: false, x: false };

    fn width_test_cfg(extra: &str) -> Config {
        Config::parse(extra).unwrap()
    }

    #[test]
    fn call_continues_with_the_widths_the_callee_returns() {
        // 8000: JSR $8010 ; 8003: LDA #$80 (M=8 after the call) ; 8005: RTS   8010: SEP #$20 ; RTS
        let rom = test_rom(&[
            (0x8000, &[0x20, 0x10, 0x80, 0xA9, 0x80, 0x60]),
            (0x8010, &[0xE2, 0x20, 0x60]),
        ]);
        let d = discover(&rom, &Config::default(), &[(0x8000, M16X16)]);
        assert_eq!(d.flags[&0x8003], Flags { m: true, x: false });
        assert_eq!(d.instrs[&0x8003].len, 2);
        assert!(d.instrs.contains_key(&0x8005));
        assert!(d.flag_conflicts.is_empty());
        assert!(d.calls_without_return.is_empty());
    }

    #[test]
    fn call_keeps_widths_when_callee_preserves_them() {
        let rom = test_rom(&[
            (0x8000, &[0x20, 0x10, 0x80, 0xEA, 0x60]),
            (0x8010, &[0xEA, 0x60]),
        ]);
        let d = discover(&rom, &Config::default(), &[(0x8000, M16X16)]);
        assert_eq!(d.flags[&0x8003], M16X16);
    }

    #[test]
    fn each_call_gets_the_returns_of_its_own_context() {
        // 8000: SEP #$20 ; JSR $8020 ; 8005: REP #$20 ; 8007: JSR $8020 ; 800A: RTS   8020: NOP ; RTS
        let rom = test_rom(&[
            (0x8000, &[0xE2, 0x20, 0x20, 0x20, 0x80, 0xC2, 0x20, 0x20, 0x20, 0x80, 0x60]),
            (0x8020, &[0xEA, 0x60]),
        ]);
        let d = discover(&rom, &Config::default(), &[(0x8000, M16X16)]);
        assert_eq!(d.flags[&0x8005].m, true);
        assert_eq!(d.flags[&0x800A].m, false);
        // Only the callee, entered both ways, is reached with two widths.
        assert_eq!(d.flag_conflicts.keys().copied().collect::<Vec<_>>(), [0x8020, 0x8021]);
    }

    #[test]
    fn returns_with_different_widths_are_a_conflict_after_the_call() {
        // 8010: BEQ $8015 ; 8012: SEP #$20 ; 8014: RTS ; 8015: RTS
        let rom = test_rom(&[
            (0x8000, &[0x20, 0x10, 0x80, 0xEA, 0x60]),
            (0x8010, &[0xF0, 0x03, 0xE2, 0x20, 0x60, 0x60]),
        ]);
        let d = discover(&rom, &Config::default(), &[(0x8000, M16X16)]);
        assert_eq!(
            d.flag_conflicts[&0x8003],
            BTreeSet::from([M16X16, Flags { m: true, x: false }])
        );
    }

    #[test]
    fn call_that_never_returns_continues_with_the_previous_widths() {
        // 8010: BRA $8010
        let rom = test_rom(&[
            (0x8000, &[0x20, 0x10, 0x80, 0xEA, 0x60]),
            (0x8010, &[0x80, 0xFE]),
        ]);
        let d = discover(&rom, &Config::default(), &[(0x8000, M16X16)]);
        assert_eq!(d.flags[&0x8003], M16X16);
        assert_eq!(d.calls_without_return, BTreeSet::from([0x8000]));
    }

    #[test]
    fn declared_no_return_is_neither_continued_nor_reported() {
        let rom = test_rom(&[(0x8000, &[0x20, 0x00, 0x81, 0xEA]), (0x8100, &[0x80, 0xFE])]);
        let cfg = width_test_cfg("[[no_return]]
addr=\"00:8100\"");
        let d = discover(&rom, &cfg, &[(0x8000, Flags::RESET)]);
        assert_eq!(addrs(&d), [0x8000, 0x8100]);
        assert!(d.calls_without_return.is_empty());
    }

    #[test]
    fn indirect_call_through_a_table_takes_the_callee_widths() {
        // 8000: JSR ($8100,X) ; 8003: NOP ; RTS   table: $8010   8010: SEP #$20 ; RTS
        let rom = test_rom(&[
            (0x8000, &[0xFC, 0x00, 0x81, 0xEA, 0x60]),
            (0x8100, &[0x10, 0x80]),
            (0x8010, &[0xE2, 0x20, 0x60]),
        ]);
        let cfg = width_test_cfg("[[table]]
kind=\"indirect\"
site=\"00:8000\"
at=\"00:8100\"
count=1");
        let d = discover(&rom, &cfg, &[(0x8000, M16X16)]);
        assert_eq!(d.flags[&0x8003], Flags { m: true, x: false });
    }

    #[test]
    fn dispatch_jump_returns_count_for_the_function_that_jumped() {
        // 8000: JSR $8010 ; 8003: NOP ; RTS   8010: JMP ($8100,X)   table: $8020   8020: SEP #$20 ; RTS
        let rom = test_rom(&[
            (0x8000, &[0x20, 0x10, 0x80, 0xEA, 0x60]),
            (0x8010, &[0x7C, 0x00, 0x81]),
            (0x8100, &[0x20, 0x80]),
            (0x8020, &[0xE2, 0x20, 0x60]),
        ]);
        let cfg = width_test_cfg("[[table]]
kind=\"indirect\"
site=\"00:8010\"
at=\"00:8100\"
count=1");
        let d = discover(&rom, &cfg, &[(0x8000, M16X16)]);
        assert_eq!(d.flags[&0x8003], Flags { m: true, x: false });
        assert!(d.calls_without_return.is_empty());
    }

    #[test]
    fn inline_helper_call_is_not_continued_nor_reported() {
        let rom = test_rom(&[
            (0x8000, &[0x20, 0x00, 0x82, 0x05, 0x80, 0xA9, 0x90, 0x60]),
            (0x8200, &[0x60]),
        ]);
        let cfg = width_test_cfg("[[inline_helper]]
addr=\"00:8200\"");
        let d = discover(&rom, &cfg, &[(0x8000, Flags::RESET)]);
        assert!(!d.fallthrough.contains_key(&0x8000));
        assert!(d.calls_without_return.is_empty());
    }

    #[test]
    fn plp_restores_the_widths_saved_by_php() {
        // 8000: PHP ; REP #$20 ; 8003: PLP ; 8004: NOP ; RTS
        let rom = test_rom(&[(0x8000, &[0x08, 0xC2, 0x20, 0x28, 0xEA, 0x60])]);
        let d = discover(&rom, &Config::default(), &[(0x8000, Flags::RESET)]);
        assert_eq!(d.flags[&0x8003], Flags { m: false, x: true });
        assert_eq!(d.flags[&0x8004], Flags::RESET);
        assert!(d.untracked_plp.is_empty());
    }

    #[test]
    fn nested_php_plp_pairs_restore_in_order() {
        // PHP ; REP #$20 ; PHP ; SEP #$30 ; PLP ; 8007: NOP ; PLP ; 8009: NOP ; RTS
        let rom = test_rom(&[(
            0x8000,
            &[0x08, 0xC2, 0x20, 0x08, 0xE2, 0x30, 0x28, 0xEA, 0x28, 0xEA, 0x60],
        )]);
        let d = discover(&rom, &Config::default(), &[(0x8000, Flags::RESET)]);
        assert_eq!(d.flags[&0x8007], Flags { m: false, x: true });
        assert_eq!(d.flags[&0x8009], Flags::RESET);
        assert!(d.untracked_plp.is_empty());
    }

    #[test]
    fn other_stack_instructions_between_php_and_plp_are_matched() {
        // PHP ; REP #$20 ; PHA ; PLA ; PLP ; 8006: NOP ; RTS
        let rom = test_rom(&[(0x8000, &[0x08, 0xC2, 0x20, 0x48, 0x68, 0x28, 0xEA, 0x60])]);
        let d = discover(&rom, &Config::default(), &[(0x8000, Flags::RESET)]);
        assert_eq!(d.flags[&0x8006], Flags::RESET);
        assert!(d.untracked_plp.is_empty());
    }

    #[test]
    fn unpaired_plp_keeps_widths_and_can_be_corrected_with_assume() {
        // 8000: REP #$20 ; 8002: PLP ; 8003: NOP ; RTS
        let rom = test_rom(&[(0x8000, &[0xC2, 0x20, 0x28, 0xEA, 0x60])]);
        let d = discover(&rom, &Config::default(), &[(0x8000, Flags::RESET)]);
        assert_eq!(d.flags[&0x8003], Flags { m: false, x: true });
        assert_eq!(d.untracked_plp, BTreeSet::from([0x8002]));

        let cfg = width_test_cfg("[[assume]]
addr=\"00:8003\"
m=8
x=8");
        let d = discover(&rom, &cfg, &[(0x8000, Flags::RESET)]);
        assert_eq!(d.flags[&0x8003], Flags::RESET);
        assert_eq!(d.untracked_plp, BTreeSet::from([0x8002]));
    }
}
