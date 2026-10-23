//! C code generation: one C function per function of the graph, one label per basic block.

use crate::disasm::{fmt_addr, Flags, Flow, Instr};
use crate::discover::{Discovery, EdgeKind};
use crate::graph::{Block, Graph, SuccKind};
use crate::opcodes::Mode;
use std::collections::{BTreeSet, VecDeque};

/// Interface between the generated code and the runtime, written next to the generated C.
pub const RUNTIME_HEADER: &str = include_str!("../runtime/snes_rt.h");

pub fn func_name(entry: u32) -> String {
    format!("f_{:02X}_{:04X}", entry >> 16, entry & 0xFFFF)
}

fn label(addr: u32) -> String {
    format!("L_{:02X}_{:04X}", addr >> 16, addr & 0xFFFF)
}

/// Effective address of a memory operand: a 24-bit C expression, and whether 16-bit accesses
/// wrap inside bank 0 (direct page and stack) instead of carrying on into the next bank.
struct Ea {
    expr: String,
    bank0: bool,
}

const DB: &str = "((uint32_t)c->db << 16)";

fn ea(ins: &Instr) -> Result<Ea, String> {
    let o = ins.operand;
    let dp = |extra: &str| format!("(uint16_t)(c->d + 0x{o:02X}{extra})");
    let sr = format!("(uint16_t)(c->s + 0x{o:02X})");
    let (expr, bank0) = match ins.mode {
        Mode::Dp => (dp(""), true),
        Mode::DpX => (dp(" + c->x"), true),
        Mode::DpY => (dp(" + c->y"), true),
        Mode::DpInd => (format!("({DB} | bus_read16_b0({}))", dp("")), false),
        Mode::DpIndX => (format!("({DB} | bus_read16_b0({}))", dp(" + c->x")), false),
        Mode::DpIndY => (format!("((({DB} | bus_read16_b0({})) + c->y) & 0xFFFFFF)", dp("")), false),
        Mode::DpIndLong => (format!("bus_read24_b0({})", dp("")), false),
        Mode::DpIndLongY => (format!("((bus_read24_b0({}) + c->y) & 0xFFFFFF)", dp("")), false),
        Mode::Sr => (sr, true),
        Mode::SrIndY => (format!("((({DB} | bus_read16_b0({sr})) + c->y) & 0xFFFFFF)"), false),
        Mode::Abs => (format!("({DB} | 0x{o:04X})"), false),
        Mode::AbsX => (format!("((({DB} | 0x{o:04X}) + c->x) & 0xFFFFFF)"), false),
        Mode::AbsY => (format!("((({DB} | 0x{o:04X}) + c->y) & 0xFFFFFF)"), false),
        Mode::Long => (format!("0x{o:06X}"), false),
        Mode::LongX => (format!("((0x{o:06X} + c->x) & 0xFFFFFF)"), false),
        m => return Err(format!("{}: addressing mode {m:?} has no effective address", fmt_addr(ins.addr))),
    };
    Ok(Ea { expr, bank0 })
}

fn read(ea: &Ea, w8: bool) -> String {
    match (w8, ea.bank0) {
        (true, _) => format!("bus_read8({})", ea.expr),
        (false, false) => format!("bus_read16({})", ea.expr),
        (false, true) => format!("bus_read16_b0({})", ea.expr),
    }
}

fn write(ea: &Ea, w8: bool, value: &str) -> String {
    match (w8, ea.bank0) {
        (true, _) => format!("bus_write8({}, {value});", ea.expr),
        (false, false) => format!("bus_write16({}, {value});", ea.expr),
        (false, true) => format!("bus_write16_b0({}, {value});", ea.expr),
    }
}

fn width(w8: bool) -> &'static str {
    if w8 { "8" } else { "16" }
}

fn ty(w8: bool) -> &'static str {
    if w8 { "uint8_t" } else { "uint16_t" }
}

/// Value an ALU-style instruction works with: the immediate, or what the operand address holds.
fn operand_value(ins: &Instr, w8: bool) -> Result<String, String> {
    match ins.mode {
        Mode::ImmM | Mode::ImmX => Ok(format!("0x{:0w$X}", ins.operand, w = (ins.len as usize - 1) * 2)),
        _ => Ok(read(&ea(ins)?, w8)),
    }
}

/// `reg` as a value of the given width.
fn reg(name: &str, w8: bool) -> String {
    if w8 { format!("(uint8_t)c->{name}") } else { format!("c->{name}") }
}

/// Assigns the low byte of the accumulator and keeps the high one (8-bit mode), or all of it.
fn set_a(value: &str, m8: bool) -> String {
    if m8 {
        format!("c->a = (uint16_t)((c->a & 0xFF00) | {value});")
    } else {
        format!("c->a = {value};")
    }
}

fn transfer(dst: &str, src: &str, w8: bool) -> Vec<String> {
    if dst == "a" && w8 {
        vec![set_a(&format!("(uint8_t)c->{src}"), true), "rt_nz8(c, (uint8_t)c->a);".into()]
    } else if w8 {
        vec![format!("c->{dst} = (uint8_t)c->{src};"), format!("rt_nz8(c, (uint8_t)c->{dst});")]
    } else {
        vec![format!("c->{dst} = c->{src};"), format!("rt_nz16(c, c->{dst});")]
    }
}

/// Where a branch, jump or call lands at run time, for the indirect forms.
fn indirect_target(ins: &Instr) -> Result<String, String> {
    let bank = ins.addr & 0xFF_0000;
    let o = ins.operand;
    match (ins.mnemonic, ins.mode) {
        ("JMP", Mode::AbsInd) => Ok(format!("(0x{bank:06X} | bus_read16_b0(0x{o:04X}))")),
        ("JMP" | "JSR", Mode::AbsIndX) => {
            Ok(format!("(0x{bank:06X} | bus_read16(0x{bank:06X} | (uint16_t)(0x{o:04X} + c->x)))"))
        }
        ("JML", Mode::AbsIndLong) => Ok(format!("bus_read24_b0(0x{o:04X})")),
        _ => Err(format!("{}: unsupported indirect {} {:?}", fmt_addr(ins.addr), ins.mnemonic, ins.mode)),
    }
}

fn branch_condition(mnemonic: &str) -> Option<&'static str> {
    Some(match mnemonic {
        "BPL" => "!c->n",
        "BMI" => "c->n",
        "BVC" => "!c->v",
        "BVS" => "c->v",
        "BCC" => "!c->c",
        "BCS" => "c->c",
        "BNE" => "!c->z",
        "BEQ" => "c->z",
        _ => return None,
    })
}

/// Address of the instruction after `ins`, within its bank.
fn after(ins: &Instr) -> u32 {
    (ins.addr + ins.len as u32) & 0xFFFF
}

