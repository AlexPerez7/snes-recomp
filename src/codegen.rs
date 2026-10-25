//! C code generation: one C function per function of the graph, one label per basic block.

use crate::config::{Hook, HookKind, HookReturn};
use crate::disasm::{fmt_addr, Flags, Flow, Instr};
use crate::discover::{Discovery, EdgeKind};
use crate::graph::{Block, Graph, SuccKind};
use crate::opcodes::Mode;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Interface between the generated code and the runtime, written next to the generated C.
pub const RUNTIME_HEADER: &str = include_str!("../runtime/snes_rt.h");

/// Runtime without video or audio, written next to the generated C by `codegen --all`.
pub const HEADLESS_RUNTIME: &str = include_str!("../runtime/headless.c");

/// Hardware core shared by the runtimes (bus, DMA, clock, contexts, report); included by `headless.c`.
pub const RUNTIME_CORE: &str = include_str!("../runtime/snes_core.inc");

/// Interactive frontend (SDL2 window and keyboard), written as `play/snes_play.c` by `codegen --all`.
pub const PLAY_RUNTIME: &str = include_str!("../runtime/play.c");

/// Options, INI file, picture layout, screen effects and key map of the interactive frontend (no SDL), written as
/// `play/play_cfg.h` by `codegen --all`.
pub const PLAY_CFG: &str = include_str!("../runtime/play_cfg.h");

/// The look of the menu without SDL (font, panel, cursor): `play/play_ui.h` written by `codegen --all`.
pub const PLAY_UI: &str = include_str!("../runtime/play_ui.h");

/// Script that builds `snes_play.exe` from the files `codegen --all` wrote (Visual Studio and SDL2).
pub const BUILD_PLAY_BAT: &str = include_str!("../runtime/build_play.bat");

