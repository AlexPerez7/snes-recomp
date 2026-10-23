//! 65816 instruction decoder and linear disassembly.

use crate::opcodes::{Mode, OPCODES};
use crate::rom::Rom;

/// Register widths. `true` means 8-bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Flags {
    pub m: bool,
    pub x: bool,
}

impl Flags {
    /// State right after reset: emulation mode, both registers 8-bit.
    pub const RESET: Flags = Flags { m: true, x: true };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Next,
    /// Conditional branch: both the target and the fall-through are reachable.
    Branch(u32),
    Jump(u32),
    Call(u32),
    IndirectJump,
    IndirectCall,
    Return,
    Stop,
}

#[derive(Debug, Clone)]
pub struct Instr {
    pub addr: u32,
    pub opcode: u8,
    pub mnemonic: &'static str,
    pub mode: Mode,
    pub len: u8,
    pub operand: u32,
}

fn operand_len(mode: Mode, flags: Flags) -> u8 {
    match mode {
        Mode::Imp | Mode::Acc => 0,
        Mode::ImmM => if flags.m { 1 } else { 2 },
        Mode::ImmX => if flags.x { 1 } else { 2 },
        Mode::Imm8 | Mode::Dp | Mode::DpX | Mode::DpY | Mode::DpInd | Mode::DpIndX
        | Mode::DpIndY | Mode::DpIndLong | Mode::DpIndLongY | Mode::Sr | Mode::SrIndY
        | Mode::Rel8 => 1,
        Mode::Abs | Mode::AbsX | Mode::AbsY | Mode::AbsInd | Mode::AbsIndX
        | Mode::AbsIndLong | Mode::Rel16 | Mode::Block => 2,
        Mode::Long | Mode::LongX => 3,
    }
}

pub fn decode(rom: &Rom, addr: u32, flags: Flags) -> Option<Instr> {
    let opcode = rom.read8(addr)?;
    let (mnemonic, mode) = OPCODES[opcode as usize];
    let n = operand_len(mode, flags);
    let mut operand = 0u32;
    for i in 0..n as u32 {
        operand |= (rom.read8((addr + 1 + i) & 0xFF_FFFF)? as u32) << (8 * i);
    }
    Some(Instr { addr, opcode, mnemonic, mode, len: 1 + n, operand })
}

impl Instr {
    fn bank(&self) -> u32 {
        self.addr & 0xFF_0000
    }

    /// Destination of a relative branch.
    fn rel_target(&self) -> u32 {
        let disp = match self.mode {
            Mode::Rel8 => self.operand as u8 as i8 as i32,
            _ => self.operand as u16 as i16 as i32,
        };
        let next = (self.addr & 0xFFFF) as i32 + self.len as i32;
        self.bank() | ((next + disp) as u32 & 0xFFFF)
    }

    pub fn flow(&self) -> Flow {
        match self.mnemonic {
            "BPL" | "BMI" | "BVC" | "BVS" | "BCC" | "BCS" | "BNE" | "BEQ" => Flow::Branch(self.rel_target()),
            "BRA" | "BRL" => Flow::Jump(self.rel_target()),
            "JMP" => match self.mode {
                Mode::Abs => Flow::Jump(self.bank() | self.operand),
                _ => Flow::IndirectJump,
            },
            "JML" => match self.mode {
                Mode::Long => Flow::Jump(self.operand),
                _ => Flow::IndirectJump,
            },
            "JSR" => match self.mode {
                Mode::Abs => Flow::Call(self.bank() | self.operand),
                _ => Flow::IndirectCall,
            },
            "JSL" => Flow::Call(self.operand),
            "RTS" | "RTL" | "RTI" => Flow::Return,
            "BRK" | "COP" | "STP" => Flow::Stop,
            _ => Flow::Next,
        }
    }

    /// Register widths in effect after this instruction (REP/SEP only; PLP/RTI are not tracked).
    pub fn next_flags(&self, f: Flags) -> Flags {
        let mask = self.operand as u8;
        match self.mnemonic {
            "REP" => Flags { m: f.m && mask & 0x20 == 0, x: f.x && mask & 0x10 == 0 },
            "SEP" => Flags { m: f.m || mask & 0x20 != 0, x: f.x || mask & 0x10 != 0 },
            _ => f,
        }
    }