/// C for every instruction that does not decide where execution goes next, plus calls.
/// `targets` are the known destinations of an indirect call. Instructions that never
/// continue (`BRK`, `COP`, `STP`) end with a `return`.
fn instr_lines(ins: &Instr, f: Flags, targets: &[u32]) -> Result<Vec<String>, String> {
    let (m8, x8) = (f.m, f.x);
    let site = ins.addr;
    let o = ins.operand;
    let mn = ins.mnemonic;
    let lines: Vec<String> = match mn {
        "ORA" | "AND" | "EOR" | "ADC" | "SBC" | "LDA" => {
            let op = mn.to_lowercase();
            vec![format!("rt_{op}{}(c, {});", width(m8), operand_value(ins, m8)?)]
        }
        "BIT" => {
            let imm = if ins.mode == Mode::ImmM { "_imm" } else { "" };
            vec![format!("rt_bit{imm}{}(c, {});", width(m8), operand_value(ins, m8)?)]
        }
        "CMP" => vec![format!("rt_cmp{}(c, {}, {});", width(m8), reg("a", m8), operand_value(ins, m8)?)],
        "CPX" | "CPY" => {
            let r = if mn == "CPX" { "x" } else { "y" };
            vec![format!("rt_cmp{}(c, {}, {});", width(x8), reg(r, x8), operand_value(ins, x8)?)]
        }
        "LDX" | "LDY" => {
            let r = mn[2..].to_lowercase();
            vec![format!("rt_ld{r}{}(c, {});", width(x8), operand_value(ins, x8)?)]
        }
        "STA" => vec![write(&ea(ins)?, m8, &reg("a", m8))],
        "STZ" => vec![write(&ea(ins)?, m8, "0")],
        "STX" => vec![write(&ea(ins)?, x8, &reg("x", x8))],
        "STY" => vec![write(&ea(ins)?, x8, &reg("y", x8))],
        "ASL" | "LSR" | "ROL" | "ROR" | "INC" | "DEC" | "TSB" | "TRB" => {
            let op = mn.to_lowercase();
            if ins.mode == Mode::Acc {
                let call = format!("rt_{op}{}(c, {})", width(m8), reg("a", m8));
                vec![if m8 { set_a(&call, true) } else { format!("c->a = {call};") }]
            } else {
                let e = ea(ins)?;
                let t = ty(m8);
                vec![
                    "{".into(),
                    format!("    uint32_t ea = {};", e.expr),
                    format!("    {t} v = {};", read(&Ea { expr: "ea".into(), bank0: e.bank0 }, m8)),
                    format!("    v = rt_{op}{}(c, v);", width(m8)),
                    format!("    {}", write(&Ea { expr: "ea".into(), bank0: e.bank0 }, m8, "v")),
                    "}".into(),
                ]
            }
        }
        "INX" | "INY" | "DEX" | "DEY" => {
            let r = mn[2..].to_lowercase();
            let sign = if mn.starts_with("IN") { "+" } else { "-" };
            let (cast, nz) = if x8 { ("uint8_t", "8") } else { ("uint16_t", "16") };
            vec![format!("c->{r} = ({cast})(c->{r} {sign} 1);"), format!("rt_nz{nz}(c, ({cast})c->{r});")]
        }
        "TAX" => transfer("x", "a", x8),
        "TAY" => transfer("y", "a", x8),
        "TXA" => transfer("a", "x", m8),
        "TYA" => transfer("a", "y", m8),
        "TXY" => transfer("y", "x", x8),
        "TYX" => transfer("x", "y", x8),
        "TSX" => transfer("x", "s", x8),
        "TCD" => transfer("d", "a", false),
        "TDC" => transfer("a", "d", false),
        "TSC" => transfer("a", "s", false),
        "TXS" => vec!["c->s = c->e ? (uint16_t)(0x100 | (c->x & 0xFF)) : c->x;".into()],
        "TCS" => vec!["c->s = c->e ? (uint16_t)(0x100 | (c->a & 0xFF)) : c->a;".into()],
        "XBA" => vec![
            "c->a = (uint16_t)((c->a << 8) | (c->a >> 8));".into(),
            "rt_nz8(c, (uint8_t)c->a);".into(),
        ],
        "XCE" => vec!["rt_xce(c);".into()],
        "PHA" => vec![format!("rt_push{}(c, {});", width(m8), reg("a", m8))],
        "PHX" => vec![format!("rt_push{}(c, {});", width(x8), reg("x", x8))],
        "PHY" => vec![format!("rt_push{}(c, {});", width(x8), reg("y", x8))],
        "PLA" => vec![format!("rt_lda{0}(c, rt_pull{0}(c));", width(m8))],
        "PLX" => vec![format!("rt_ldx{0}(c, rt_pull{0}(c));", width(x8))],
        "PLY" => vec![format!("rt_ldy{0}(c, rt_pull{0}(c));", width(x8))],
        "PHP" => vec!["rt_push8(c, rt_get_p(c));".into()],
        "PLP" => vec!["rt_set_p(c, rt_pull8(c));".into()],
        "PHB" => vec!["rt_push8(c, c->db);".into()],
        "PLB" => vec!["c->db = rt_pull8(c);".into(), "rt_nz8(c, c->db);".into()],
        "PHD" => vec!["rt_push16(c, c->d);".into()],
        "PLD" => vec!["c->d = rt_pull16(c);".into(), "rt_nz16(c, c->d);".into()],
        "PHK" => vec![format!("rt_push8(c, 0x{:02X});", ins.addr >> 16)],
        "PEA" => vec![format!("rt_push16(c, 0x{o:04X});")],
        "PEI" => vec![format!("rt_push16(c, bus_read16_b0((uint16_t)(c->d + 0x{o:02X})));")],
        "PER" => vec![format!("rt_push16(c, 0x{:04X});", ins.rel_target() & 0xFFFF)],
        "CLC" => vec!["c->c = 0;".into()],
        "SEC" => vec!["c->c = 1;".into()],
        "CLI" => vec!["c->irq = 0;".into()],
        "SEI" => vec!["c->irq = 1;".into()],
        "CLD" => vec!["c->dec = 0;".into()],
        "SED" => vec!["c->dec = 1;".into()],
        "CLV" => vec!["c->v = 0;".into()],
        "REP" => vec![format!("rt_rep(c, 0x{o:02X});")],
        "SEP" => vec![format!("rt_sep(c, 0x{o:02X});")],
        "NOP" => vec![],
        "WDM" => vec!["rt_wdm(c);".into()],
        "WAI" => vec!["rt_wai(c);".into()],
        "STP" => vec!["rt_stp(c);".into(), "return;".into()],
        "BRK" => vec!["rt_brk(c);".into(), "return;".into()],
        "COP" => vec!["rt_cop(c);".into(), "return;".into()],
        "MVN" | "MVP" => {
            let dir = if mn == "MVN" { 1 } else { -1 };
            vec![format!("rt_block_move(c, 0x{:02X}, 0x{:02X}, {dir});", o & 0xFF, o >> 8)]
        }
        "JSR" | "JSL" => call_lines(ins, targets)?,
        _ => return Err(format!("{}: {} has no translation", fmt_addr(site), ins.text())),
    };
    Ok(lines)
}

/// Pushes the return address the way the CPU does and calls the target function(s).
fn call_lines(ins: &Instr, targets: &[u32]) -> Result<Vec<String>, String> {
    let ret = after(ins).wrapping_sub(1) & 0xFFFF;
    let mut push = Vec::new();
    if ins.mnemonic == "JSL" {
        push.push(format!("rt_push8(c, 0x{:02X});", ins.addr >> 16));
    }
    push.push(format!("rt_push16(c, 0x{ret:04X});"));
    match ins.flow() {
        Flow::Call(t) => {
            push.push(format!("{}(c);", func_name(t)));
            Ok(push)
        }
        Flow::IndirectCall => {
            let mut out = vec!["{".to_string(), format!("    uint32_t t = {};", indirect_target(ins)?)];
            out.extend(push.into_iter().map(|l| format!("    {l}")));
            out.push("    switch (t) {".into());
            for &t in targets {
                out.push(format!("    case 0x{t:06X}: {}(c); break;", func_name(t)));
            }
            out.push(format!("    default: rt_unresolved(c, 0x{:06X}, t); break;", ins.addr));
            out.push("    }".into());
            out.push("}".into());
            Ok(out)
        }
        _ => Err(format!("{}: {} is not a call", fmt_addr(ins.addr), ins.text())),
    }
}

/// Emits the functions of a graph.
pub struct Emitter<'a> {
    d: &'a Discovery,
    g: &'a Graph,
}

/// A function body under construction: the blocks it owns and what it refers to.
struct Body {
    entry: u32,
    blocks: BTreeSet<u32>,
    used_labels: BTreeSet<u32>,
    refs: BTreeSet<u32>,
}

impl<'a> Emitter<'a> {
    pub fn new(d: &'a Discovery, g: &'a Graph) -> Self {
        Emitter { d, g }
    }

