//! Basic blocks and functions, built from the result of code discovery.

use crate::disasm::Flow;
use crate::discover::{Discovery, EdgeKind};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SuccKind {
    Fall,
    Branch,
    Jump,
    Dispatch,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub start: u32,
    /// Addresses of the instructions in execution order.
    pub instrs: Vec<u32>,
    pub succs: Vec<(u32, SuccKind)>,
    /// Functions called from inside the block.
    pub calls: Vec<u32>,
    pub ret: bool,
}

impl Block {
    pub fn end(&self) -> u32 {
        *self.instrs.last().expect("a block has at least one instruction")
    }
}

#[derive(Debug, Clone)]
pub struct Function {
    pub entry: u32,
    pub blocks: BTreeSet<u32>,
    pub calls: BTreeSet<u32>,
    /// Other functions entered by jumping or falling into them.
    pub tail_calls: BTreeSet<u32>,
    pub returns: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Diagnostic {
    /// `to` is not a discovered instruction.
    MissingTarget { from: u32, to: u32 },
    /// `block` is reachable from `other` but belongs to `owner`.
    SharedBlock { block: u32, owner: u32, other: u32 },
}

#[derive(Debug, Default)]
pub struct Graph {
    pub blocks: BTreeMap<u32, Block>,
    pub functions: BTreeMap<u32, Function>,
    /// Function that owns each block.
    pub block_owner: BTreeMap<u32, u32>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Graph {
    /// `roots` are the configured entry points; call and dispatch targets become entries too.
    pub fn build(d: &Discovery, roots: &[u32]) -> Graph {
        let mut g = Graph::default();
        let entries = collect_entries(d, roots, &mut g.diagnostics);
        let leaders = collect_leaders(d, &entries);
        for &start in &leaders {
            let block = build_block(d, start, &leaders, &mut g.diagnostics);
            g.blocks.insert(start, block);
        }
        group_functions(&mut g, &entries);
        g
    }

    /// Block that contains the instruction at `addr`.
    pub fn block_at(&self, addr: u32) -> Option<&Block> {
        let (_, b) = self.blocks.range(..=addr).next_back()?;
        b.instrs.contains(&addr).then_some(b)
    }

    /// Function that owns the block containing `addr`.
    pub fn function_at(&self, addr: u32) -> Option<&Function> {
        let owner = self.block_owner.get(&self.block_at(addr)?.start)?;
        self.functions.get(owner)
    }
}

fn collect_entries(d: &Discovery, roots: &[u32], diags: &mut Vec<Diagnostic>) -> BTreeSet<u32> {
    let mut entries = BTreeSet::new();
    for &r in roots {
        if d.instrs.contains_key(&r) {
            entries.insert(r);
        } else {
            diags.push(Diagnostic::MissingTarget { from: r, to: r });
        }
    }
    for e in d.edges.values().flatten() {
        if matches!(e.kind, EdgeKind::Call | EdgeKind::Dispatch) && d.instrs.contains_key(&e.to) {
            entries.insert(e.to);
        }
    }
    entries
}

fn collect_leaders(d: &Discovery, entries: &BTreeSet<u32>) -> BTreeSet<u32> {
    let mut leaders = entries.clone();
    for e in d.edges.values().flatten() {
        if d.instrs.contains_key(&e.to) {
            leaders.insert(e.to);
        }
    }
    // A conditional branch ends its block, so what follows it starts another.
    for (site, &next) in &d.fallthrough {
        if matches!(d.instrs[site].flow(), Flow::Branch(_)) && d.instrs.contains_key(&next) {
            leaders.insert(next);
        }
    }
    // Two different instructions falling into the same address would otherwise share its tail.
    let mut falls_into: BTreeMap<u32, usize> = BTreeMap::new();
    for &next in d.fallthrough.values() {
        *falls_into.entry(next).or_default() += 1;
    }
    leaders.extend(falls_into.into_iter().filter(|&(a, n)| n > 1 && d.instrs.contains_key(&a)).map(|(a, _)| a));
    leaders
}

fn build_block(d: &Discovery, start: u32, leaders: &BTreeSet<u32>, diags: &mut Vec<Diagnostic>) -> Block {
    let mut b = Block { start, instrs: Vec::new(), succs: Vec::new(), calls: Vec::new(), ret: false };
    let mut cur = start;
    loop {
        b.instrs.push(cur);
        let flow = d.instrs[&cur].flow();
        let fall = d.fallthrough.get(&cur).copied();
        for e in d.edges.get(&cur).into_iter().flatten() {
            if !d.instrs.contains_key(&e.to) {
                diags.push(Diagnostic::MissingTarget { from: start, to: e.to });
                continue;
            }
            match e.kind {
                EdgeKind::Branch => b.succs.push((e.to, SuccKind::Branch)),
                EdgeKind::Jump => b.succs.push((e.to, SuccKind::Jump)),
                EdgeKind::Call => b.calls.push(e.to),
                // Dispatch that does not come back to the next instruction is a jump to the target.
                EdgeKind::Dispatch if fall.is_some() => b.calls.push(e.to),
                EdgeKind::Dispatch => b.succs.push((e.to, SuccKind::Dispatch)),
            }
        }
        b.ret |= flow == Flow::Return && b.succs.iter().all(|s| s.1 != SuccKind::Dispatch);
        let Some(next) = fall else { break };
        if !d.instrs.contains_key(&next) {
            diags.push(Diagnostic::MissingTarget { from: start, to: next });
            break;
        }
        if matches!(flow, Flow::Branch(_)) || leaders.contains(&next) {
            b.succs.push((next, SuccKind::Fall));
            break;
        }
        cur = next;
    }
    b.succs.sort();
    b.succs.dedup();
    b.calls.sort();
    b.calls.dedup();
    b
}

fn group_functions(g: &mut Graph, entries: &BTreeSet<u32>) {
    // Entries go in address order, so a block shared between functions goes to the lowest one.
    for &entry in entries {
        let mut f = Function {
            entry,
            blocks: BTreeSet::new(),
            calls: BTreeSet::new(),
            tail_calls: BTreeSet::new(),
            returns: 0,
        };
        let mut stack = vec![entry];
        let mut reported = BTreeSet::new();
        while let Some(at) = stack.pop() {
            match g.block_owner.get(&at) {
                Some(&owner) if owner == entry => continue,
                Some(&owner) => {
                    // Several paths can lead into the same shared block; say it once per function.
                    if reported.insert(at) {
                        g.diagnostics.push(Diagnostic::SharedBlock { block: at, owner, other: entry });
                    }
                    continue;
                }
                None => {}
            }
            g.block_owner.insert(at, entry);
            f.blocks.insert(at);
            let b = &g.blocks[&at];
            f.calls.extend(&b.calls);
            f.returns += b.ret as usize;
            for &(to, _) in &b.succs {
                if to != entry && entries.contains(&to) {
                    f.tail_calls.insert(to);
                } else {
                    stack.push(to);
                }
            }
        }
        g.functions.insert(entry, f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::disasm::Flags;
    use crate::discover::discover;
    use crate::rom::test_rom;

    fn graph(rom: &crate::rom::Rom, cfg: &str, roots: &[u32]) -> Graph {
        let cfg = Config::parse(cfg).unwrap();
        let r: Vec<_> = roots.iter().map(|&a| (a, Flags::RESET)).collect();
        Graph::build(&discover(rom, &cfg, &r), roots)
    }

    fn succs(g: &Graph, block: u32) -> Vec<(u32, SuccKind)> {
        g.blocks[&block].succs.clone()
    }

    #[test]
    fn conditional_branch_splits_blocks() {
        // 8000: BNE $8003 ; 8002: NOP ; 8003: RTS
        let rom = test_rom(&[(0x8000, &[0xD0, 0x01, 0xEA, 0x60])]);
        let g = graph(&rom, "", &[0x8000]);
        assert_eq!(g.blocks.keys().copied().collect::<Vec<_>>(), [0x8000, 0x8002, 0x8003]);
        assert_eq!(succs(&g, 0x8000), [(0x8002, SuccKind::Fall), (0x8003, SuccKind::Branch)]);
        assert_eq!(succs(&g, 0x8002), [(0x8003, SuccKind::Fall)]);
    }

    #[test]
    fn jump_target_in_the_middle_splits_a_sequence() {
        // 8000: NOP ; 8001: NOP ; 8002: BNE $8001 ; 8004: RTS
        let rom = test_rom(&[(0x8000, &[0xEA, 0xEA, 0xD0, 0xFD, 0x60])]);
        let g = graph(&rom, "", &[0x8000]);
        assert_eq!(g.blocks.keys().copied().collect::<Vec<_>>(), [0x8000, 0x8001, 0x8004]);
        assert_eq!(succs(&g, 0x8000), [(0x8001, SuccKind::Fall)]);
        assert_eq!(g.blocks[&0x8001].instrs, [0x8001, 0x8002]);
        assert_eq!(succs(&g, 0x8001), [(0x8001, SuccKind::Branch), (0x8004, SuccKind::Fall)]);
    }

    #[test]
    fn return_has_no_successors() {
        let rom = test_rom(&[(0x8000, &[0xEA, 0x60])]);
        let g = graph(&rom, "", &[0x8000]);
        let b = &g.blocks[&0x8000];
        assert!(b.ret && b.succs.is_empty());
        assert_eq!(b.end(), 0x8001);
    }

    #[test]
    fn call_falls_to_next_instruction() {
        // 8000: JSR $8010 ; 8003: RTS ; 8010: RTS
        let rom = test_rom(&[(0x8000, &[0x20, 0x10, 0x80, 0x60]), (0x8010, &[0x60])]);
        let g = graph(&rom, "", &[0x8000]);
        assert_eq!(g.functions.keys().copied().collect::<Vec<_>>(), [0x8000, 0x8010]);
        let b = &g.blocks[&0x8000];
        assert_eq!(b.instrs, [0x8000, 0x8003]);
        assert_eq!(b.calls, [0x8010]);
        assert_eq!(g.functions[&0x8000].calls, BTreeSet::from([0x8010]));
        assert!(g.functions[&0x8000].blocks == BTreeSet::from([0x8000]));
    }

    #[test]
    fn function_with_two_returns() {
        // 8000: BNE $8003 ; 8002: RTS ; 8003: RTS
        let rom = test_rom(&[(0x8000, &[0xD0, 0x01, 0x60, 0x60])]);
        let g = graph(&rom, "", &[0x8000]);
        let f = &g.functions[&0x8000];
        assert_eq!(f.returns, 2);
        assert_eq!(f.blocks.len(), 3);
    }

    #[test]
    fn jump_to_other_function_is_a_tail_call() {
        // 8000: JMP $8010 ; 8010: RTS (both are entries)
        let rom = test_rom(&[(0x8000, &[0x4C, 0x10, 0x80]), (0x8010, &[0x60])]);
        let g = graph(&rom, "", &[0x8000, 0x8010]);
        let f = &g.functions[&0x8000];
        assert_eq!(f.tail_calls, BTreeSet::from([0x8010]));
        assert_eq!(f.blocks, BTreeSet::from([0x8000]));
        assert!(g.diagnostics.is_empty());
    }

    #[test]
    fn indirect_table_targets_are_function_entries() {
        let rom = test_rom(&[
            (0x8000, &[0x7C, 0x00, 0x81]),
            (0x8100, &[0x10, 0x80, 0x20, 0x80]),
            (0x8010, &[0x60]),
            (0x8020, &[0x60]),
        ]);
        let g = graph(&rom, "[[table]]\nkind=\"indirect\"\nsite=\"00:8000\"\nat=\"00:8100\"\ncount=2", &[0x8000]);
        assert_eq!(succs(&g, 0x8000), [(0x8010, SuccKind::Dispatch), (0x8020, SuccKind::Dispatch)]);
        assert_eq!(g.functions.keys().copied().collect::<Vec<_>>(), [0x8000, 0x8010, 0x8020]);
    }

    #[test]
    fn inline_table_without_fallthrough_does_not_fall_into_the_table() {
        // 8000: JSR $8200 ; inline: $8010, $8020
        let rom = test_rom(&[
            (0x8000, &[0x20, 0x00, 0x82, 0x10, 0x80, 0x20, 0x80]),
            (0x8200, &[0x60]),
            (0x8010, &[0x60]),
            (0x8020, &[0x60]),
        ]);
        let g = graph(&rom, "[[table]]\nkind=\"inline\"\nsite=\"00:8000\"\ncount=2", &[0x8000]);
        let b = &g.blocks[&0x8000];
        assert_eq!(b.instrs, [0x8000]);
        assert_eq!(b.succs, [(0x8010, SuccKind::Dispatch), (0x8020, SuccKind::Dispatch)]);
        assert_eq!(b.calls, [0x8200]);
        assert!(g.diagnostics.is_empty());
    }

    #[test]
    fn inline_helper_targets_are_entries() {
        // 8000: JSR $8200 (helper) ; inline: $8010
        let rom = test_rom(&[
            (0x8000, &[0x20, 0x00, 0x82, 0x10, 0x80]),
            (0x8200, &[0x60]),
            (0x8010, &[0x60]),
        ]);
        let g = graph(&rom, "[[inline_helper]]\naddr=\"00:8200\"", &[0x8000]);
        assert_eq!(g.blocks[&0x8000].instrs, [0x8000]);
        assert!(g.functions.contains_key(&0x8010));
        assert!(g.functions[&0x8000].tail_calls.contains(&0x8010));
    }

    #[test]
    fn inline_table_with_fallthrough_continues_in_the_block() {
        // 8000: JSR $8200 ; inline: $8010 ; 8005: RTS
        let rom = test_rom(&[
            (0x8000, &[0x20, 0x00, 0x82, 0x10, 0x80, 0x60]),
            (0x8200, &[0x60]),
            (0x8010, &[0x60]),
        ]);
        let g = graph(&rom, "[[table]]\nkind=\"inline\"\nsite=\"00:8000\"\ncount=1\nfalls_through=true", &[0x8000]);
        let b = &g.blocks[&0x8000];
        assert_eq!(b.instrs, [0x8000, 0x8005]);
        assert_eq!(b.calls, [0x8010, 0x8200]);
    }

    #[test]
    fn undiscovered_target_is_reported() {
        // 8000: JMP $4000, which is not mapped to the ROM
        let rom = test_rom(&[(0x8000, &[0x4C, 0x00, 0x40])]);
        let g = graph(&rom, "", &[0x8000]);
        assert_eq!(g.diagnostics, [Diagnostic::MissingTarget { from: 0x8000, to: 0x4000 }]);
        assert_eq!(g.blocks.len(), 1);
    }

    #[test]
    fn shared_block_goes_to_the_lowest_function() {
        // 8000: JMP $8100 ; 8010: JMP $8100 ; 8100: RTS
        let rom = test_rom(&[
            (0x8000, &[0x4C, 0x00, 0x81]),
            (0x8010, &[0x4C, 0x00, 0x81]),
            (0x8100, &[0x60]),
        ]);
        let g = graph(&rom, "", &[0x8000, 0x8010]);
        assert_eq!(g.block_owner[&0x8100], 0x8000);
        assert_eq!(g.functions[&0x8010].blocks, BTreeSet::from([0x8010]));
        assert_eq!(g.diagnostics, [Diagnostic::SharedBlock { block: 0x8100, owner: 0x8000, other: 0x8010 }]);
        assert_eq!(g.function_at(0x8100).unwrap().entry, 0x8000);
    }
}