/// PPU renderer and PNG writer, included by `snes_core.inc`.
pub const RUNTIME_PPU: &str = include_str!("../runtime/snes_ppu.inc");
pub const RUNTIME_SPC: &str = include_str!("../runtime/snes_spc.inc");

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
    // A return that does not match the stack and address this call expects belongs to an outer caller.
    let lost = format!("if (c->s != s0 || c->ret != 0x{ret:04X}) return;");
    let mut push = vec!["uint16_t s0 = c->s;".to_string()];
    if ins.mnemonic == "JSL" {
        push.push(format!("rt_push8(c, 0x{:02X});", ins.addr >> 16));
    }
    push.push(format!("rt_push16(c, 0x{ret:04X});"));
    match ins.flow() {
        Flow::Call(t) => {
            push.push(format!("{}(c);", func_name(t)));
            push.push(lost);
            let mut out = vec!["{".to_string()];
            out.extend(push.into_iter().map(|l| format!("    {l}")));
            out.push("}".into());
            Ok(out)
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
            out.push(format!("    {lost}"));
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
    /// Calls to C functions of the game, by the entry of the function they hook.
    hooks: BTreeMap<u32, Vec<Hook>>,
}

/// A function body under construction: the blocks it owns and what it refers to.
struct Body {
    entry: u32,
    blocks: BTreeSet<u32>,
    used_labels: BTreeSet<String>,
    refs: BTreeSet<u32>,
    /// Set when blocks are emitted once per combination of widths they are reached with.
    var: Option<Variants>,
}

/// The copies of a function's blocks, one per `(start, widths)`.
struct Variants {
    all: BTreeSet<(u32, Flags)>,
    /// Starts with more than one copy: only these get the widths in their label.
    multi: BTreeSet<u32>,
    /// Last instruction (and its widths) of the block being emitted.
    cur: (u32, Flags),
}

impl Variants {
    fn label(&self, start: u32, f: Flags) -> String {
        if self.multi.contains(&start) {
            format!("{}_m{}x{}", label(start), if f.m { 8 } else { 16 }, if f.x { 8 } else { 16 })
        } else {
            label(start)
        }
    }
}

impl<'a> Emitter<'a> {
    pub fn new(d: &'a Discovery, g: &'a Graph) -> Self {
        Emitter { d, g, hooks: BTreeMap::new() }
    }

    /// Hooks the functions of the game's profile: a hook runs `before` the body of the function at `addr`, `after` it
    /// has returned (by any of its returns, or a tail call), or `replace`s the body and returns as the hook says.
    /// Every `addr` has to be the entry of a function of the graph.
    pub fn with_hooks(mut self, hooks: &[Hook]) -> Result<Self, String> {
        for h in hooks {
            if !self.g.functions.contains_key(&h.addr) {
                return Err(format!("hook `{}`: {} is not the entry of a function", h.call, fmt_addr(h.addr)));
            }
            self.hooks.entry(h.addr).or_default().push(h.clone());
        }
        Ok(self)
    }

    /// The names of the C functions the hooks call, each once.
    fn hook_calls(&self) -> BTreeSet<&str> {
        self.hooks.values().flatten().map(|h| h.call.as_str()).collect()
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

    /// Widths before each instruction of `block` when it starts with `f`.
    fn chain(&self, block: &Block, f: Flags) -> Result<Vec<Flags>, String> {
        let mut out = vec![f];
        for pair in block.instrs.windows(2) {
            let key = (pair[0], *out.last().expect("not empty"));
            let mut next = self.d.transitions.get(&key).into_iter().flatten().filter(|t| t.0 == pair[1]);
            match (next.next(), next.next()) {
                (Some(t), None) => out.push(t.1),
                _ => return Err(format!("{} is reached with different register widths and the widths after it are not known", fmt_addr(pair[0]))),
            }
        }
        Ok(out)
    }

    /// The widths a function is entered with: all those it is called with, the first one found first.
    fn entry_widths(&self, entry: u32) -> Result<Vec<Flags>, String> {
        let first = *self.d.flags.get(&entry).ok_or_else(|| format!("{} has no widths", fmt_addr(entry)))?;
        let mut out = vec![first];
        out.extend(self.d.flag_conflicts.get(&entry).into_iter().flatten().copied().filter(|&f| f != first));
        Ok(out)
    }

    /// Every `(start, widths)` copy of the blocks of a function reachable from its entry.
    fn variants(&self, entry: u32, blocks: &BTreeSet<u32>) -> Result<BTreeSet<(u32, Flags)>, String> {
        let mut all = BTreeSet::new();
        let mut queue: Vec<(u32, Flags)> = self.entry_widths(entry)?.into_iter().map(|f| (entry, f)).collect();
        while let Some((start, f)) = queue.pop() {
            if !all.insert((start, f)) {
                continue;
            }
            let block = &self.g.blocks[&start];
            let last = (block.end(), *self.chain(block, f)?.last().expect("not empty"));
            for &(to, nf) in self.d.transitions.get(&last).into_iter().flatten() {
                if blocks.contains(&to) {
                    queue.push((to, nf));
                }
            }
        }
        Ok(all)
    }

    /// The C label of the block at `to` that control reaches from the block being emitted.
    fn target_label(&self, body: &Body, to: u32) -> Result<String, String> {
        let Some(v) = &body.var else { return Ok(label(to)) };
        let mut ways = self.d.transitions.get(&v.cur).into_iter().flatten().filter(|t| t.0 == to);
        match (ways.next(), ways.next()) {
            (Some(t), None) => Ok(v.label(to, t.1)),
            _ => {
                let mut copies = v.all.iter().filter(|c| c.0 == to);
                match (copies.next(), copies.next()) {
                    (Some(c), None) => Ok(v.label(to, c.1)),
                    _ => Err(format!("{} is reached with different register widths and the copy to jump to is not known", fmt_addr(to))),
                }
            }
        }
    }

    /// Transfers control to `target`: a `goto` inside the function, or a tail call.
    fn jump(&self, body: &mut Body, target: u32) -> Result<String, String> {
        if body.blocks.contains(&target) {
            let l = self.target_label(body, target)?;
            body.used_labels.insert(l.clone());
            Ok(format!("goto {l};"))
        } else if self.g.functions.contains_key(&target) {
            body.refs.insert(target);
            Ok(format!("{}(c); return;", func_name(target)))
        } else {
            Err(format!("{} jumps to {}, which is not code", func_name(body.entry), fmt_addr(target)))
        }
    }

    fn block_lines(&self, body: &mut Body, block: &Block, next: Option<&str>, copy: Option<Flags>) -> Result<Vec<String>, String> {
        let flags = copy.unwrap_or(self.d.flags[&block.start]);
        // A width that differs among the ways the block is entered (2) is not checked; a copy has one.
        let varies = |pick: fn(&Flags) -> bool| {
            copy.is_none() && self.d.flag_conflicts.get(&block.start).is_some_and(|s| s.iter().any(|f| pick(f) != pick(&flags)))
        };
        let (m, x) = (if varies(|f| f.m) { 2 } else { flags.m as u8 }, if varies(|f| f.x) { 2 } else { flags.x as u8 });
        let widths: Vec<Flags> = match copy {
            Some(f0) => self.chain(block, f0)?,
            None => block.instrs.iter().map(|a| self.d.flags[a]).collect(),
        };
        if let Some(v) = &mut body.var {
            v.cur = (block.end(), *widths.last().expect("a block has at least one instruction"));
        }
        let mut out = vec!["rt_tick(c);".to_string(), format!("rt_check_flags(c, 0x{:06X}, {}, {});", block.start, m, x)];
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
            let f = widths[i];
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
                Flow::Return if !dispatch.is_empty() => {
                    // PHA/RTS dispatch: jump to the pushed address + 1, in the site's bank.
                    ends = true;
                    // An address outside the table is a plain return: the RTS can be shared with a branch that does not dispatch.
                    let mut v = vec!["{".to_string(), "    uint16_t p = rt_pull16(c);".into(), format!("    uint32_t t = 0x{:06X} | (uint16_t)(p + 1);", a & 0xFF_0000), "    switch (t) {".into()];
                    for &t in &dispatch {
                        v.push(format!("    case 0x{t:06X}: {}", self.jump(body, t)?));
                    }
                    v.push("    default: c->ret = p; return;".into());
                    v.push("    }".into());
                    v.push("}".into());
                    v
                }
                Flow::Return => {
                    ends = true;
                    match ins.mnemonic {
                        "RTS" => vec!["c->ret = rt_pull16(c);".into(), "return;".into()],
                        "RTL" => vec!["c->ret = rt_pull16(c);".into(), "(void)rt_pull8(c);".into(), "return;".into()],
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
                Some(&(to, _)) if body.blocks.contains(&to) && Some(self.target_label(body, to)?.as_str()) == next => {}
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
        let Some(hooks) = self.hooks.get(&entry) else {
            return self.plain_function(entry);
        };
        let name = func_name(entry);
        let replace = hooks.iter().find(|h| h.kind == HookKind::Replace);
        // a replaced body is not translated at all, so a function that cannot be translated can still be replaced
        let (mut text, refs) = if replace.is_some() { (String::new(), BTreeSet::new()) } else { self.plain_function(entry)? };
        let mut call = String::new();
        if replace.is_none() {
            let header = format!("void {name}(Cpu *c) {{");
            assert!(text.starts_with(&header), "a function starts with its header");
            text = format!("static void {name}_body(Cpu *c) {{{}", &text[header.len()..]);
            text.push('\n');
        }
        let hooked = |kind: HookKind| hooks.iter().filter(move |h| h.kind == kind);
        text.push_str(&format!("void {name}(Cpu *c) {{\n"));
        for h in hooked(HookKind::Before) {
            call.push_str(&format!("    {}(c);\n", h.call));
        }
        match replace {
            Some(h) => {
                call.push_str(&format!("    {}(c);\n    c->ret = rt_pull16(c);\n", h.call));
                if h.ret == Some(HookReturn::Rtl) {
                    call.push_str("    (void)rt_pull8(c);\n");
                }
            }
            None => call.push_str(&format!("    {name}_body(c);\n")),
        }
        for h in hooked(HookKind::After) {
            call.push_str(&format!("    {}(c);\n", h.call));
        }
        text.push_str(&call);
        text.push_str("}\n");
        Ok((text, refs))
    }

    fn plain_function(&self, entry: u32) -> Result<(String, BTreeSet<u32>), String> {
        let blocks = self.reachable_blocks(entry);
        match self.check_widths(&blocks) {
            Ok(()) => self.function_text(entry, blocks, None),
            // The C depends on widths that differ: one copy of the blocks per combination of widths,
            // or the original error if they cannot be told apart.
            Err(e) => {
                let all = self.variants(entry, &blocks).map_err(|_| e.clone())?;
                self.function_text(entry, blocks, Some(all)).map_err(|inner| if inner.contains("different register widths") { e } else { inner })
            }
        }
    }

    fn function_text(&self, entry: u32, blocks: BTreeSet<u32>, all: Option<BTreeSet<(u32, Flags)>>) -> Result<(String, BTreeSet<u32>), String> {
        let var = all.map(|all| {
            let mut multi = BTreeSet::new();
            let mut seen = BTreeSet::new();
            for &(start, _) in &all {
                if !seen.insert(start) {
                    multi.insert(start);
                }
            }
            Variants { all, multi, cur: (entry, Flags { m: true, x: true }) }
        });
        // (start, widths of the copy, label) in layout order.
        let order: Vec<(u32, Option<Flags>, String)> = match &var {
            None => blocks.iter().map(|&b| (b, None, label(b))).collect(),
            Some(v) => v.all.iter().map(|&(b, f)| (b, Some(f), v.label(b, f))).collect(),
        };
        let entry_label = match &var {
            None => label(entry),
            Some(v) => v.label(entry, self.d.flags[&entry]),
        };
        let mut body = Body { entry, blocks: blocks.clone(), used_labels: BTreeSet::new(), refs: BTreeSet::new(), var };
        let mut chunks = Vec::new();
        for (i, (start, copy, name)) in order.iter().enumerate() {
            let next = order.get(i + 1).map(|o| o.2.as_str());
            let lines = self.block_lines(&mut body, &self.g.blocks[start], next, *copy)?;
            chunks.push((name.clone(), lines));
        }
        let mut text = format!("void {}(Cpu *c) {{\n", func_name(entry));
        // Blocks are laid out by address, so an entry that is not the lowest block needs a jump.
        let mut entries = Vec::new();
        if let Some(v) = &body.var {
            // Entered with several widths: pick the copy that matches the CPU, the first found by default.
            for f in self.entry_widths(entry)?.into_iter().skip(1) {
                entries.push((f, v.label(entry, f)));
            }
        }
        for (f, name) in &entries {
            body.used_labels.insert(name.clone());
            text.push_str(&format!("    if (c->m == {} && c->xf == {}) goto {name};\n", f.m as u8, f.x as u8));
        }
        if !entries.is_empty() || order.first().map(|o| &o.2) != Some(&entry_label) {
            body.used_labels.insert(entry_label.clone());
            text.push_str(&format!("    goto {entry_label};\n"));
        }
        for (name, lines) in chunks {
            if body.used_labels.contains(&name) {
                text.push_str(&format!("{name}:\n"));
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
            // Only the entry has to translate; a callee that cannot is left out like one past the depth.
            let (text, refs) = match self.function(at) {
                Ok(f) => f,
                Err(e) if level == 0 => return Err(e),
                Err(_) => continue,
            };
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
        for call in self.hook_calls() {
            out.push_str(&format!("void {call}(Cpu *c);\n"));
        }
        for (_, text) in bodies {
            out.push('\n');
            out.push_str(&text);
        }
        Ok(out)
    }

    /// Every function of the graph, split by the bank of its entry. A function that cannot be
    /// translated becomes a stub that calls `rt_unsupported`, and is listed with its cause.
    pub fn generate_all(&self) -> FullOutput {
        let banner = "/* Generated by snes-recomp from the ROM: do not commit or distribute. */\n";
        let mut banks: BTreeMap<u8, Vec<String>> = BTreeMap::new();
        let mut unsupported = Vec::new();
        let mut referenced: BTreeSet<u32> = self.g.functions.keys().copied().collect();
        for &entry in self.g.functions.keys() {
            let text = match self.function(entry) {
                Ok((text, refs)) => {
                    referenced.extend(refs);
                    text
                }
                Err(message) => {
                    unsupported.push(Unsupported { entry, cause: Cause::classify(&message), message });
                    format!("void {}(Cpu *c) {{\n    rt_unsupported(c, 0x{entry:06X});\n}}\n", func_name(entry))
                }
            };
            banks.entry((entry >> 16) as u8).or_default().push(text);
        }
        let mut header = format!("{banner}#include \"snes_rt.h\"\n\n");
        for r in &referenced {
            header.push_str(&format!("void {}(Cpu *c);\n", func_name(*r)));
        }
        for call in self.hook_calls() {
            header.push_str(&format!("void {call}(Cpu *c);\n"));
        }
        let banks = banks
            .into_iter()
            .map(|(bank, bodies)| {
                let mut out = format!("{banner}#include \"snes_funcs.h\"\n");
                for b in bodies {
                    out.push('\n');
                    out.push_str(&b);
                }
                (bank, out)
            })
            .collect();
        header.push_str("\nint snes_call_entry(Cpu *c, uint32_t addr);\n");
        let mut entry = format!("{banner}#include \"snes_funcs.h\"\n\n");
        entry.push_str("/* Calls the function whose entry is `addr`; 0 if there is none. */\n");
        entry.push_str("int snes_call_entry(Cpu *c, uint32_t addr) {\n    switch (addr & 0xFFFFFF) {\n");
        for &f in self.g.functions.keys() {
            entry.push_str(&format!("    case 0x{f:06X}: {}(c); return 1;\n", func_name(f)));
        }
        entry.push_str("    default: return 0;\n    }\n}\n");
        FullOutput { header, entry, banks, functions: self.g.functions.len(), unsupported }
    }
}

/// Why a function could not be translated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cause {
    /// Register widths differ among the ways it is reached, and the C depends on them.
    Widths,
    /// An instruction or addressing mode has no translation.
    NoTranslation,
    /// A jump or branch lands on something that is not code.
    NotCode,
    /// An indirect jump or call of a form the emitter does not support.
    Indirect,
    Other,
}

impl Cause {
    pub fn classify(message: &str) -> Cause {
        if message.contains("different register widths") {
            Cause::Widths
        } else if message.contains("has no translation") || message.contains("has no effective address") {
            Cause::NoTranslation
        } else if message.contains("which is not code") {
            Cause::NotCode
        } else if message.contains("unsupported indirect") {
            Cause::Indirect
        } else {
            Cause::Other
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            Cause::Widths => "register widths in conflict",
            Cause::NoTranslation => "no translation",
            Cause::NotCode => "jump to non-code",
            Cause::Indirect => "unsupported indirect form",
            Cause::Other => "other",
        }
    }
}

/// A function emitted as a stub.
#[derive(Debug)]
pub struct Unsupported {
    pub entry: u32,
    pub cause: Cause,
    pub message: String,
}

/// The C for a whole ROM.
#[derive(Debug)]
pub struct FullOutput {
    /// `snes_funcs.h`: declarations of every function.
    pub header: String,
    /// `snes_entry.c`: calls a function by the address of its entry.
    pub entry: String,
    /// C source per bank of the function entries.
    pub banks: BTreeMap<u8, String>,
    pub functions: usize,
    pub unsupported: Vec<Unsupported>,
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
        for extra in ["rt_check_flags", "rt_unresolved", "rt_unsupported", "rt_rti", "rt_pull8", "rt_pull16", "bus_read24_b0"] {
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
        Emitter::new(&d, &g).with_hooks(&cfg.hooks)?.generate(entry, depth)
    }

    fn gen_all(segments: &[(u32, &[u8])], cfg: &str, roots: &[u32]) -> FullOutput {
        let rom = test_rom(segments);
        let cfg = crate::config::Config::parse(cfg).unwrap();
        let r: Vec<_> = roots.iter().map(|&a| (a, Flags::RESET)).collect();
        let d = crate::discover::discover(&rom, &cfg, &r);
        let g = Graph::build(&d, roots);
        Emitter::new(&d, &g).with_hooks(&cfg.hooks).unwrap().generate_all()
    }

    /// Roots 00:8000 (widths in conflict, so it cannot be translated), 00:9000 and 01:8000.
    const ALL_ROM: [(u32, &[u8]); 3] = [
        (0x8000, &[0xF0, 0x02, 0xC2, 0x20, 0xA9, 0x12, 0x00, 0x60]),
        (0x9000, &[0xA9, 0x01, 0x60]),
        (0x1_8000, &[0x60]),
    ];

    #[test]
    fn generate_all_stubs_what_it_cannot_translate_and_emits_the_rest() {
        let out = gen_all(&ALL_ROM, "", &[0x8000, 0x9000, 0x1_8000]);
        assert_eq!(out.functions, 3);
        assert_eq!(out.unsupported.len(), 1);
        assert_eq!((out.unsupported[0].entry, out.unsupported[0].cause), (0x8000, Cause::Widths));
        assert!(out.unsupported[0].message.contains("00:8004"), "{}", out.unsupported[0].message);
        let b0 = &out.banks[&0];
        assert_eq!(body(b0, "f_00_8000"), ["rt_unsupported(c, 0x008000);"]);
        assert!(body(b0, "f_00_9000").contains(&"// 00:9000 LDA #$01".to_string()), "{b0}");
    }

    #[test]
    fn generate_all_splits_by_bank_and_defines_each_function_once() {
        let out = gen_all(&ALL_ROM, "", &[0x8000, 0x9000, 0x1_8000]);
        assert_eq!(out.banks.keys().copied().collect::<Vec<_>>(), [0, 1]);
        let all: String = out.banks.values().cloned().collect();
        for f in ["f_00_8000", "f_00_9000", "f_01_8000"] {
            assert_eq!(all.matches(&format!("void {f}(Cpu *c) {{")).count(), 1, "{f}");
            assert!(out.header.contains(&format!("void {f}(Cpu *c);")), "{f}");
        }
        assert!(out.banks[&1].contains("void f_01_8000(Cpu *c) {") && !out.banks[&0].contains("f_01_8000"));
        for text in out.banks.values() {
            assert!(text.contains("#include \"snes_funcs.h\""));
        }
        assert!(out.header.contains("#include \"snes_rt.h\""));
    }

    #[test]
    fn generate_all_is_deterministic() {
        let a = gen_all(&ALL_ROM, "", &[0x8000, 0x9000, 0x1_8000]);
        let b = gen_all(&ALL_ROM, "", &[0x8000, 0x9000, 0x1_8000]);
        assert_eq!(a.header, b.header);
        assert_eq!(a.banks, b.banks);
    }

    #[test]
    fn entry_dispatch_calls_a_function_by_address_and_rejects_the_rest() {
        // 00:8000: LDA #$42 ; STA $10 ; RTS     00:9000: LDA #$77 ; STA $10 ; RTS
        let rom = [(0x8000, &[0xA9, 0x42, 0x85, 0x10, 0x60][..]), (0x9000, &[0xA9, 0x77, 0x85, 0x10, 0x60][..])];
        let out = gen_all(&rom, "", &[0x8000, 0x9000]);
        assert!(out.entry.contains("int snes_call_entry(Cpu *c, uint32_t addr) {"));
        assert!(out.entry.contains("case 0x009000: f_00_9000(c); return 1;"), "{}", out.entry);
        // Only addresses and boilerplate: nothing is copied from the ROM.
        for l in out.entry.lines() {
            let known = l.is_empty()
                || l.starts_with("/*")
                || l.starts_with("#include")
                || l.starts_with("int snes_call_entry")
                || l.trim_start().starts_with("case 0x00")
                || ["    switch (addr & 0xFFFFFF) {", "    default: return 0;", "    }", "}"].contains(&l);
            assert!(known, "unexpected line: {l}");
        }

        let Some(cc) = crate::cctest::find() else {
            eprintln!("skipped: no C compiler found");
            return;
        };
        let main = "#include \"snes_funcs.h\"\n#include <stdio.h>\n\
            uint8_t bus_read8(uint32_t a);\n\
            int main(void) { Cpu c = {0}; int a, b; c.m = 1; c.xf = 1; c.s = 0x01FF;\n\
              a = snes_call_entry(&c, 0x009000); b = snes_call_entry(&c, 0x00A000);\n\
              printf(\"%d %d %02X\\n\", a, b, bus_read8(0x10)); return 0; }\n";
        let mut files: Vec<(String, &str)> = vec![
            ("snes_rt.h".into(), RUNTIME_HEADER),
            ("snes_funcs.h".into(), &out.header),
            ("snes_entry.c".into(), &out.entry),
            ("rt.c".into(), MOCK_RUNTIME),
            ("main.c".into(), main),
        ];
        files.extend(out.banks.iter().map(|(b, t)| (format!("rom_{b:02X}.c"), t.as_str())));
        let files: Vec<(&str, &str)> = files.iter().map(|(n, t)| (n.as_str(), *t)).collect();
        let run = cc.compile_run("entry", &files, &[]).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(run, "1 0 77\n");
    }

    #[test]
    fn stub_causes_are_classified_by_their_message() {
        let cases = [
            ("00:8004 is reached with different register widths (M=8 / M=16)", Cause::Widths),
            ("00:8000: FOO has no translation", Cause::NoTranslation),
            ("00:8000: addressing mode Imp has no effective address", Cause::NoTranslation),
            ("f_00_8000 jumps to 00:9000, which is not code", Cause::NotCode),
            ("00:8000: unsupported indirect JMP AbsInd", Cause::Indirect),
            ("something new", Cause::Other),
        ];
        for (msg, cause) in cases {
            assert_eq!(Cause::classify(msg), cause, "{msg}");
        }
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
        assert!(b.contains(&"c->ret = rt_pull16(c);".to_string()), "RTS leaves the address it pulled in c->ret");
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
        assert_eq!(b[at + 2], "if (c->s != s0 || c->ret != 0x8002) return;", "the call checks that the return is the one it expects");
        assert_eq!(b[at - 1], "uint16_t s0 = c->s;", "S is kept before the return address is pushed");
        // JSL also pushes the program bank first.
        let rom = [(0x8000, &[0x22, 0x10, 0x80, 0x01, 0x60][..]), (0x1_8010, &[0x6B][..])];
        let out = gen(&rom, "", &[0x8000], 0x8000, 1).unwrap();
        let b = body(&out, "f_00_8000");
        let at = b.iter().position(|l| l == "rt_push8(c, 0x00);").unwrap();
        assert_eq!(&b[at + 1..at + 3], ["rt_push16(c, 0x8003);", "f_01_8010(c);"]);
        let callee = body(&out, "f_01_8010");
        assert_eq!(callee.iter().filter(|l| *l == "c->ret = rt_pull16(c);").count(), 1);
        assert_eq!(callee.iter().filter(|l| *l == "(void)rt_pull8(c);").count(), 1, "RTL also drops the bank");
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
        assert_eq!(b.iter().filter(|l| l.starts_with("c->ret = rt_pull16")).count(), 1, "execution goes on to the RTS");
        assert!(b.contains(&"if (c->s != s0 || c->ret != 0x8002) return;".to_string()), "{b:?}");
    }

    /// A caller (REP #$30 ; JSR $8200 ; table $8010, $8020) and a helper that pops the return address,
    /// reads the table entry selected by the word at $10 and jumps to it with `JML [$0000]` (at 820F).
    const HELPER_CALLER: &[u8] = &[0xC2, 0x30, 0x20, 0x00, 0x82, 0x10, 0x80, 0x20, 0x80, 0x60, 0x00];
    const HELPER_BODY: &[u8] = &[
        0xC2, 0x30, 0x68, 0x1A, 0x85, 0x00, 0xA5, 0x10, 0xA8, 0xB1, 0x00, 0x85, 0x00, 0xE2, 0x30, 0xDC, 0x00, 0x00,
    ];
    const HELPER_CFG: &str = "[[inline_helper]]
addr=\"00:8200\"
exit=\"00:820F\"";

    #[test]
    fn inline_helper_exit_has_a_case_per_target_and_reports_the_rest() {
        let rom = [
            (0x8000, HELPER_CALLER),
            (0x8200, HELPER_BODY),
            (0x8010, &[0x60][..]),
            (0x8020, &[0x60][..]),
        ];
        let out = gen(&rom, HELPER_CFG, &[0x8000], 0x8000, 1).unwrap();
        let b = body(&out, "f_00_8200");
        assert!(b.contains(&"case 0x008010: f_00_8010(c); return;".to_string()), "{b:?}");
        assert!(b.contains(&"case 0x008020: f_00_8020(c); return;".to_string()), "{b:?}");
        assert!(b.contains(&"default: rt_unresolved(c, 0x00820F, t); return;".to_string()), "{b:?}");
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
    fn every_block_starts_with_the_tick_hook() {
        // 8000: BNE $8003 ; 8002: NOP ; 8003: RTS  (three blocks)
        let out = gen(&[(0x8000, &[0xD0, 0x01, 0xEA, 0x60])], "", &[0x8000], 0x8000, 0).unwrap();
        let b = body(&out, "f_00_8000");
        assert_eq!(b.iter().filter(|l| l.starts_with("rt_check_flags(")).count(), 3);
        assert_eq!(b.first().map(String::as_str), Some("rt_tick(c);"));
        for (i, l) in b.iter().enumerate() {
            if l.starts_with("rt_check_flags(") {
                assert_eq!(b[i - 1], "rt_tick(c);", "{b:?}");
            }
        }
        assert_eq!(b.iter().filter(|l| *l == "rt_tick(c);").count(), 3);
    }

    #[test]
    fn the_tick_hook_costs_nothing_without_snes_rt_tick_and_runs_per_block_execution_with_it() {
        let Some(cc) = crate::cctest::find() else {
            eprintln!("skipped: no C compiler found");
            return;
        };
        // LDX #$05 ; loop: DEX ; BNE loop ; RTS: the entry block, five runs of the loop block, the RTS block
        let rom = [(0x8000, &[0xA2, 0x05, 0xCA, 0xD0, 0xFD, 0x60][..])];
        let generated = gen(&rom, "", &[0x8000], 0x8000, 0).unwrap();
        let main = "#include \"snes_rt.h\"\n#include <stdio.h>\nvoid f_00_8000(Cpu *c);\nextern unsigned ticks;\n\
                    int main(void) { Cpu c = {0}; c.m = 1; c.xf = 1; c.s = 0x01FF; f_00_8000(&c); printf(\"ticks=%u\\n\", ticks); return 0; }\n";
        let poll = "#include \"snes_rt.h\"\nunsigned ticks;\nvoid rt_poll(Cpu *c) { (void)c; ticks++; }\n";
        let files = [
            ("snes_rt.h", RUNTIME_HEADER),
            ("rt.c", MOCK_RUNTIME),
            ("gen.c", generated.as_str()),
            ("poll.c", poll),
            ("main.c", main),
        ];
        let out = cc.compile_run("tick", &files, &["SNES_RT_TICK"]).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(out, "ticks=7\n");
        // Without the define the hook disappears: no rt_poll is needed and the program still links.
        let files = [("snes_rt.h", RUNTIME_HEADER), ("rt.c", MOCK_RUNTIME), ("gen.c", generated.as_str()), ("main.c", &main.replace("extern unsigned ticks;", "unsigned ticks;"))];
        let out = cc.compile_run("tick-off", &files, &[]).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(out, "ticks=0\n");
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
    fn a_block_reached_with_two_widths_is_emitted_once_per_width() {
        // 8000: SEP #$20 ; 8002: BEQ $8006 ; 8004: REP #$20 ; 8006: INC $0200 (M=8 or M=16) ; 8009: RTS
        let rom = [(0x8000, &[0xE2, 0x20, 0xF0, 0x02, 0xC2, 0x20, 0xEE, 0x00, 0x02, 0x60][..])];
        let out = gen(&rom, "", &[0x8000], 0x8000, 0).unwrap();
        let b = body(&out, "f_00_8000");
        assert!(b.contains(&"L_00_8006_m8x8:".to_string()), "{b:?}");
        // The 16-bit copy follows REP directly, so it needs no label.
        assert!(b.contains(&"rt_check_flags(c, 0x008006, 1, 1);".to_string()), "{b:?}");
        assert!(b.contains(&"rt_check_flags(c, 0x008006, 0, 1);".to_string()), "{b:?}");
        // The branch goes to the 8-bit copy; REP falls into the 16-bit one.
        assert!(b.iter().any(|l| l.contains("goto L_00_8006_m8x8;")), "{b:?}");
        // Blocks that are reached with one width keep their plain labels.
        assert!(!out.contains("L_00_8004_m"), "{out}");
    }

    #[test]
    fn the_copies_of_a_block_run_with_their_own_width() {
        // Same program: with Z set the branch skips REP and INC is 8-bit; without it INC is 16-bit.
        let rom = [(0x8000, &[0xE2, 0x20, 0xF0, 0x02, 0xC2, 0x20, 0xEE, 0x00, 0x02, 0x60][..])];
        let report = r#"//report
            printf("%02X%02X m=%d\n", bus_read8(0x201), bus_read8(0x200), c.m);"#;
        let Some(out) = exec("copies-8", &rom, "", &[0x8000], &format!("c.z = 1; bus_write8(0x200, 0xFF);{report}")) else { return };
        assert_eq!(out, "0000 m=1\n");
        let Some(out) = exec("copies-16", &rom, "", &[0x8000], &format!("c.z = 0; bus_write8(0x200, 0xFF);{report}")) else { return };
        assert_eq!(out, "0100 m=0\n");
    }

    #[test]
    fn a_function_entered_with_two_widths_picks_the_copy_of_the_cpu() {
        // 8000: JSR $8010 ; JSR $8020 ... 8010: SEP #$20 ; RTS ; entry 8030: INC $0200 ; RTS reached by two callers
        // 8000: SEP #$20 ; JSR $8030 ; REP #$20 ; JSR $8030 ; RTS      8030: INC $0200 ; RTS
        let rom = [
            (0x8000, &[0xE2, 0x20, 0x20, 0x30, 0x80, 0xC2, 0x20, 0x20, 0x30, 0x80, 0x60][..]),
            (0x8030, &[0xEE, 0x00, 0x02, 0x60][..]),
        ];
        let Some(out) = exec("entry-copies", &rom, "", &[0x8000], r#"bus_write8(0x200, 0xFF);//report
            printf("%02X%02X\n", bus_read8(0x201), bus_read8(0x200));"#) else { return };
        // 8-bit INC: FF -> 00 with no carry; then 16-bit INC: 0000 -> 0001.
        assert_eq!(out, "0001\n");
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
        exec_with(name, segments, cfg, roots, "", main_body)
    }

    /// `exec` with `extra` C code (the functions of hooks) in front of `main`.
    fn exec_with(name: &str, segments: &[(u32, &[u8])], cfg: &str, roots: &[u32], extra: &str, main_body: &str) -> Option<String> {
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
{extra}
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
            extra = extra,
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

    fn hook_cfg(extra: &str) -> String {
        extra.replace("HOOK", "[[hook]]")
    }

    #[test]
    fn a_function_without_hooks_is_emitted_as_before() {
        let rom = [(0x8000, &[0xA9, 0x01, 0x60][..])];
        let plain = gen(&rom, "", &[0x8000], 0x8000, 0).unwrap();
        assert!(!plain.contains("_body") && plain.contains("void f_00_8000(Cpu *c) {"), "{plain}");
        let other = hook_cfg("HOOK\naddr=\"00:9000\"\nkind=\"before\"\ncall=\"hk\"");
        let rom2 = [(0x8000, &[0xA9, 0x01, 0x60][..]), (0x9000, &[0x60][..])];
        let hooked = gen(&rom2, &other, &[0x8000, 0x9000], 0x8000, 0).unwrap();
        assert!(hooked.contains("void hk(Cpu *c);"), "the hook function is declared");
        assert!(!hooked.contains("f_00_8000_body"), "a function nobody hooks keeps its shape");
    }

    #[test]
    fn a_hook_on_something_that_is_not_a_function_is_an_error() {
        let rom = [(0x8000, &[0x60][..])];
        let cfg = hook_cfg("HOOK\naddr=\"00:8001\"\nkind=\"before\"\ncall=\"hk\"");
        let e = gen(&rom, &cfg, &[0x8000], 0x8000, 0).unwrap_err();
        assert!(e.contains("00:8001") && e.contains("hk"), "{e}");
    }

    #[test]
    fn a_before_hook_sees_the_state_at_entry_and_an_after_hook_the_state_at_the_return() {
        // LDA #$01 ; STA $10 ; RTS
        let rom = [(0x8000, &[0xA9, 0x01, 0x85, 0x10, 0x60][..])];
        let cfg = hook_cfg("HOOK\naddr=\"00:8000\"\nkind=\"before\"\ncall=\"hk_before\"\nHOOK\naddr=\"00:8000\"\nkind=\"after\"\ncall=\"hk_after\"");
        let extra = "void hk_before(Cpu *c) { bus_write8(0x20, bus_read8(0x10)); }\nvoid hk_after(Cpu *c) { bus_write8(0x21, bus_read8(0x10)); }";
        let Some(out) = exec_with("hook-before-after", &rom, &cfg, &[0x8000], extra, r#"bus_write8(0x10, 7);
            //report
            printf("before=%02X after=%02X s=%04X\n", bus_read8(0x20), bus_read8(0x21), c.s);"#) else { return };
        assert_eq!(out, "before=07 after=01 s=0201\n");
    }

    #[test]
    fn an_after_hook_runs_for_each_of_the_returns_of_the_function() {
        // LDA $30 ; BEQ +3 ; INC $12 ; RTS ; INC $13 ; RTS
        let rom = [(0x8000, &[0xA5, 0x30, 0xF0, 0x03, 0xE6, 0x12, 0x60, 0xE6, 0x13, 0x60][..])];
        let cfg = hook_cfg("HOOK\naddr=\"00:8000\"\nkind=\"after\"\ncall=\"hk_after\"");
        let extra = "void hk_after(Cpu *c) { bus_write8(0x20, bus_read8(0x20) + 1); }";
        let Some(out) = exec_with("hook-after-returns", &rom, &cfg, &[0x8000], extra, r#"bus_write8(0x30, 1);
            //report
            bus_write8(0x30, 0); c.s = 0x01FF; f_00_8000(&c);
            printf("calls=%02X path1=%02X path2=%02X\n", bus_read8(0x20), bus_read8(0x12), bus_read8(0x13));"#) else { return };
        assert_eq!(out, "calls=02 path1=01 path2=01\n");
    }

    #[test]
    fn a_replace_hook_stands_in_for_the_body_and_returns_like_an_rts() {
        // 8100: JSR $8000 ; STA $11 ; RTS      8000: LDA #$01 ; STA $10 ; RTS (replaced)
        let rom = [(0x8100, &[0x20, 0x00, 0x80, 0x85, 0x11, 0x60][..]), (0x8000, &[0xA9, 0x01, 0x85, 0x10, 0x60][..])];
        let cfg = hook_cfg("HOOK\naddr=\"00:8000\"\nkind=\"replace\"\ncall=\"hk_rep\"\nret=\"rts\"");
        let extra = "void hk_rep(Cpu *c) { bus_write8(0x10, 0x99); c->a = 0x42; }";
        let Some(out) = exec_with("hook-replace-rts", &rom, &cfg, &[0x8100, 0x8000], extra, r#"//report
            printf("m10=%02X m11=%02X s=%04X\n", bus_read8(0x10), bus_read8(0x11), c.s);"#) else { return };
        assert_eq!(out, "m10=99 m11=42 s=0201\n");
    }

    #[test]
    fn a_replace_hook_with_rtl_pulls_the_bank_too() {
        // 8100: JSL $018000 ; STA $11 ; RTS      01:8000: RTL (replaced)
        let rom = [(0x8100, &[0x22, 0x00, 0x80, 0x01, 0x85, 0x11, 0x60][..]), (0x1_8000, &[0x6B][..])];
        let cfg = hook_cfg("HOOK\naddr=\"01:8000\"\nkind=\"replace\"\ncall=\"hk_rep\"\nret=\"rtl\"");
        let extra = "void hk_rep(Cpu *c) { c->a = 0x37; }";
        let Some(out) = exec_with("hook-replace-rtl", &rom, &cfg, &[0x8100, 0x1_8000], extra, r#"//report
            printf("m11=%02X s=%04X\n", bus_read8(0x11), c.s);"#) else { return };
        assert_eq!(out, "m11=37 s=0201\n");
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
    fn rts_and_rtl_leave_the_address_they_pulled_in_ret() {
        // RTS with $1234 on top of the stack, and RTL with $1234 and a bank below it.
        let Some(out) = exec("ret-rts", &[(0x8000, &[0x60][..])], "", &[0x8000], r#"load(0x200, (const uint8_t[]){0x34, 0x12}, 2);
            //report
            printf("ret=%04X s=%04X\n", (unsigned)c.ret, c.s);"#) else { return };
        assert_eq!(out, "ret=1234 s=0201\n");
        let Some(out) = exec("ret-rtl", &[(0x8000, &[0x6B][..])], "", &[0x8000], r#"load(0x200, (const uint8_t[]){0x34, 0x12, 0x05}, 3);
            //report
            printf("ret=%04X s=%04X\n", (unsigned)c.ret, c.s);"#) else { return };
        assert_eq!(out, "ret=1234 s=0202\n");
    }

    /// 8000: JSR $8010 ; LDA #$11 ; STA $10 ; RTS     8010: JSR $8020 ; LDA #$22 ; STA $11 ; RTS
    /// 8020: PLA ; PLA ; RTS (drops its own return address, so the RTS goes back to the caller of $8010)
    const DROPS_A_RETURN: [(u32, &[u8]); 3] = [
        (0x8000, &[0x20, 0x10, 0x80, 0xA9, 0x11, 0x85, 0x10, 0x60]),
        (0x8010, &[0x20, 0x20, 0x80, 0xA9, 0x22, 0x85, 0x11, 0x60]),
        (0x8020, &[0x68, 0x68, 0x60]),
    ];

    #[test]
    fn a_return_that_drops_an_address_reaches_the_outer_caller() {
        let Some(out) = exec("drop-return", &DROPS_A_RETURN, "", &[0x8000], r#"//report
            printf("m10=%02X m11=%02X s=%04X\n", bus_read8(0x10), bus_read8(0x11), c.s);"#) else { return };
        // Only the outer caller goes on: the middle function does not run what follows its call, and the
        // stack ends where the CPU would leave it.
        assert_eq!(out, "m10=11 m11=00 s=0201\n");
    }

    #[test]
    fn a_return_to_another_address_with_the_same_stack_is_not_taken_as_the_expected_one() {
        // 8000: JSR $8010 ; LDA #$11 ; STA $10 ; RTS     8010: PLA ; PLA ; PEA $8FFF ; RTS (returns to $9000)
        let rom = [
            (0x8000, &[0x20, 0x10, 0x80, 0xA9, 0x11, 0x85, 0x10, 0x60][..]),
            (0x8010, &[0x68, 0x68, 0xF4, 0xFF, 0x8F, 0x60][..]),
        ];
        let Some(out) = exec("other-address", &rom, "", &[0x8000], r#"//report
            printf("m10=%02X ret=%04X s=%04X\n", bus_read8(0x10), (unsigned)c.ret, c.s);"#) else { return };
        assert_eq!(out, "m10=00 ret=8FFF s=01FF\n");
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
    fn a_pha_rts_dispatch_site_also_works_as_a_plain_return() {
        // 8000: JSR $8010 ; LDA #$55 ; STA $11 ; RTS
        // 8010: LDA $10 ; BEQ $801A ; LDA #$80 ; PHA ; LDA #$2F ; PHA ; 801A: RTS    table: $8030    8030: LDA #$77 ; STA $12 ; RTS
        let rom = [
            (0x8000, &[0x20, 0x10, 0x80, 0xA9, 0x55, 0x85, 0x11, 0x60][..]),
            (0x8010, &[0xA5, 0x10, 0xF0, 0x06, 0xA9, 0x80, 0x48, 0xA9, 0x2F, 0x48, 0x60][..]),
            (0x8030, &[0xA9, 0x77, 0x85, 0x12, 0x60][..]),
            (0x8100, &[0x30, 0x80][..]),
        ];
        let cfg = "[[table]]\nkind=\"indirect\"\nsite=\"00:801A\"\nat=\"00:8100\"\ncount=1";
        for (flag, want, name) in [(1u8, "mem=77 55", "pharts_dispatch"), (0u8, "mem=00 55", "pharts_return")] {
            let body = format!(
                r#"{{ const uint8_t v[] = {{{flag}}}, t[] = {{0x30, 0x80}}; load(0x10, v, 1); load(0x8100, t, 2); }}
            //report
            printf("mem=%02X %02X s=%04X\n", bus_read8(0x12), bus_read8(0x11), c.s);"#
            );
            let Some(out) = exec(name, &rom, cfg, &[0x8000], &body) else { return };
            assert_eq!(out, format!("{want} s=0201\n"));
        }
    }

    #[test]
    fn runs_the_target_chosen_by_an_inline_helper() {
        let rom = [
            (0x8000, HELPER_CALLER),
            (0x8200, HELPER_BODY),
            (0x8010, &[0xA9, 0x11, 0x85, 0x12, 0x60][..]),
            (0x8020, &[0xA9, 0x77, 0x85, 0x12, 0x60][..]),
        ];
        // The helper already popped the call's return address, so the target's RTS pops the caller's.
        for (index, want, name) in [(0u8, "11", "helper_first"), (2u8, "77", "helper_second")] {
            let body = format!(
                r#"{{ const uint8_t v[] = {{{index}}}, t[] = {{0x10, 0x80, 0x20, 0x80}}; load(0x10, v, 1); load(0x8005, t, 4); }}
            //report
            printf("mem=%02X s=%04X\n", bus_read8(0x12), c.s);"#
            );
            let Some(out) = exec(name, &rom, HELPER_CFG, &[0x8000], &body) else { return };
            assert_eq!(out, format!("mem={want} s=0201\n"));
        }
        // A pointer the recompiler never saw is reported at the helper's exit.
        let body = r#"{ const uint8_t v[] = {2}, t[] = {0x10, 0x80, 0x00, 0x90}; load(0x10, v, 1); load(0x8005, t, 4); }
            //report
            printf("mem=%02X\n", bus_read8(0x12));"#;
        let Some(out) = exec("helper_unknown", &rom, HELPER_CFG, &[0x8000], body) else { return };
        assert_eq!(out, "UNRESOLVED site=00820F target=009000\nmem=00\n");
    }

    #[test]
    fn starts_at_the_entry_even_when_a_lower_block_belongs_to_the_function() {
        // The callee at $8010 jumps back to a block at $8000 that only it reaches.
        let rom = [
            (0x9000, &[0x20, 0x10, 0x80, 0x60][..]),
            (0x8010, &[0xA9, 0x01, 0x85, 0x10, 0x4C, 0x00, 0x80][..]),
            (0x8000, &[0xA9, 0x22, 0x85, 0x11, 0x60][..]),
        ];
        let Some(out) = exec("entry_not_lowest", &rom, "", &[0x9000], r#"//report
            printf("a=%02X b=%02X\n", bus_read8(0x10), bus_read8(0x11));"#) else { return };
        assert_eq!(out, "a=01 b=22\n");
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
void rt_rti(Cpu *c) { rt_rti_pop(c); }
void rt_lost_return(Cpu *c) { (void)c; printf("LOST_RETURN\n"); exit(8); }
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

    /// Recompile `entry` of A Link to the Past with its callees down to `depth` and run it against the
    /// real ROM (`SNES_GAME_PROFILE` of `alttp_usa`, `SNES_GAME_ROM`); `setup` is C that runs on the CPU state right
    /// after reset, and the result is the output of the run. `None` when there is no C compiler or that game.
    fn run_alttp(name: &str, entry: u32, depth: usize, setup: &dyn Fn(Flags) -> String) -> Option<String> {
        let Some(cc) = crate::cctest::find() else {
            eprintln!("skipped: no C compiler found");
            return None;
        };
        let game = crate::cctest::real_game_id("alttp_usa")?;
        let rom = game.rom;
        let cfg = crate::config::Config::parse(&game.profile.config).unwrap();
        let roots: Vec<(u32, Flags)> = cfg.entries.iter().map(|e| (e.addr, e.flags)).collect();
        let d = crate::discover::discover(&rom, &cfg, &roots);
        let addrs: Vec<u32> = roots.iter().map(|r| r.0).collect();
        let g = Graph::build(&d, &addrs);
        let generated = Emitter::new(&d, &g).generate(entry, depth).unwrap();
        let flags = d.flags[&entry];

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
        .replace("#include <stdlib.h>", "#include <stdlib.h>\n#include <string.h>\nextern const unsigned char rom_image[];")
        .replace("void f_00_8000(Cpu *c);", &format!("void {}(Cpu *c);", func_name(entry)))
        .replace("    f_00_8000(&c);", &format!("    {}\n    {}(&c);", setup(flags), func_name(entry)));
        // Functions that are only declared get a stub: reaching one means the run left the slice.
        let stubs: String = generated
            .lines()
            .filter_map(|l| l.strip_prefix("void ")?.strip_suffix("(Cpu *c);"))
            .filter(|name| !generated.contains(&format!("void {name}(Cpu *c) {{")))
            .map(|name| {
                format!("void {name}(Cpu *c) {{ (void)c; printf(\"UNEMITTED {name}\\n\"); exit(0); }}\n")
            })
            .collect();
        let stubs = format!("#include \"snes_rt.h\"
#include <stdio.h>
#include <stdlib.h>
{stubs}");
        let out = cc
            .compile_run(
                name,
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
        Some(out)
    }

    /// The whole of the real game (`SNES_GAME_PROFILE`, `SNES_GAME_ROM`), generated with `generate_all`, compiles and
    /// links against the mock runtime. Skipped without the game or a C compiler.
    #[test]
    fn alttp_full_rom_compiles_and_links() {
        let Some(cc) = crate::cctest::find() else {
            eprintln!("skipped: no C compiler found");
            return;
        };
        let Some(game) = crate::cctest::real_game() else { return };
        let rom = game.rom;
        let cfg = crate::config::Config::parse(&game.profile.config).unwrap();
        let roots: Vec<(u32, Flags)> = cfg.entries.iter().map(|e| (e.addr, e.flags)).collect();
        let d = crate::discover::discover(&rom, &cfg, &roots);
        let addrs: Vec<u32> = roots.iter().map(|r| r.0).collect();
        let g = Graph::build(&d, &addrs);
        let all = Emitter::new(&d, &g).generate_all();
        eprintln!(
            "{} functions, {} stubs, {} banks, {} lines of C",
            all.functions,
            all.unsupported.len(),
            all.banks.len(),
            all.banks.values().map(|t| t.lines().count()).sum::<usize>()
        );
        assert!(all.functions > 2000, "{} functions", all.functions);
        assert!(all.unsupported.len() <= 7, "{:?}", all.unsupported);

        let mut files: Vec<(String, &str)> = vec![
            ("snes_rt.h".into(), RUNTIME_HEADER),
            ("snes_funcs.h".into(), &all.header),
            ("rt.c".into(), MOCK_RUNTIME),
            ("main.c".into(), "int main(void) { return 0; }\n"),
        ];
        files.extend(all.banks.iter().map(|(bank, text)| (format!("rom_{bank:02X}.c"), text.as_str())));
        let files: Vec<(&str, &str)> = files.iter().map(|(n, t)| (n.as_str(), *t)).collect();
        let started = std::time::Instant::now();
        let out = cc.compile_run("alttp-full", &files, &["SNES_RT_CHECKS"]).unwrap_or_else(|e| panic!("{e}"));
        eprintln!("compiled and linked in {:?}", started.elapsed());
        assert_eq!(out, "");
    }

    /// 02:8C14 of A Link to the Past calls the inline helper at 00:8781 with the index in $B0. Each index
    /// has to reach its own target (02:8CA9, 02:8D01, 02:8CE2), which runs until it calls a function the
    /// slice does not emit; the helper's exit never reports a target as unresolved.
    #[test]
    fn alttp_inline_helper_call_reaches_its_target() {
        for (index, first_callee) in [(0u8, "f_02_8CAC"), (1, "f_00_E914"), (2, "f_02_A2F0")] {
            let Some(out) = run_alttp("alttp-helper", 0x02_8C14, 2, &|f| {
                format!("c.e = 0; c.m = {}; c.xf = {}; wram[0xB0] = {index};", f.m as u8, f.x as u8)
            }) else { return };
            assert_eq!(out, format!("UNEMITTED {first_callee}\n"), "index {index}");
        }
    }
}