    /// Blocks reachable from `entry` without entering another function. Unlike the graph's own
    /// grouping, a block shared with another function is included here too (it is emitted twice).
    fn reachable_blocks(&self, entry: u32) -> BTreeSet<u32> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![entry];
        while let Some(at) = stack.pop() {
            if !seen.insert(at) {
                continue;
            }
            for &(to, _) in &self.g.blocks[&at].succs {
                if to == entry || !self.g.functions.contains_key(&to) {
                    stack.push(to);
                }
            }
        }
        seen
    }

    /// Whether the C for `a`, or its length, would differ among the widths it is reached with.
    fn depends_on_widths(&self, a: u32, set: &BTreeSet<Flags>) -> bool {
        let ins = &self.d.instrs[&a];
        let varies = |pick: fn(&Flags) -> bool| set.iter().map(pick).collect::<BTreeSet<_>>().len() > 1;
        match ins.mode {
            Mode::ImmM if varies(|f| f.m) => return true,
            Mode::ImmX if varies(|f| f.x) => return true,
            _ => {}
        }
        let mut lines = set.iter().map(|&f| instr_lines(ins, f, &[]));
        let first = lines.next();
        lines.any(|l| Some(l) != first)
    }

    fn check_widths(&self, blocks: &BTreeSet<u32>) -> Result<(), String> {
        let mut problems = Vec::new();
        for start in blocks {
            for a in &self.g.blocks[start].instrs {
                if let Some(set) = self.d.flag_conflicts.get(a) {
                    if !self.depends_on_widths(*a, set) {
                        continue;
                    }
                    let seen: Vec<String> = set
                        .iter()
                        .map(|f| format!("M={} X={}", if f.m { 8 } else { 16 }, if f.x { 8 } else { 16 }))
                        .collect();
                    problems.push(format!("{} is reached with different register widths ({})", fmt_addr(*a), seen.join(" / ")));
                }
            }
        }
        if problems.is_empty() { Ok(()) } else { Err(problems.join("
")) }
    }

    /// Transfers control to `target`: a `goto` inside the function, or a tail call.
    fn jump(&self, body: &mut Body, target: u32) -> Result<String, String> {
        if body.blocks.contains(&target) {
            body.used_labels.insert(target);
            Ok(format!("goto {};", label(target)))
        } else if self.g.functions.contains_key(&target) {
            body.refs.insert(target);
            Ok(format!("{}(c); return;", func_name(target)))
        } else {
            Err(format!("{} jumps to {}, which is not code", func_name(body.entry), fmt_addr(target)))
        }
    }

    fn block_lines(&self, body: &mut Body, block: &Block, next: Option<u32>) -> Result<Vec<String>, String> {
        let flags = self.d.flags[&block.start];
        // A width that differs among the ways the block is entered (2) is not checked.
        let varies = |pick: fn(&Flags) -> bool| {
            self.d.flag_conflicts.get(&block.start).is_some_and(|s| s.iter().any(|f| pick(f) != pick(&flags)))
        };
        let (m, x) = (if varies(|f| f.m) { 2 } else { flags.m as u8 }, if varies(|f| f.x) { 2 } else { flags.x as u8 });
        let mut out = vec![format!("rt_check_flags(c, 0x{:06X}, {}, {});", block.start, m, x)];
        let dispatch: Vec<u32> = block
            .succs
            .iter()
            .filter(|s| s.1 == SuccKind::Dispatch)
            .map(|s| s.0)
            .collect();
        let last_index = block.instrs.len() - 1;
        let mut ends = false;
        for (i, &a) in block.instrs.iter().enumerate() {
            let ins = &self.d.instrs[&a];
            let f = self.d.flags[&a];
            out.push(format!("// {} {}", fmt_addr(a), ins.text()));
            let lines = match ins.flow() {
                Flow::Call(t) => {
                    body.refs.insert(t);
                    instr_lines(ins, f, &[])?
                }
                Flow::IndirectCall => {
                    let targets: Vec<u32> = self
                        .d
                        .edges
                        .get(&a)
                        .into_iter()
                        .flatten()
                        .filter(|e| e.kind == EdgeKind::Dispatch && self.d.instrs.contains_key(&e.to))
                        .map(|e| e.to)
                        .collect();
                    body.refs.extend(&targets);
                    instr_lines(ins, f, &targets)?
                }
                Flow::Branch(t) if i == last_index => {
                    let cond = branch_condition(ins.mnemonic)
                        .ok_or_else(|| format!("{}: unknown branch {}", fmt_addr(a), ins.mnemonic))?;
                    vec![format!("if ({cond}) {{ {} }}", self.jump(body, t)?)]
                }
                Flow::Jump(t) if i == last_index => {
                    ends = true;
                    vec![self.jump(body, t)?]
                }
                Flow::IndirectJump if i == last_index => {
                    ends = true;
                    let mut v = vec!["{".to_string(), format!("    uint32_t t = {};", indirect_target(ins)?), "    switch (t) {".into()];
                    for &t in &dispatch {
                        v.push(format!("    case 0x{t:06X}: {}", self.jump(body, t)?));
                    }
                    v.push(format!("    default: rt_unresolved(c, 0x{a:06X}, t); return;"));
                    v.push("    }".into());
                    v.push("}".into());
                    v
                }
                Flow::Return => {
                    ends = true;
                    match ins.mnemonic {
                        "RTS" => vec!["(void)rt_pull16(c);".into(), "return;".into()],
                        "RTL" => vec!["(void)rt_pull16(c);".into(), "(void)rt_pull8(c);".into(), "return;".into()],
                        _ => vec!["rt_rti(c);".into(), "return;".into()],
                    }
                }
                Flow::Stop => {
                    ends = true;
                    instr_lines(ins, f, &[])?
                }
                _ => instr_lines(ins, f, &[])?,
            };
            out.extend(lines);
        }
        if !ends {
            match block.succs.iter().find(|s| s.1 == SuccKind::Fall) {
                Some(&(to, _)) if Some(to) == next && body.blocks.contains(&to) => {}
                Some(&(to, _)) => out.push(self.jump(body, to)?),
                None => out.push("return;".into()),
            }
        }
        Ok(out)
    }

    /// C for one function, and the functions it refers to.
    pub fn function(&self, entry: u32) -> Result<(String, BTreeSet<u32>), String> {
        if !self.g.functions.contains_key(&entry) {
            return Err(format!("{} is not the entry of a function", fmt_addr(entry)));
        }
        let blocks = self.reachable_blocks(entry);
        self.check_widths(&blocks)?;
        let mut body = Body { entry, blocks: blocks.clone(), used_labels: BTreeSet::new(), refs: BTreeSet::new() };
        let order: Vec<u32> = blocks.iter().copied().collect();
        let mut chunks = Vec::new();
        for (i, &start) in order.iter().enumerate() {
            let lines = self.block_lines(&mut body, &self.g.blocks[&start], order.get(i + 1).copied())?;
            chunks.push((start, lines));
        }
        let mut text = format!("void {}(Cpu *c) {{\n", func_name(entry));
        for (start, lines) in chunks {
            if body.used_labels.contains(&start) {
                text.push_str(&format!("{}:\n", label(start)));
            }
            for l in lines {
                text.push_str(&format!("    {l}\n"));
            }
        }
        text.push_str("}\n");
        Ok((text, body.refs))
    }

    /// The function at `entry` and, up to `depth` levels, the functions it calls. Everything else
    /// that is referenced is declared and left for the linker.
    pub fn generate(&self, entry: u32, depth: usize) -> Result<String, String> {
        let mut emitted: BTreeSet<u32> = BTreeSet::new();
        let mut referenced: BTreeSet<u32> = BTreeSet::new();
        let mut bodies = Vec::new();
        let mut queue = VecDeque::from([(entry, 0usize)]);
        while let Some((at, level)) = queue.pop_front() {
            if !emitted.insert(at) {
                continue;
            }
            let (text, refs) = self.function(at)?;
            bodies.push((at, text));
            referenced.extend(&refs);
            if level < depth {
                queue.extend(refs.into_iter().map(|r| (r, level + 1)));
            }
        }
        bodies.sort_by_key(|b| b.0);
        referenced.extend(&emitted);
        let mut out = String::from("/* Generated by snes-recomp from the ROM: do not commit or distribute. */\n#include \"snes_rt.h\"\n\n");
        for r in &referenced {
            out.push_str(&format!("void {}(Cpu *c);\n", func_name(*r)));
        }
        for (_, text) in bodies {
            out.push('\n');
            out.push_str(&text);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cctest::{Compiler, MOCK_RUNTIME};
    use crate::disasm::decode;
    use crate::rom::test_rom;

    /// Names of runtime symbols (`rt_*`, `bus_*`) mentioned in some generated C.
    fn runtime_symbols(text: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if text[i..].starts_with("rt_") || text[i..].starts_with("bus_") {
                let end = text[i..]
                    .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                    .map_or(text.len(), |n| i + n);
                out.insert(text[i..end].to_string());
                i = end;
            } else {
                i += 1;
            }
        }
        out
    }

    /// C for every opcode that `instr_lines` is responsible for, under all four M/X combinations.
    fn all_opcode_lines() -> Vec<(u8, Flags, Vec<String>)> {
        let mut out = Vec::new();
        for op in 0..=255u8 {
            for (m, x) in [(true, true), (true, false), (false, true), (false, false)] {
                let f = Flags { m, x };
                let rom = test_rom(&[(0x8000, &[op, 0x10, 0x20, 0x30])]);
                let ins = decode(&rom, 0x8000, f).unwrap();
                // Control flow is emitted by the block emitter.
                if matches!(ins.flow(), Flow::Branch(_) | Flow::Jump(_) | Flow::IndirectJump | Flow::Return) {
                    continue;
                }
                let lines = instr_lines(&ins, f, &[]).unwrap_or_else(|e| panic!("opcode {op:02X}: {e}"));
                out.push((op, f, lines));
            }
        }
        out
    }

    #[test]
    fn header_defines_every_symbol_the_emitter_uses() {
        let mut used = BTreeSet::new();
        for (_, _, lines) in all_opcode_lines() {
            used.extend(runtime_symbols(&lines.join("\n")));
        }
        // Used by the block emitter rather than by single instructions.
        for extra in ["rt_check_flags", "rt_unresolved", "rt_rti", "rt_pull8", "rt_pull16", "bus_read24_b0"] {
            used.insert(extra.to_string());
        }
        assert!(used.len() > 40, "suspiciously few symbols: {used:?}");
        for name in used {
            // The shift/increment families are defined once by a macro, as `rt_asl##W`.
            let stem = name.trim_end_matches(|ch: char| ch.is_ascii_digit());
            let defined = RUNTIME_HEADER.contains(&format!("{name}("))
                || RUNTIME_HEADER.contains(&format!("{stem}##W("));
            assert!(defined, "{name} is used by the emitter but not defined in snes_rt.h");
        }
    }

    #[test]
    fn every_opcode_has_a_translation() {
        let covered: BTreeSet<u8> = all_opcode_lines().iter().map(|l| l.0).collect();
        let control: Vec<u8> = (0..=255u8)
            .filter(|op| {
                let rom = test_rom(&[(0x8000, &[*op, 0x10, 0x20, 0x30])]);
                let ins = decode(&rom, 0x8000, Flags::RESET).unwrap();
                matches!(ins.flow(), Flow::Branch(_) | Flow::Jump(_) | Flow::IndirectJump | Flow::Return)
            })
            .collect();
        assert_eq!(covered.len() + control.len(), 256);
    }

    #[test]
    fn runtime_arithmetic_stack_and_flags_behave() {
        let cc: Compiler = crate::need_compiler!();
        let main = r#"
#include "snes_rt.h"
#include <stdio.h>
static Cpu c;
static void reset(int m, int dec) { c = (Cpu){0}; c.m = m; c.xf = 1; c.dec = dec; c.s = 0x01FF; }
#define SHOW(name) printf("%s %04X c=%d z=%d v=%d n=%d\n", name, c.a, c.c, c.z, c.v, c.n)
#define SHOWD(name) printf("%s %04X c=%d z=%d\n", name, c.a, c.c, c.z)
int main(void) {
    reset(1, 0); c.a = 0x12FF; rt_adc8(&c, 1); SHOW("adc8");
    reset(0, 0); c.a = 0x7FFF; rt_adc16(&c, 1); SHOW("adc16");
    reset(1, 0); c.a = 0x50; rt_adc8(&c, 0x50); SHOW("adc8ov");
    reset(1, 1); c.a = 0x19; rt_adc8(&c, 0x28); SHOWD("dec19+28");
    reset(1, 1); c.a = 0x99; rt_adc8(&c, 0x01); SHOWD("dec99+01");
    reset(1, 1); c.a = 0x58; rt_adc8(&c, 0x46); SHOWD("dec58+46");
    reset(0, 1); c.a = 0x1999; rt_adc16(&c, 1); SHOWD("dec1999+1");
    reset(1, 0); c.a = 0x50; c.c = 1; rt_sbc8(&c, 0x30); SHOW("sbc50-30");
    reset(1, 0); c.a = 0x30; c.c = 1; rt_sbc8(&c, 0x50); SHOW("sbc30-50");
    reset(1, 0); c.a = 0x80; c.c = 1; rt_sbc8(&c, 0x01); SHOW("sbc80-01");
    reset(1, 1); c.a = 0x50; c.c = 1; rt_sbc8(&c, 0x19); SHOWD("decsbc50-19");
    reset(1, 1); c.a = 0x00; c.c = 1; rt_sbc8(&c, 0x01); SHOWD("decsbc00-01");
    reset(1, 0); rt_cmp8(&c, 0x10, 0x20); SHOW("cmp10-20");
    reset(1, 0); c.c = 1; c.a = 0x7780; c.a = (uint16_t)((c.a & 0xFF00) | rt_rol8(&c, 0x80)); SHOW("rol8");
    reset(1, 0); c.a = 0x7701; c.a = (uint16_t)((c.a & 0xFF00) | rt_ror8(&c, 0x01)); SHOW("ror8");
    reset(1, 0); c.a = 0x7780; rt_lda8(&c, 0x00); SHOW("lda8");
    reset(1, 0); rt_push16(&c, 0x1234);
    printf("push %02X %02X s=%04X\n", bus_read8(0x1FF), bus_read8(0x1FE), c.s);
    { uint16_t v = rt_pull16(&c); printf("pull %04X s=%04X\n", v, c.s); }
    reset(0, 0); c.e = 1; c.c = 0; rt_xce(&c);
    printf("xce e=%d c=%d m=%d xf=%d\n", c.e, c.c, c.m, c.xf);
    reset(0, 0); c.e = 0; c.c = 1; c.x = 0x1234; c.s = 0x0456; rt_xce(&c);
    printf("xce-emu e=%d c=%d m=%d xf=%d x=%04X s=%04X\n", c.e, c.c, c.m, c.xf, c.x, c.s);
    reset(1, 0); c.xf = 1; c.x = 0x1FF; rt_rep(&c, 0x30);
    printf("rep m=%d xf=%d\n", c.m, c.xf);
    c.x = 0x1234; rt_sep(&c, 0x10);
    printf("sep xf=%d x=%04X p=%02X\n", c.xf, c.x, rt_get_p(&c));
    return 0;
}
"#;
        let out = cc
            .compile_run("rt", &[("snes_rt.h", RUNTIME_HEADER), ("rt.c", MOCK_RUNTIME), ("main.c", main)], &[])
            .unwrap();
        let expected = "\
adc8 1200 c=1 z=1 v=0 n=0
adc16 8000 c=0 z=0 v=1 n=1
adc8ov 00A0 c=0 z=0 v=1 n=1
dec19+28 0047 c=0 z=0
dec99+01 0000 c=1 z=1
dec58+46 0004 c=1 z=0
dec1999+1 2000 c=0 z=0
sbc50-30 0020 c=1 z=0 v=0 n=0
sbc30-50 00E0 c=0 z=0 v=0 n=1
sbc80-01 007F c=1 z=0 v=1 n=0
decsbc50-19 0031 c=1 z=0
decsbc00-01 0099 c=0 z=0
cmp10-20 0000 c=0 z=0 v=0 n=1
rol8 7701 c=1 z=0 v=0 n=0
ror8 7700 c=1 z=1 v=0 n=0
lda8 7700 c=0 z=1 v=0 n=0
push 12 34 s=01FD
pull 1234 s=01FF
xce e=0 c=1 m=0 xf=1
xce-emu e=1 c=0 m=1 xf=1 x=0034 s=0156
rep m=0 xf=0
sep xf=1 x=0034 p=10
";
        assert_eq!(out, expected);
    }

    /// C lines for the instruction encoded by `bytes`, decoded under the given register widths.
    fn lines(bytes: &[u8], m: bool, x: bool) -> Vec<String> {
        let f = Flags { m, x };
        let rom = test_rom(&[(0x8000, bytes)]);
        let ins = decode(&rom, 0x8000, f).unwrap();
        instr_lines(&ins, f, &[]).unwrap()
    }

    #[test]
    fn addressing_modes_build_the_effective_address() {
        let db = "((uint32_t)c->db << 16)";
        let cases: Vec<(&[u8], String)> = vec![
            (&[0xA5, 0x10], "rt_lda8(c, bus_read8((uint16_t)(c->d + 0x10)));".into()),
            (&[0xB5, 0x10], "rt_lda8(c, bus_read8((uint16_t)(c->d + 0x10 + c->x)));".into()),
            (&[0xAD, 0x00, 0x21], format!("rt_lda8(c, bus_read8(({db} | 0x2100)));")),
            (&[0xBD, 0x00, 0x21], format!("rt_lda8(c, bus_read8(((({db} | 0x2100) + c->x) & 0xFFFFFF)));")),
            (&[0xB9, 0x00, 0x21], format!("rt_lda8(c, bus_read8(((({db} | 0x2100) + c->y) & 0xFFFFFF)));")),
            (&[0xAF, 0x34, 0x12, 0x7E], "rt_lda8(c, bus_read8(0x7E1234));".into()),
            (&[0xBF, 0x34, 0x12, 0x7E], "rt_lda8(c, bus_read8(((0x7E1234 + c->x) & 0xFFFFFF)));".into()),
            (&[0xB2, 0x20], format!("rt_lda8(c, bus_read8(({db} | bus_read16_b0((uint16_t)(c->d + 0x20)))));")),
            (&[0xA1, 0x20], format!("rt_lda8(c, bus_read8(({db} | bus_read16_b0((uint16_t)(c->d + 0x20 + c->x)))));")),
            (&[0xB1, 0x20], format!("rt_lda8(c, bus_read8(((({db} | bus_read16_b0((uint16_t)(c->d + 0x20))) + c->y) & 0xFFFFFF)));")),
            (&[0xA7, 0x20], "rt_lda8(c, bus_read8(bus_read24_b0((uint16_t)(c->d + 0x20))));".into()),
            (&[0xB7, 0x20], "rt_lda8(c, bus_read8(((bus_read24_b0((uint16_t)(c->d + 0x20)) + c->y) & 0xFFFFFF)));".into()),
            (&[0xA3, 0x05], "rt_lda8(c, bus_read8((uint16_t)(c->s + 0x05)));".into()),
            (&[0xB3, 0x05], format!("rt_lda8(c, bus_read8(((({db} | bus_read16_b0((uint16_t)(c->s + 0x05))) + c->y) & 0xFFFFFF)));")),
        ];
        for (bytes, want) in cases {
            assert_eq!(lines(bytes, true, true), [want], "opcode {:02X}", bytes[0]);
        }
    }

    #[test]
    fn sixteen_bit_accesses_wrap_in_bank_zero_only_for_direct_page_and_stack() {
        assert_eq!(lines(&[0xA5, 0x10], false, true), ["rt_lda16(c, bus_read16_b0((uint16_t)(c->d + 0x10)));"]);
        assert_eq!(lines(&[0xA3, 0x05], false, true), ["rt_lda16(c, bus_read16_b0((uint16_t)(c->s + 0x05)));"]);
        assert!(lines(&[0xAD, 0x00, 0x21], false, true)[0].starts_with("rt_lda16(c, bus_read16(("));
    }

    #[test]
    fn loads_stores_transfers_and_stack_follow_the_register_widths() {
        assert_eq!(lines(&[0xA9, 0x34, 0x12], false, true), ["rt_lda16(c, 0x1234);"]);
        assert_eq!(lines(&[0xA9, 0x80], true, true), ["rt_lda8(c, 0x80);"]);
        assert_eq!(lines(&[0xA2, 0x34, 0x12], true, false), ["rt_ldx16(c, 0x1234);"]);
        assert_eq!(lines(&[0x85, 0x10], true, true), ["bus_write8((uint16_t)(c->d + 0x10), (uint8_t)c->a);"]);
        assert_eq!(lines(&[0x85, 0x10], false, true), ["bus_write16_b0((uint16_t)(c->d + 0x10), c->a);"]);
        assert_eq!(lines(&[0x64, 0x10], false, true), ["bus_write16_b0((uint16_t)(c->d + 0x10), 0);"]);
        assert_eq!(lines(&[0x86, 0x10], true, false), ["bus_write16_b0((uint16_t)(c->d + 0x10), c->x);"]);
        assert_eq!(lines(&[0xAA], true, true), ["c->x = (uint8_t)c->a;", "rt_nz8(c, (uint8_t)c->x);"]);
        assert_eq!(lines(&[0xAA], true, false), ["c->x = c->a;", "rt_nz16(c, c->x);"]);
        assert_eq!(
            lines(&[0x8A], true, false),
            ["c->a = (uint16_t)((c->a & 0xFF00) | (uint8_t)c->x);", "rt_nz8(c, (uint8_t)c->a);"]
        );
        assert_eq!(lines(&[0x48], false, true), ["rt_push16(c, c->a);"]);
        assert_eq!(lines(&[0x48], true, true), ["rt_push8(c, (uint8_t)c->a);"]);
        assert_eq!(lines(&[0x68], true, true), ["rt_lda8(c, rt_pull8(c));"]);
        assert_eq!(lines(&[0xDA], true, false), ["rt_push16(c, c->x);"]);
        assert_eq!(lines(&[0xEB], true, true).len(), 2);
        assert_eq!(lines(&[0xFB], true, true), ["rt_xce(c);"]);
        assert_eq!(lines(&[0x08], true, true), ["rt_push8(c, rt_get_p(c));"]);
        assert_eq!(lines(&[0x28], true, true), ["rt_set_p(c, rt_pull8(c));"]);
        assert_eq!(lines(&[0x4B], true, true), ["rt_push8(c, 0x00);"]);
        assert_eq!(lines(&[0xF4, 0x34, 0x12], true, true), ["rt_push16(c, 0x1234);"]);
    }

    #[test]
    fn arithmetic_logic_and_shifts_use_the_accumulator_width() {
        assert_eq!(lines(&[0x69, 0x01], true, true), ["rt_adc8(c, 0x01);"]);
        assert_eq!(lines(&[0xE9, 0x01, 0x00], false, true), ["rt_sbc16(c, 0x0001);"]);
        assert_eq!(lines(&[0x29, 0x0F], true, true), ["rt_and8(c, 0x0F);"]);
        assert_eq!(lines(&[0xC9, 0x10], true, true), ["rt_cmp8(c, (uint8_t)c->a, 0x10);"]);
        assert_eq!(lines(&[0xE0, 0x10, 0x00], true, false), ["rt_cmp16(c, c->x, 0x0010);"]);
        assert_eq!(lines(&[0x89, 0x80], true, true), ["rt_bit_imm8(c, 0x80);"]);
        assert_eq!(lines(&[0x24, 0x10], true, true), ["rt_bit8(c, bus_read8((uint16_t)(c->d + 0x10)));"]);
        assert_eq!(
            lines(&[0x1A], true, true),
            ["c->a = (uint16_t)((c->a & 0xFF00) | rt_inc8(c, (uint8_t)c->a));"]
        );
        assert_eq!(lines(&[0x0A], false, true), ["c->a = rt_asl16(c, c->a);"]);
        assert_eq!(
            lines(&[0x06, 0x10], false, true),
            [
                "{",
                "    uint32_t ea = (uint16_t)(c->d + 0x10);",
                "    uint16_t v = bus_read16_b0(ea);",
                "    v = rt_asl16(c, v);",
                "    bus_write16_b0(ea, v);",
                "}"
            ]
        );
        assert_eq!(lines(&[0x04, 0x10], true, true)[3], "    v = rt_tsb8(c, v);");
        assert_eq!(lines(&[0xE8], true, true), ["c->x = (uint8_t)(c->x + 1);", "rt_nz8(c, (uint8_t)c->x);"]);
        assert_eq!(lines(&[0x88], true, false), ["c->y = (uint16_t)(c->y - 1);", "rt_nz16(c, (uint16_t)c->y);"]);
    }

    #[test]
    fn flag_system_and_block_move_instructions() {
        assert_eq!(lines(&[0x18], true, true), ["c->c = 0;"]);
        assert_eq!(lines(&[0x38], true, true), ["c->c = 1;"]);
        assert_eq!(lines(&[0xF8], true, true), ["c->dec = 1;"]);
        assert_eq!(lines(&[0xC2, 0x30], true, true), ["rt_rep(c, 0x30);"]);
        assert_eq!(lines(&[0xE2, 0x20], true, true), ["rt_sep(c, 0x20);"]);
        assert!(lines(&[0xEA], true, true).is_empty());
        assert_eq!(lines(&[0xCB], true, true), ["rt_wai(c);"]);
        assert_eq!(lines(&[0xDB], true, true), ["rt_stp(c);", "return;"]);
        assert_eq!(lines(&[0x00, 0x00], true, true), ["rt_brk(c);", "return;"]);
        assert_eq!(lines(&[0x02, 0x00], true, true), ["rt_cop(c);", "return;"]);
        assert_eq!(lines(&[0x42, 0x00], true, true), ["rt_wdm(c);"]);
        // MVN encodes the destination bank first: MVN $7F,$7E copies from $7E to $7F.
        assert_eq!(lines(&[0x54, 0x7F, 0x7E], true, true), ["rt_block_move(c, 0x7F, 0x7E, 1);"]);
        assert_eq!(lines(&[0x44, 0x7F, 0x7E], true, true), ["rt_block_move(c, 0x7F, 0x7E, -1);"]);
    }

    /// C for the function at `entry` of a synthetic ROM.
    fn gen(segments: &[(u32, &[u8])], cfg: &str, roots: &[u32], entry: u32, depth: usize) -> Result<String, String> {
        let rom = test_rom(segments);
        let cfg = crate::config::Config::parse(cfg).unwrap();
        let r: Vec<_> = roots.iter().map(|&a| (a, Flags::RESET)).collect();
        let d = crate::discover::discover(&rom, &cfg, &r);
        let g = Graph::build(&d, roots);
        Emitter::new(&d, &g).generate(entry, depth)
    }

    /// The body lines of function `name`, trimmed.
    fn body(text: &str, name: &str) -> Vec<String> {
        let start = text.find(&format!("void {name}(Cpu *c) {{")).unwrap_or_else(|| panic!("{name} not emitted:\n{text}"));
        text[start..]
            .lines()
            .skip(1)
            .take_while(|l| *l != "}")
            .map(|l| l.trim().to_string())
            .collect()
    }

    #[test]
    fn emits_one_function_per_graph_function_with_a_stable_signature() {
        let rom = [(0x8000, &[0x20, 0x10, 0x80, 0x60][..]), (0x8010, &[0x60][..])];
        let out = gen(&rom, "", &[0x8000], 0x8000, 1).unwrap();
        assert!(out.contains("void f_00_8010(Cpu *c) {"), "{out}");
        assert!(out.contains("void f_00_8000(Cpu *c) {"), "{out}");
        assert!(out.starts_with("/* Generated by snes-recomp"));
        assert!(out.contains("#include \"snes_rt.h\""));
        assert_eq!(out, gen(&rom, "", &[0x8000], 0x8000, 1).unwrap(), "generation is deterministic");
    }

    #[test]
    fn conditional_branch_jumps_on_the_flag_and_falls_to_the_next_block() {
        // 8000: BNE $8003 ; 8002: NOP ; 8003: RTS
        let out = gen(&[(0x8000, &[0xD0, 0x01, 0xEA, 0x60])], "", &[0x8000], 0x8000, 0).unwrap();
        let b = body(&out, "f_00_8000");
        assert!(b.contains(&"if (!c->z) { goto L_00_8003; }".to_string()), "{b:?}");
        assert!(b.contains(&"L_00_8003:".to_string()));
        assert!(!b.contains(&"L_00_8002:".to_string()), "nothing jumps to the fall-through block");
        assert!(b.contains(&"(void)rt_pull16(c);".to_string()));
        assert_eq!(b.last().unwrap(), "return;");
    }

    #[test]
    fn calls_push_the_return_address_and_call_the_function() {
        // 8000: JSR $8010 ; 8003: RTS ; 8010: RTS
        let rom = [(0x8000, &[0x20, 0x10, 0x80, 0x60][..]), (0x8010, &[0x60][..])];
        let out = gen(&rom, "", &[0x8000], 0x8000, 1).unwrap();
        let b = body(&out, "f_00_8000");
        let at = b.iter().position(|l| l == "rt_push16(c, 0x8002);").expect("return address minus one");
        assert_eq!(b[at + 1], "f_00_8010(c);");
        // JSL also pushes the program bank first.
        let rom = [(0x8000, &[0x22, 0x10, 0x80, 0x01, 0x60][..]), (0x1_8010, &[0x6B][..])];
        let out = gen(&rom, "", &[0x8000], 0x8000, 1).unwrap();
        let b = body(&out, "f_00_8000");
        let at = b.iter().position(|l| l == "rt_push8(c, 0x00);").unwrap();
        assert_eq!(&b[at + 1..at + 3], ["rt_push16(c, 0x8003);", "f_01_8010(c);"]);
        assert_eq!(body(&out, "f_01_8010").iter().filter(|l| l.starts_with("(void)rt_pull")).count(), 2);
    }

    #[test]
    fn a_jump_to_another_function_is_a_tail_call() {
        // 8000: JMP $8010 ; 8010: RTS (both are entries)
        let rom = [(0x8000, &[0x4C, 0x10, 0x80][..]), (0x8010, &[0x60][..])];
        let out = gen(&rom, "", &[0x8000, 0x8010], 0x8000, 0).unwrap();
        let b = body(&out, "f_00_8000");
        assert!(b.contains(&"f_00_8010(c); return;".to_string()), "{b:?}");
        assert!(out.contains("void f_00_8010(Cpu *c);"), "declared, not defined");
        assert!(!out.contains("void f_00_8010(Cpu *c) {"));
    }

    #[test]
    fn depth_bounds_which_callees_are_emitted() {
        // 8000 -> 8010 -> 8020
        let rom = [
            (0x8000, &[0x20, 0x10, 0x80, 0x60][..]),
            (0x8010, &[0x20, 0x20, 0x80, 0x60][..]),
            (0x8020, &[0x60][..]),
        ];
        let d0 = gen(&rom, "", &[0x8000], 0x8000, 0).unwrap();
        assert!(d0.contains("void f_00_8000(Cpu *c) {") && !d0.contains("void f_00_8010(Cpu *c) {"));
        let d1 = gen(&rom, "", &[0x8000], 0x8000, 1).unwrap();
        assert!(d1.contains("void f_00_8010(Cpu *c) {") && !d1.contains("void f_00_8020(Cpu *c) {"));
        assert!(d1.contains("void f_00_8020(Cpu *c);"), "its call is declared");
        let d2 = gen(&rom, "", &[0x8000], 0x8000, 2).unwrap();
        assert!(d2.contains("void f_00_8020(Cpu *c) {"));
    }

    #[test]
    fn an_address_that_is_not_an_entry_is_an_error() {
        let rom = [(0x8000, &[0xEA, 0x60][..])];
        let err = gen(&rom, "", &[0x8000], 0x8001, 0).unwrap_err();
        assert!(err.contains("00:8001"), "{err}");
    }

    #[test]
    fn indirect_jump_dispatches_on_the_known_targets() {
        // 8000: JMP ($8100,X) with two table entries
        let rom = [
            (0x8000, &[0x7C, 0x00, 0x81][..]),
            (0x8100, &[0x10, 0x80, 0x20, 0x80][..]),
            (0x8010, &[0x60][..]),
            (0x8020, &[0x60][..]),
        ];
        let cfg = "[[table]]\nkind=\"indirect\"\nsite=\"00:8000\"\nat=\"00:8100\"\ncount=2";
        let out = gen(&rom, cfg, &[0x8000], 0x8000, 0).unwrap();
        let b = body(&out, "f_00_8000");
        assert!(
            b.contains(&"uint32_t t = (0x000000 | bus_read16(0x000000 | (uint16_t)(0x8100 + c->x)));".to_string()),
            "{b:?}"
        );
        assert!(b.contains(&"case 0x008010: f_00_8010(c); return;".to_string()));
        assert!(b.contains(&"case 0x008020: f_00_8020(c); return;".to_string()));
        assert!(b.contains(&"default: rt_unresolved(c, 0x008000, t); return;".to_string()));
        // Without the table nothing is known: only the default remains.
        let bare = gen(&rom, "", &[0x8000], 0x8000, 0).unwrap();
        assert!(!body(&bare, "f_00_8000").iter().any(|l| l.starts_with("case ")));
    }

    #[test]
    fn indirect_call_dispatches_and_continues() {
        // 8000: JSR ($8100,X) ; 8003: RTS
        let rom = [
            (0x8000, &[0xFC, 0x00, 0x81, 0x60][..]),
            (0x8100, &[0x10, 0x80][..]),
            (0x8010, &[0x60][..]),
        ];
        let cfg = "[[table]]\nkind=\"indirect\"\nsite=\"00:8000\"\nat=\"00:8100\"\ncount=1";
        let out = gen(&rom, cfg, &[0x8000], 0x8000, 0).unwrap();
        let b = body(&out, "f_00_8000");
        assert!(b.contains(&"case 0x008010: f_00_8010(c); break;".to_string()), "{b:?}");
        assert!(b.contains(&"rt_push16(c, 0x8002);".to_string()));
        assert_eq!(b.iter().filter(|l| l.starts_with("(void)rt_pull16")).count(), 1, "execution goes on to the RTS");
    }

    #[test]
    fn inline_helper_call_does_not_fall_into_its_table() {
        // 8000: JSR $8200 (helper) ; inline pointer $8010
        let rom = [
            (0x8000, &[0x20, 0x00, 0x82, 0x10, 0x80][..]),
            (0x8200, &[0x60][..]),
            (0x8010, &[0x60][..]),
        ];
        let out = gen(&rom, "[[inline_helper]]\naddr=\"00:8200\"", &[0x8000], 0x8000, 0).unwrap();
        let b = body(&out, "f_00_8000");
        assert_eq!(b.last().unwrap(), "return;");
        assert_eq!(b.iter().filter(|l| l.starts_with("// ")).count(), 1, "the table bytes are not instructions");
    }

    #[test]
    fn blocks_check_the_register_widths_they_assume() {
        // 8000: REP #$20 ; 8002: BEQ $8007 ; 8004: LDA #$1234 ; 8007: RTS
        let rom = [(0x8000, &[0xC2, 0x20, 0xF0, 0x03, 0xA9, 0x34, 0x12, 0x60][..])];
        let out = gen(&rom, "", &[0x8000], 0x8000, 0).unwrap();
        let b = body(&out, "f_00_8000");
        assert!(b.contains(&"rt_check_flags(c, 0x008000, 1, 1);".to_string()), "{b:?}");
        assert!(b.contains(&"rt_check_flags(c, 0x008004, 0, 1);".to_string()), "{b:?}");
    }

    #[test]
    fn conflicting_widths_are_an_error_with_no_partial_output() {
        // 8000: BEQ $8004 ; 8002: REP #$20 ; 8004: LDA #$12 (M=8 via the branch, M=16 falling through)
        let rom = [(0x8000, &[0xF0, 0x02, 0xC2, 0x20, 0xA9, 0x12, 0x00, 0x60][..])];
        let err = gen(&rom, "", &[0x8000], 0x8000, 0).unwrap_err();
        assert!(err.contains("00:8004") && err.contains("M=8") && err.contains("M=16"), "{err}");
    }

    #[test]
    fn conflicting_widths_on_width_independent_instructions_are_allowed() {
        // 8000: BEQ $8004 ; 8002: REP #$20 ; 8004: SEP #$30 (reached with M=8 and M=16) ; 8006: LDA #$12 ; RTS
        let rom = [(0x8000, &[0xF0, 0x02, 0xC2, 0x20, 0xE2, 0x30, 0xA9, 0x12, 0x60][..])];
        let out = gen(&rom, "", &[0x8000], 0x8000, 0).unwrap();
        let b = body(&out, "f_00_8000");
        assert!(b.contains(&"rt_check_flags(c, 0x008004, 2, 1);".to_string()), "{b:?}");
        assert!(b.contains(&"rt_check_flags(c, 0x008002, 1, 1);".to_string()), "{b:?}");
    }

    #[test]
    fn conflict_in_one_register_does_not_block_instructions_on_the_other() {
        // 8000: SEP #$10 ; BEQ $8006 ; 8004: REP #$10 ; 8006: STA $10 ; RTS
        // 8006 is reached with X=8 and X=16, but STA only depends on M.
        let rom = [(0x8000, &[0xE2, 0x10, 0xF0, 0x02, 0xC2, 0x10, 0x85, 0x10, 0x60][..])];
        let out = gen(&rom, "", &[0x8000], 0x8000, 0).unwrap();
        let b = body(&out, "f_00_8000");
        assert!(b.contains(&"rt_check_flags(c, 0x008006, 1, 2);".to_string()), "{b:?}");
    }

    #[test]
    fn code_shared_with_another_function_is_emitted_in_each() {
        // 8000: JMP $8100 ; 8010: JMP $8100 ; 8100: RTS (both entries reach the same block)
        let rom = [
            (0x8000, &[0x4C, 0x00, 0x81][..]),
            (0x8010, &[0x4C, 0x00, 0x81][..]),
            (0x8100, &[0x60][..]),
        ];
        let out = gen(&rom, "", &[0x8000, 0x8010], 0x8010, 0).unwrap();
        assert!(body(&out, "f_00_8010").contains(&"L_00_8100:".to_string()));
    }

    /// Generates C for the function at `roots[0]`, compiles it with `SNES_RT_CHECKS` against the
    /// mock runtime and runs `main_body` (which sees a zeroed native-mode `Cpu c` with a stack at
    /// $01FF and a helper `load`). `None` when there is no C compiler.
    fn exec(name: &str, segments: &[(u32, &[u8])], cfg: &str, roots: &[u32], main_body: &str) -> Option<String> {
        let cc = match crate::cctest::find() {
            Some(cc) => cc,
            None => {
                eprintln!("skipped {name}: no C compiler found");
                return None;
            }
        };
        let generated = gen(segments, cfg, roots, roots[0], 4).unwrap();
        let main = format!(
            r#"#include "snes_rt.h"
#include <stdio.h>
void {entry}(Cpu *c);
static void load(uint32_t a, const uint8_t *p, int n) {{ int i; for (i = 0; i < n; i++) bus_write8(a + i, p[i]); }}
int main(void) {{
    Cpu c = {{0}};
    c.m = 1; c.xf = 1; c.s = 0x01FF;
    {body}
    {entry}(&c);
    {report}
    return 0;
}}
"#,
            entry = func_name(roots[0]),
            body = main_body.split("//report").next().unwrap(),
            report = main_body.split("//report").nth(1).unwrap_or(""),
        );
        let out = cc
            .compile_run(
                name,
                &[("snes_rt.h", RUNTIME_HEADER), ("rt.c", MOCK_RUNTIME), ("gen.c", &generated), ("main.c", &main)],
                &["SNES_RT_CHECKS"],
            )
            .unwrap_or_else(|e| panic!("{name}: {e}\n--- generated C ---\n{generated}"));
        Some(out)
    }

    #[test]
    fn runs_8_bit_load_add_and_store() {
        // LDA #$10 ; CLC ; ADC #$22 ; STA $10 ; RTS
        let rom = [(0x8000, &[0xA9, 0x10, 0x18, 0x69, 0x22, 0x85, 0x10, 0x60][..])];
        let Some(out) = exec("add8", &rom, "", &[0x8000], r#"//report
            printf("a=%04X mem=%02X s=%04X\n", c.a, bus_read8(0x10), c.s);"#) else { return };
        assert_eq!(out, "a=0032 mem=32 s=0201\n");
    }

    #[test]
    fn runs_16_bit_arithmetic_after_rep() {
        // REP #$20 ; LDA #$1234 ; CLC ; ADC #$1111 ; STA $2000 ; RTS
        let rom = [(0x8000, &[0xC2, 0x20, 0xA9, 0x34, 0x12, 0x18, 0x69, 0x11, 0x11, 0x8D, 0x00, 0x20, 0x60][..])];
        let Some(out) = exec("add16", &rom, "", &[0x8000], r#"//report
            printf("a=%04X mem=%02X%02X m=%d\n", c.a, bus_read8(0x2001), bus_read8(0x2000), c.m);"#) else { return };
        assert_eq!(out, "a=2345 mem=2345 m=0\n");
    }

    #[test]
    fn runs_a_counting_loop() {
        // LDX #$05 ; LDA #$00 ; loop: CLC ; ADC #$02 ; DEX ; BNE loop ; STA $10 ; RTS
        let rom = [(0x8000, &[0xA2, 0x05, 0xA9, 0x00, 0x18, 0x69, 0x02, 0xCA, 0xD0, 0xFA, 0x85, 0x10, 0x60][..])];
        let Some(out) = exec("loop", &rom, "", &[0x8000], r#"//report
            printf("mem=%02X x=%02X z=%d\n", bus_read8(0x10), c.x, c.z);"#) else { return };
        assert_eq!(out, "mem=0A x=00 z=1\n");
    }

    #[test]
    fn runs_compare_and_branch() {
        // LDA #$05 ; CMP #$05 ; BNE skip ; LDA #$AA ; skip: STA $10 ; RTS
        let rom = [(0x8000, &[0xA9, 0x05, 0xC9, 0x05, 0xD0, 0x02, 0xA9, 0xAA, 0x85, 0x10, 0x60][..])];
        let Some(out) = exec("cmp", &rom, "", &[0x8000], r#"//report
            printf("mem=%02X c=%d\n", bus_read8(0x10), c.c);"#) else { return };
        assert_eq!(out, "mem=AA c=1\n");
    }

    #[test]
    fn runs_jsr_and_rts_keeping_the_stack_balanced() {
        // 8000: JSR $8010 ; INC A ; STA $10 ; RTS     8010: LDA #$41 ; RTS
        let rom = [
            (0x8000, &[0x20, 0x10, 0x80, 0x1A, 0x85, 0x10, 0x60][..]),
            (0x8010, &[0xA9, 0x41, 0x60][..]),
        ];
        let Some(out) = exec("jsr", &rom, "", &[0x8000], r#"//report
            printf("mem=%02X s=%04X\n", bus_read8(0x10), c.s);"#) else { return };
        assert_eq!(out, "mem=42 s=0201\n");
    }

    #[test]
    fn runs_jsl_and_rtl_across_banks() {
        // 8000: JSL $018010 ; STA $10 ; RTS     01:8010: LDA #$55 ; RTL
        let rom = [(0x8000, &[0x22, 0x10, 0x80, 0x01, 0x85, 0x10, 0x60][..]), (0x1_8010, &[0xA9, 0x55, 0x6B][..])];
        let Some(out) = exec("jsl", &rom, "", &[0x8000], r#"//report
            printf("mem=%02X s=%04X\n", bus_read8(0x10), c.s);"#) else { return };
        assert_eq!(out, "mem=55 s=0201\n");
    }

    #[test]
    fn runs_an_indirect_jump_through_a_table() {
        // 8000: LDX #$02 ; JMP ($8100,X)    table: $8010, $8020    8020: LDA #$77 ; STA $10 ; RTS
        let rom = [
            (0x8000, &[0xA2, 0x02, 0x7C, 0x00, 0x81][..]),
            (0x8100, &[0x10, 0x80, 0x20, 0x80][..]),
            (0x8010, &[0xA9, 0x11, 0x85, 0x10, 0x60][..]),
            (0x8020, &[0xA9, 0x77, 0x85, 0x10, 0x60][..]),
        ];
        let cfg = "[[table]]\nkind=\"indirect\"\nsite=\"00:8002\"\nat=\"00:8100\"\ncount=2";
        let body = r#"{ const uint8_t t[] = {0x10, 0x80, 0x20, 0x80}; load(0x8100, t, 4); }
            //report
            printf("mem=%02X\n", bus_read8(0x10));"#;
        let Some(out) = exec("jmpx", &rom, cfg, &[0x8000], body) else { return };
        assert_eq!(out, "mem=77\n");
        // A pointer the recompiler never saw is reported, not followed.
        let body = r#"{ const uint8_t t[] = {0x10, 0x80, 0x00, 0x00}; load(0x8100, t, 4); }
            //report
            printf("mem=%02X\n", bus_read8(0x10));"#;
        let Some(out) = exec("jmpx-unknown", &rom, cfg, &[0x8000], body) else { return };
        assert_eq!(out, "UNRESOLVED site=008002 target=000000\nmem=00\n");
    }

    #[test]
    fn runs_decimal_mode_addition() {
        // SED ; CLC ; LDA #$19 ; ADC #$28 ; STA $10 ; CLD ; RTS
        let rom = [(0x8000, &[0xF8, 0x18, 0xA9, 0x19, 0x69, 0x28, 0x85, 0x10, 0xD8, 0x60][..])];
        let Some(out) = exec("bcd", &rom, "", &[0x8000], r#"//report
            printf("mem=%02X dec=%d\n", bus_read8(0x10), c.dec);"#) else { return };
        assert_eq!(out, "mem=47 dec=0\n");
    }

    #[test]
    fn runs_a_block_move() {
        // REP #$30 ; LDA #$0003 ; LDX #$1000 ; LDY #$2000 ; MVN $7F,$7E ; RTS
        let rom = [(
            0x8000,
            &[0xC2, 0x30, 0xA9, 0x03, 0x00, 0xA2, 0x00, 0x10, 0xA0, 0x00, 0x20, 0x54, 0x7F, 0x7E, 0x60][..],
        )];
        let body = r#"{ const uint8_t s[] = {0xAA, 0xBB, 0xCC, 0xDD}; load(0x7E1000, s, 4); }
            //report
            printf("%02X%02X%02X%02X a=%04X x=%04X y=%04X db=%02X\n", bus_read8(0x7F2000), bus_read8(0x7F2001),
                   bus_read8(0x7F2002), bus_read8(0x7F2003), c.a, c.x, c.y, c.db);"#;
        let Some(out) = exec("mvn", &rom, "", &[0x8000], body) else { return };
        assert_eq!(out, "AABBCCDD a=FFFF x=1004 y=2004 db=7F\n");
    }

    #[test]
    fn checked_builds_report_no_width_mismatch_for_consistent_code() {
        // SEP/REP round trip: the assumed widths always match what the CPU state says.
        // REP #$30 ; LDA #$1234 ; SEP #$20 ; LDA #$FF ; RTS
        let rom = [(0x8000, &[0xC2, 0x30, 0xA9, 0x34, 0x12, 0xE2, 0x20, 0xA9, 0xFF, 0x60][..])];
        let Some(out) = exec("checks", &rom, "", &[0x8000], r#"//report
            printf("a=%04X\n", c.a);"#) else { return };
        assert_eq!(out, "a=12FF\n");
    }

    #[test]
    fn checked_builds_accept_a_callee_that_returns_with_either_width() {
        // 8000: JSR $8020 ; 8003: SEP #$30 ; 8005: LDA #$12 ; 8007: RTS
        // 8020: LDA $10 ; BEQ $8026 ; 8024: REP #$30 ; 8026: RTS (returns 8/8 or 16/16)
        let rom = [
            (0x8000, &[0x20, 0x20, 0x80, 0xE2, 0x30, 0xA9, 0x12, 0x60][..]),
            (0x8020, &[0xA5, 0x10, 0xF0, 0x02, 0xC2, 0x30, 0x60][..]),
        ];
        for (flag, name) in [(0u8, "either_zero"), (1u8, "either_nonzero")] {
            let body = format!(
                r#"{{ const uint8_t v[] = {{{flag}}}; load(0x10, v, 1); }}
            //report
            printf("a=%02X m=%d xf=%d\n", c.a & 0xFF, c.m, c.xf);"#
            );
            let Some(out) = exec(name, &rom, "", &[0x8000], &body) else { return };
            assert_eq!(out, "a=12 m=1 xf=1\n");
        }
    }

    /// Bus for the real game: LoROM in banks $00-$3F/$80-$BF, WRAM, and a log of hardware writes.
    /// Stops the program when the reset code starts waiting for the NMI (a read of $12 after
    /// `$4200` is written with $81), or after too many bus accesses.
    const ALTTP_BUS: &str = r#"
#include "snes_rt.h"
#include <stdio.h>
#include <stdlib.h>

static uint8_t wram[0x20000];
static uint8_t rom[0x200000]; /* banks $00-$3F, 32 KiB each */
static unsigned long accesses;
static int armed;
/* A stand-in for the sound CPU: its ports say $AA,$BB until the upload handshake ($CC on port 0)
 * and echo what the CPU writes afterwards. */
static uint8_t apu_port[4] = {0xAA, 0xBB, 0, 0};
static int apu_started;

static void load_rom(void) {
    FILE *f = fopen("lorom.bin", "rb");
    if (!f || fread(rom, 1, sizeof rom, f) != sizeof rom) { printf("NO ROM\n"); exit(4); }
    fclose(f);
}

uint8_t bus_read8(uint32_t a) {
    uint32_t bank = (a >> 16) & 0xFF, off = a & 0xFFFF;
    if (++accesses > 30000000UL) { printf("TIMEOUT\n"); exit(3); }
    if (bank == 0x7E) return wram[off];
    if (bank == 0x7F) return wram[0x10000 + off];
    if (bank < 0x40 || (bank >= 0x80 && bank < 0xC0)) {
        if (off < 0x2000) {
            if (off == 0x12 && armed) { printf("WAIT_LOOP\n"); exit(0); }
            return wram[off];
        }
        if (off >= 0x8000) return rom[((bank & 0x3F) << 15) | (off & 0x7FFF)];
        if (off >= 0x2140 && off < 0x2144) return apu_port[off - 0x2140];
    }
    return 0;
}

void bus_write8(uint32_t a, uint8_t v) {
    uint32_t bank = (a >> 16) & 0xFF, off = a & 0xFFFF;
    if (++accesses > 30000000UL) { printf("TIMEOUT\n"); exit(3); }
    if (bank == 0x7E) { wram[off] = v; return; }
    if (bank == 0x7F) { wram[0x10000 + off] = v; return; }
    if (bank < 0x40 || (bank >= 0x80 && bank < 0xC0)) {
        if (off < 0x2000) { wram[off] = v; return; }
        if (off == 0x2140 && v == 0xCC) apu_started = 1;
        if (off >= 0x2140 && off < 0x2144 && apu_started) apu_port[off - 0x2140] = v;
        if ((off >= 0x2100 && off < 0x2200) || (off >= 0x4200 && off < 0x4400)) {
            printf("W %04X=%02X\n", (unsigned)off, v);
            if (off == 0x4200 && v == 0x81) armed = 1;
        }
    }
}

void rt_unresolved(Cpu *c, uint32_t site, uint32_t target) {
    (void)c; printf("UNRESOLVED site=%06X target=%06X\n", (unsigned)site, (unsigned)target); exit(5);
}
void rt_flag_mismatch(Cpu *c, uint32_t addr) { (void)c; printf("FLAG_MISMATCH %06X\n", (unsigned)addr); exit(6); }
void rt_wai(Cpu *c) { (void)c; printf("WAI\n"); }
void rt_stp(Cpu *c) { (void)c; printf("STP\n"); exit(7); }
void rt_brk(Cpu *c) { (void)c; printf("BRK\n"); exit(7); }
void rt_cop(Cpu *c) { (void)c; printf("COP\n"); exit(7); }
void rt_wdm(Cpu *c) { (void)c; }

void f_00_8000(Cpu *c);
int main(void) {
    Cpu c = {0};
    load_rom();
    c.e = 1; c.m = 1; c.xf = 1; c.s = 0x01FF; c.irq = 1; /* state right after reset */
    f_00_8000(&c);
    printf("RETURNED\n");
    return 0;
}
"#;

    /// Reset of A Link to the Past, recompiled with its callees and run against the real ROM until
    /// it reaches the wait for the first NMI. Skipped without the ROM or a C compiler.
    #[test]
    fn alttp_reset_runs_to_the_nmi_wait_loop() {
        let Some(cc) = crate::cctest::find() else {
            eprintln!("skipped: no C compiler found");
            return;
        };
        let Ok(rom) = crate::rom::Rom::load("roms/alttp_usa.sfc") else {
            eprintln!("skipped: roms/alttp_usa.sfc not found");
            return;
        };
        let cfg = crate::config::Config::load("games/alttp_usa.toml").unwrap();
        let roots: Vec<(u32, Flags)> = cfg.entries.iter().map(|e| (e.addr, e.flags)).collect();
        let d = crate::discover::discover(&rom, &cfg, &roots);
        let addrs: Vec<u32> = roots.iter().map(|r| r.0).collect();
        let g = Graph::build(&d, &addrs);
        let generated = Emitter::new(&d, &g).generate(0x8000, 2).unwrap();

        let mut image = vec![0u8; 0x20_0000];
        for bank in 0..0x40u32 {
            for off in 0..0x8000u32 {
                image[((bank << 15) | off) as usize] = rom.read8((bank << 16) | 0x8000 | off).unwrap_or(0);
            }
        }
        // The compiler runs in a scratch directory, so the ROM image travels as C source.
        let rom_c: String = {
            let mut s = String::from("const unsigned char rom_image[] = {");
            for (i, b) in image.iter().enumerate() {
                if i % 32 == 0 {
                    s.push('\n');
                }
                s.push_str(&format!("{b},"));
            }
            s.push_str("0};\n");
            s
        };
        let bus = ALTTP_BUS.replace(
            "FILE *f = fopen(\"lorom.bin\", \"rb\");\n    if (!f || fread(rom, 1, sizeof rom, f) != sizeof rom) { printf(\"NO ROM\\n\"); exit(4); }\n    fclose(f);",
            "memcpy(rom, rom_image, sizeof rom);",
        )
        .replace("#include <stdlib.h>", "#include <stdlib.h>\n#include <string.h>\nextern const unsigned char rom_image[];");
        // Functions that are only declared get a stub: reaching one means the run left the slice.
        let stubs: String = generated
            .lines()
            .filter_map(|l| l.strip_prefix("void ")?.strip_suffix("(Cpu *c);"))
            .filter(|name| !generated.contains(&format!("void {name}(Cpu *c) {{")))
            .map(|name| {
                format!("void {name}(Cpu *c) {{ (void)c; printf(\"UNEMITTED {name}\\n\"); exit(8); }}\n")
            })
            .collect();
        let stubs = format!("#include \"snes_rt.h\"
#include <stdio.h>
#include <stdlib.h>
{stubs}");
        let out = cc
            .compile_run(
                "alttp-reset",
                &[
                    ("snes_rt.h", RUNTIME_HEADER),
                    ("gen.c", &generated),
                    ("stubs.c", &stubs),
                    ("rom.c", &rom_c),
                    ("bus.c", &bus),
                ],
                &["SNES_RT_CHECKS"],
            )
            .unwrap_or_else(|e| panic!("{e}"));
        let lines: Vec<&str> = out.lines().collect();
        eprintln!("{} lines; first 12: {:?}; last 3: {:?}", lines.len(), &lines[..lines.len().min(12)], &lines[lines.len().saturating_sub(3)..]);
        // The first block of the reset code, as the disassembly reads: clear NMI/DMA enables and
        // the APU ports, then force blank.
        assert_eq!(
            &lines[..8],
            ["W 4200=00", "W 420C=00", "W 420B=00", "W 2140=00", "W 2141=00", "W 2142=00", "W 2143=00", "W 2100=80"]
        );
        assert_eq!(lines.last(), Some(&"WAIT_LOOP"), "output tail: {:?}", &lines[lines.len().saturating_sub(5)..]);
        assert!(lines.contains(&"W 4200=81"), "NMI is enabled just before the wait");
        assert!(!lines.iter().any(|l| l.starts_with("FLAG_MISMATCH") || l.starts_with("UNRESOLVED")));
    }
}
