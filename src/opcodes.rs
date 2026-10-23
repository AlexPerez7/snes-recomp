//! 65816 opcode table: mnemonic and addressing mode for each of the 256 opcodes.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Imp,
    Acc,
    /// Immediate whose width follows the M flag.
    ImmM,
    /// Immediate whose width follows the X flag.
    ImmX,
    Imm8,
    Dp,
    DpX,
    DpY,
    Abs,
    AbsX,
    AbsY,
    Long,
    LongX,
    DpInd,
    DpIndX,
    DpIndY,
    DpIndLong,
    DpIndLongY,
    Sr,
    SrIndY,
    AbsInd,
    AbsIndX,
    AbsIndLong,
    Rel8,
    Rel16,
    Block,
}

use Mode::*;

pub const OPCODES: [(&str, Mode); 256] = [
    // 0x00
    ("BRK", Imm8), ("ORA", DpIndX), ("COP", Imm8), ("ORA", Sr),
    ("TSB", Dp), ("ORA", Dp), ("ASL", Dp), ("ORA", DpIndLong),
    ("PHP", Imp), ("ORA", ImmM), ("ASL", Acc), ("PHD", Imp),
    ("TSB", Abs), ("ORA", Abs), ("ASL", Abs), ("ORA", Long),
    // 0x10
    ("BPL", Rel8), ("ORA", DpIndY), ("ORA", DpInd), ("ORA", SrIndY),
    ("TRB", Dp), ("ORA", DpX), ("ASL", DpX), ("ORA", DpIndLongY),
    ("CLC", Imp), ("ORA", AbsY), ("INC", Acc), ("TCS", Imp),
    ("TRB", Abs), ("ORA", AbsX), ("ASL", AbsX), ("ORA", LongX),
    // 0x20
    ("JSR", Abs), ("AND", DpIndX), ("JSL", Long), ("AND", Sr),
    ("BIT", Dp), ("AND", Dp), ("ROL", Dp), ("AND", DpIndLong),
    ("PLP", Imp), ("AND", ImmM), ("ROL", Acc), ("PLD", Imp),
    ("BIT", Abs), ("AND", Abs), ("ROL", Abs), ("AND", Long),
    // 0x30
    ("BMI", Rel8), ("AND", DpIndY), ("AND", DpInd), ("AND", SrIndY),
    ("BIT", DpX), ("AND", DpX), ("ROL", DpX), ("AND", DpIndLongY),
    ("SEC", Imp), ("AND", AbsY), ("DEC", Acc), ("TSC", Imp),
    ("BIT", AbsX), ("AND", AbsX), ("ROL", AbsX), ("AND", LongX),
    // 0x40
    ("RTI", Imp), ("EOR", DpIndX), ("WDM", Imm8), ("EOR", Sr),
    ("MVP", Block), ("EOR", Dp), ("LSR", Dp), ("EOR", DpIndLong),
    ("PHA", Imp), ("EOR", ImmM), ("LSR", Acc), ("PHK", Imp),
    ("JMP", Abs), ("EOR", Abs), ("LSR", Abs), ("EOR", Long),
    // 0x50
    ("BVC", Rel8), ("EOR", DpIndY), ("EOR", DpInd), ("EOR", SrIndY),
    ("MVN", Block), ("EOR", DpX), ("LSR", DpX), ("EOR", DpIndLongY),
    ("CLI", Imp), ("EOR", AbsY), ("PHY", Imp), ("TCD", Imp),
    ("JML", Long), ("EOR", AbsX), ("LSR", AbsX), ("EOR", LongX),
    // 0x60
    ("RTS", Imp), ("ADC", DpIndX), ("PER", Rel16), ("ADC", Sr),
    ("STZ", Dp), ("ADC", Dp), ("ROR", Dp), ("ADC", DpIndLong),
    ("PLA", Imp), ("ADC", ImmM), ("ROR", Acc), ("RTL", Imp),
    ("JMP", AbsInd), ("ADC", Abs), ("ROR", Abs), ("ADC", Long),
    // 0x70
    ("BVS", Rel8), ("ADC", DpIndY), ("ADC", DpInd), ("ADC", SrIndY),
    ("STZ", DpX), ("ADC", DpX), ("ROR", DpX), ("ADC", DpIndLongY),
    ("SEI", Imp), ("ADC", AbsY), ("PLY", Imp), ("TDC", Imp),
    ("JMP", AbsIndX), ("ADC", AbsX), ("ROR", AbsX), ("ADC", LongX),
    // 0x80
    ("BRA", Rel8), ("STA", DpIndX), ("BRL", Rel16), ("STA", Sr),
    ("STY", Dp), ("STA", Dp), ("STX", Dp), ("STA", DpIndLong),
    ("DEY", Imp), ("BIT", ImmM), ("TXA", Imp), ("PHB", Imp),
    ("STY", Abs), ("STA", Abs), ("STX", Abs), ("STA", Long),
    // 0x90
    ("BCC", Rel8), ("STA", DpIndY), ("STA", DpInd), ("STA", SrIndY),
    ("STY", DpX), ("STA", DpX), ("STX", DpY), ("STA", DpIndLongY),
    ("TYA", Imp), ("STA", AbsY), ("TXS", Imp), ("TXY", Imp),
    ("STZ", Abs), ("STA", AbsX), ("STZ", AbsX), ("STA", LongX),
    // 0xA0
    ("LDY", ImmX), ("LDA", DpIndX), ("LDX", ImmX), ("LDA", Sr),
    ("LDY", Dp), ("LDA", Dp), ("LDX", Dp), ("LDA", DpIndLong),
    ("TAY", Imp), ("LDA", ImmM), ("TAX", Imp), ("PLB", Imp),
    ("LDY", Abs), ("LDA", Abs), ("LDX", Abs), ("LDA", Long),
    // 0xB0
    ("BCS", Rel8), ("LDA", DpIndY), ("LDA", DpInd), ("LDA", SrIndY),
    ("LDY", DpX), ("LDA", DpX), ("LDX", DpY), ("LDA", DpIndLongY),
    ("CLV", Imp), ("LDA", AbsY), ("TSX", Imp), ("TYX", Imp),
    ("LDY", AbsX), ("LDA", AbsX), ("LDX", AbsY), ("LDA", LongX),
    // 0xC0
    ("CPY", ImmX), ("CMP", DpIndX), ("REP", Imm8), ("CMP", Sr),
    ("CPY", Dp), ("CMP", Dp), ("DEC", Dp), ("CMP", DpIndLong),
    ("INY", Imp), ("CMP", ImmM), ("DEX", Imp), ("WAI", Imp),
    ("CPY", Abs), ("CMP", Abs), ("DEC", Abs), ("CMP", Long),
    // 0xD0
    ("BNE", Rel8), ("CMP", DpIndY), ("CMP", DpInd), ("CMP", SrIndY),
    ("PEI", DpInd), ("CMP", DpX), ("DEC", DpX), ("CMP", DpIndLongY),
    ("CLD", Imp), ("CMP", AbsY), ("PHX", Imp), ("STP", Imp),
    ("JML", AbsIndLong), ("CMP", AbsX), ("DEC", AbsX), ("CMP", LongX),
    // 0xE0
    ("CPX", ImmX), ("SBC", DpIndX), ("SEP", Imm8), ("SBC", Sr),
    ("CPX", Dp), ("SBC", Dp), ("INC", Dp), ("SBC", DpIndLong),
    ("INX", Imp), ("SBC", ImmM), ("NOP", Imp), ("XBA", Imp),
    ("CPX", Abs), ("SBC", Abs), ("INC", Abs), ("SBC", Long),
    // 0xF0
    ("BEQ", Rel8), ("SBC", DpIndY), ("SBC", DpInd), ("SBC", SrIndY),
    ("PEA", Abs), ("SBC", DpX), ("INC", DpX), ("SBC", DpIndLongY),
    ("SED", Imp), ("SBC", AbsY), ("PLX", Imp), ("XCE", Imp),
    ("JSR", AbsIndX), ("SBC", AbsX), ("INC", AbsX), ("SBC", LongX),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spot_checks() {
        assert_eq!(OPCODES[0xEA], ("NOP", Imp));
        assert_eq!(OPCODES[0x6B], ("RTL", Imp));
        assert_eq!(OPCODES[0xA9], ("LDA", ImmM));
        assert_eq!(OPCODES[0xC2], ("REP", Imm8));
        assert_eq!(OPCODES[0xFC], ("JSR", AbsIndX));
        assert_eq!(OPCODES[0x5C], ("JML", Long));
    }

    #[test]
    fn mnemonic_counts() {
        let count = |m: &str| OPCODES.iter().filter(|(n, _)| *n == m).count();
        // 15 addressing forms of the classic ALU ops, plus the 16th with immediate
        assert_eq!(count("LDA"), 15);
        assert_eq!(count("STA"), 14);
        assert_eq!(count("ORA"), 15);
    }
}
