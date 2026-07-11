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

impl Instruction {
    //This code turns an opcode byte into the instruction that it represents.
    //Prefixed tells us whether to look in the normal opcode table or the 0xCB prefixed table.
    //when the CPU encounters 0xCB, it reads the byte from memory as a secondary instruction
    //for bit level manipulations like BIT, RLC, and SET. If the byte is not mapped yet, it returns None.
    pub fn from_byte(byte: u8, prefixed: bool) -> Option<Instruction> {
        if prefixed {
            Instruction::from_byte_prefixed(byte)
        } else {
            Instruction::from_byte_not_prefixed(byte)
        }
    }

   //The 0xCB prefixed table is really easy to decode structurally, so I don't need to
    //write out all 256 entries by hand.
    //The low 3 bits(byte & 0x07) pick the target register: B,C,D,E,H,L,(HL),A
    //The high bits pick the operation
    //For BIT/RES/SET the middle 3 bits (byte >> 3 & 0x07) also encode which bit (0-7)
    //I haven't written code to support the (HL) memory operand yet, so target index 6 decodes to None.
    fn from_byte_prefixed(byte: u8) -> Option<Instruction> {
        let target = Instruction::prefixed_target(byte)?;
        let bit = Instruction::bit_position(byte);

        let instruction = match byte {
            0x00..=0x07 => Instruction::RLC(target),
            0x08..=0x0F => Instruction::RRC(target),
            0x10..=0x17 => Instruction::RL(target),
            0x18..=0x1F => Instruction::RR(target),
            0x20..=0x27 => Instruction::SLA(target),
            0x28..=0x2F => Instruction::SRA(target),
            0x30..=0x37 => Instruction::SWAP(target),
            0x38..=0x3F => Instruction::SRL(target),
            0x40..=0x7F => Instruction::BIT(target, bit),
            0x80..=0xBF => Instruction::RESET(target, bit),
            0xC0..=0xFF => Instruction::SET(target, bit),
        };
        Some(instruction)
    }

    //This decodes the low 3 bits of a 0xCB opcode into a register target. Index 6 
    //is the (HL) memory operand, which needs a memory access we haven't built yet,
    //so we return None for it for now
    fn prefixed_target(byte: u8) -> Option<ArithmeticTarget> {
        match byte & 0x07 {
            0 => Some(ArithmeticTarget::B),
            1 => Some(ArithmeticTarget::C),
            2 => Some(ArithmeticTarget::D),
            3 => Some(ArithmeticTarget::E),
            4 => Some(ArithmeticTarget::H),
            5 => Some(ArithmeticTarget::L),
            7 => Some(ArithmeticTarget::A),
            //6 => (HL) for later
            _ => None,
        }
    }

    //This decodes bits 3 to 5 of a 0xCB opcode into the bit number used by BIT/RES/SET.
    //For the rotate/shift/swap opcodes these bits are part of the operation
    //selector instead, so the value we return there is ignored
    fn bit_position(byte: u8) -> BitPosition {
        match (byte >> 3) & 0x07 {
            0 => BitPosition::B0,
            1 => BitPosition::B1,
            2 => BitPosition::B2,
            3 => BitPosition::B3,
            4 => BitPosition::B4,
            5 => BitPosition::B5,
            6 => BitPosition::B6,
            _ => BitPosition::B7,
        }
    }


