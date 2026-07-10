//this is for adding instructions on register data. First we need to
//define all the instructions(I got this list from the github article I'm using)
//and then we need to define the targets for those instructions. some instructions
//operate on 8 bit registers, some operate on 16 bit register pairs, and some
//operate on a single bit of a register.
pub enum Instruction {
    ADD(ArithmeticTarget),
    ADDHL(ADDHLTarget),
    ADC(ArithmeticTarget),
    SUB(ArithmeticTarget),
    SBC(ArithmeticTarget),
    AND(ArithmeticTarget),
    OR(ArithmeticTarget),
    XOR(ArithmeticTarget),
    CP(ArithmeticTarget),
    INC(ArithmeticTarget),
    DEC(ArithmeticTarget),
    CCF,
    SCF,
    RRA,
    RLA,
    RRCA,
    RLCA,
    CPL,
    BIT(ArithmeticTarget, BitPosition),
    RESET(ArithmeticTarget, BitPosition),
    SET(ArithmeticTarget, BitPosition),
    SRL(ArithmeticTarget),
    RR(ArithmeticTarget),
    RL(ArithmeticTarget),
    RRC(ArithmeticTarget),
    RLC(ArithmeticTarget),
    SRA(ArithmeticTarget),
    SLA(ArithmeticTarget),
    SWAP(ArithmeticTarget),
}

#[derive(Clone, Copy)]
pub enum ArithmeticTarget {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
}

//ADDHL adds a 16 bit register pair into HL, so it needs its own target list instead of ArithmeticTarget
#[derive(Clone, Copy)]
pub enum ADDHLTarget {
    BC,
    DE,
    HL,
    SP,
}

//BIT/RESET/SET/etc. operate on a single bit (0-7) of a register
#[derive(Clone, Copy)]
pub enum BitPosition {
    B0,
    B1,
    B2,
    B3,
    B4,
    B5,
    B6,
    B7,
}

impl std::convert::From<BitPosition> for u8 {
    fn from(position: BitPosition) -> u8 {
        match position {
            BitPosition::B0 => 0,
            BitPosition::B1 => 1,
            BitPosition::B2 => 2,
            BitPosition::B3 => 3,
            BitPosition::B4 => 4,
            BitPosition::B5 => 5,
            BitPosition::B6 => 6,
            BitPosition::B7 => 7,
        }
    }
}