    pub fn operand_text(&self) -> String {
        let o = self.operand;
        let w = (self.len as usize - 1) * 2;
        match self.mode {
            Mode::Imp => String::new(),
            Mode::Acc => "A".into(),
            Mode::ImmM | Mode::ImmX | Mode::Imm8 => format!("#${o:0w$X}"),
            Mode::Dp | Mode::Abs | Mode::Long => format!("${o:0w$X}"),
            Mode::DpX | Mode::AbsX | Mode::LongX => format!("${o:0w$X},X"),
            Mode::DpY | Mode::AbsY => format!("${o:0w$X},Y"),
            Mode::DpInd => format!("(${o:02X})"),
            Mode::DpIndX => format!("(${o:02X},X)"),
            Mode::DpIndY => format!("(${o:02X}),Y"),
            Mode::DpIndLong => format!("[${o:02X}]"),
            Mode::DpIndLongY => format!("[${o:02X}],Y"),
            Mode::Sr => format!("${o:02X},S"),
            Mode::SrIndY => format!("(${o:02X},S),Y"),
            Mode::AbsInd => format!("(${o:04X})"),
            Mode::AbsIndX => format!("(${o:04X},X)"),
            Mode::AbsIndLong => format!("[${o:04X}]"),
            Mode::Rel8 | Mode::Rel16 => format!("${:06X}", self.rel_target()),
            // MVN/MVP encode destination first; assembler syntax lists source first.
            Mode::Block => format!("${:02X},${:02X}", o >> 8, o & 0xFF),
        }
    }

    pub fn text(&self) -> String {
        let op = self.operand_text();
        if op.is_empty() { self.mnemonic.to_string() } else { format!("{} {}", self.mnemonic, op) }
    }
}

pub fn fmt_addr(addr: u32) -> String {
    format!("{:02X}:{:04X}", addr >> 16, addr & 0xFFFF)
}

/// Decode `count` instructions in sequence starting at `addr`.
pub fn linear(rom: &Rom, mut addr: u32, mut flags: Flags, count: usize) -> Vec<Instr> {
    let mut out = Vec::new();
    while out.len() < count {
        let Some(ins) = decode(rom, addr, flags) else { break };
        flags = ins.next_flags(flags);
        addr = (addr & 0xFF_0000) | ((addr + ins.len as u32) & 0xFFFF);
        out.push(ins);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rom::test_rom;

    #[test]
    fn linear_tracks_rep_sep() {
        // SEI; CLC; XCE; REP #$30; LDA #$1234; LDX #$5678; SEP #$20; LDA #$FF
        let rom = test_rom(&[(0x8000, &[
            0x78, 0x18, 0xFB, 0xC2, 0x30, 0xA9, 0x34, 0x12, 0xA2, 0x78, 0x56, 0xE2, 0x20, 0xA9, 0xFF,
        ])]);
        let text: Vec<_> = linear(&rom, 0x8000, Flags::RESET, 8).iter().map(Instr::text).collect();
        assert_eq!(
            text,
            ["SEI", "CLC", "XCE", "REP #$30", "LDA #$1234", "LDX #$5678", "SEP #$20", "LDA #$FF"]
        );
    }

    #[test]
    fn branch_and_call_targets() {
        // 00:8000 BNE +2 -> 8004 ; 8002 JSR $9000 ; 8005 RTS
        let rom = test_rom(&[(0x8000, &[0xD0, 0x02, 0x20, 0x00, 0x90, 0x60])]);
        let bne = decode(&rom, 0x8000, Flags::RESET).unwrap();
        assert_eq!(bne.flow(), Flow::Branch(0x8004));
        let jsr = decode(&rom, 0x8002, Flags::RESET).unwrap();
        assert_eq!(jsr.flow(), Flow::Call(0x9000));
        assert_eq!(jsr.text(), "JSR $9000");
    }
}