    fn from_byte_not_prefixed(byte: u8) -> Option<Instruction> {
        match byte {
            //ADD A, r
            0x80 => Some(Instruction::ADD(ArithmeticTarget::B)),
            0x81 => Some(Instruction::ADD(ArithmeticTarget::C)),
            0x82 => Some(Instruction::ADD(ArithmeticTarget::D)),
            0x83 => Some(Instruction::ADD(ArithmeticTarget::E)),
            0x84 => Some(Instruction::ADD(ArithmeticTarget::H)),
            0x85 => Some(Instruction::ADD(ArithmeticTarget::L)),
            0x87 => Some(Instruction::ADD(ArithmeticTarget::A)),

            //ADD HL, rr
            0x09 => Some(Instruction::ADDHL(ADDHLTarget::BC)),
            0x19 => Some(Instruction::ADDHL(ADDHLTarget::DE)),
            0x29 => Some(Instruction::ADDHL(ADDHLTarget::HL)),
            0x39 => Some(Instruction::ADDHL(ADDHLTarget::SP)),

            //ADC A, r
            0x88 => Some(Instruction::ADC(ArithmeticTarget::B)),
            0x89 => Some(Instruction::ADC(ArithmeticTarget::C)),
            0x8A => Some(Instruction::ADC(ArithmeticTarget::D)),
            0x8B => Some(Instruction::ADC(ArithmeticTarget::E)),
            0x8C => Some(Instruction::ADC(ArithmeticTarget::H)),
            0x8D => Some(Instruction::ADC(ArithmeticTarget::L)),
            0x8F => Some(Instruction::ADC(ArithmeticTarget::A)),

            //SUB r
            0x90 => Some(Instruction::SUB(ArithmeticTarget::B)),
            0x91 => Some(Instruction::SUB(ArithmeticTarget::C)),
            0x92 => Some(Instruction::SUB(ArithmeticTarget::D)),
            0x93 => Some(Instruction::SUB(ArithmeticTarget::E)),
            0x94 => Some(Instruction::SUB(ArithmeticTarget::H)),
            0x95 => Some(Instruction::SUB(ArithmeticTarget::L)),
            0x97 => Some(Instruction::SUB(ArithmeticTarget::A)),

            //SBC A, r
            0x98 => Some(Instruction::SBC(ArithmeticTarget::B)),
            0x99 => Some(Instruction::SBC(ArithmeticTarget::C)),
            0x9A => Some(Instruction::SBC(ArithmeticTarget::D)),
            0x9B => Some(Instruction::SBC(ArithmeticTarget::E)),
            0x9C => Some(Instruction::SBC(ArithmeticTarget::H)),
            0x9D => Some(Instruction::SBC(ArithmeticTarget::L)),
            0x9F => Some(Instruction::SBC(ArithmeticTarget::A)),

            //AND r
            0xA0 => Some(Instruction::AND(ArithmeticTarget::B)),
            0xA1 => Some(Instruction::AND(ArithmeticTarget::C)),
            0xA2 => Some(Instruction::AND(ArithmeticTarget::D)),
            0xA3 => Some(Instruction::AND(ArithmeticTarget::E)),
            0xA4 => Some(Instruction::AND(ArithmeticTarget::H)),
            0xA5 => Some(Instruction::AND(ArithmeticTarget::L)),
            0xA7 => Some(Instruction::AND(ArithmeticTarget::A)),

            //XOR r
            0xA8 => Some(Instruction::XOR(ArithmeticTarget::B)),
            0xA9 => Some(Instruction::XOR(ArithmeticTarget::C)),
            0xAA => Some(Instruction::XOR(ArithmeticTarget::D)),
            0xAB => Some(Instruction::XOR(ArithmeticTarget::E)),
            0xAC => Some(Instruction::XOR(ArithmeticTarget::H)),
            0xAD => Some(Instruction::XOR(ArithmeticTarget::L)),
            0xAF => Some(Instruction::XOR(ArithmeticTarget::A)),

            //OR r
            0xB0 => Some(Instruction::OR(ArithmeticTarget::B)),
            0xB1 => Some(Instruction::OR(ArithmeticTarget::C)),
            0xB2 => Some(Instruction::OR(ArithmeticTarget::D)),
            0xB3 => Some(Instruction::OR(ArithmeticTarget::E)),
            0xB4 => Some(Instruction::OR(ArithmeticTarget::H)),
            0xB5 => Some(Instruction::OR(ArithmeticTarget::L)),
            0xB7 => Some(Instruction::OR(ArithmeticTarget::A)),

            //CP r
            0xB8 => Some(Instruction::CP(ArithmeticTarget::B)),
            0xB9 => Some(Instruction::CP(ArithmeticTarget::C)),
            0xBA => Some(Instruction::CP(ArithmeticTarget::D)),
            0xBB => Some(Instruction::CP(ArithmeticTarget::E)),
            0xBC => Some(Instruction::CP(ArithmeticTarget::H)),
            0xBD => Some(Instruction::CP(ArithmeticTarget::L)),
            0xBF => Some(Instruction::CP(ArithmeticTarget::A)),

            //INC r (8 bit)
            0x04 => Some(Instruction::INC(ArithmeticTarget::B)),
            0x0C => Some(Instruction::INC(ArithmeticTarget::C)),
            0x14 => Some(Instruction::INC(ArithmeticTarget::D)),
            0x1C => Some(Instruction::INC(ArithmeticTarget::E)),
            0x24 => Some(Instruction::INC(ArithmeticTarget::H)),
            0x2C => Some(Instruction::INC(ArithmeticTarget::L)),
            0x3C => Some(Instruction::INC(ArithmeticTarget::A)),

            //DEC r (8 bit)
            0x05 => Some(Instruction::DEC(ArithmeticTarget::B)),
            0x0D => Some(Instruction::DEC(ArithmeticTarget::C)),
            0x15 => Some(Instruction::DEC(ArithmeticTarget::D)),
            0x1D => Some(Instruction::DEC(ArithmeticTarget::E)),
            0x25 => Some(Instruction::DEC(ArithmeticTarget::H)),
            0x2D => Some(Instruction::DEC(ArithmeticTarget::L)),
            0x3D => Some(Instruction::DEC(ArithmeticTarget::A)),

            //Single-byte accumulator/flag operations
            0x07 => Some(Instruction::RLCA),
            0x0F => Some(Instruction::RRCA),
            0x17 => Some(Instruction::RLA),
            0x1F => Some(Instruction::RRA),
            0x2F => Some(Instruction::CPL),
            0x37 => Some(Instruction::SCF),
            0x3F => Some(Instruction::CCF),

            //I also need to add the remaining opcodes (LD, jumps, stack ops, etc) as I build them out
            _ => None,
        }
    }
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

