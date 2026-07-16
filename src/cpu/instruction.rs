//this is for adding instructions on register data. First we need to
//define all the instructions(I got this list from the github article I'm using)
//and then we need to define the targets for those instructions. some instructions
//operate on 8 bit registers, some operate on 16 bit register pairs, and some
//operate on a single bit of a register.

//copy so I can both time an instruction and execute it without moving it twice
#[derive(Clone, Copy)]
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
    //adding 16 bit increment/decrements of a register pair
    //unlike 8bit INC/DEC, these don't touch any flags
    INC16(WordRegister),
    DEC16(WordRegister),
    //adds sp, 48 adds signed byte to stack pointer to reserve stack space
    ADDSP,
    CCF,
    SCF,
    RRA,
    RLA,
    RRCA,
    RLCA,
    CPL,
    //daa adjusts A into a valid binary coded decimal value after add/subtract
    DAA,
    //nop only advances pc, halt pauses cpu until interrupt wakes it up,
    //stop is a deeper sleep until a button press
    NOP,
    HALT,
    STOP,
    //rst calls one of 8 fixed low addresses
    RST(u16),
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
    //Jumps change what the program counter does next instead of just falling
    //through to the following instruction
    //JP  = jump to an absolute 16 bit address that follows the opcode in memory
    //JR  = jump relative, by a signed 8 bit offset that follows the opcode
    //JPI = jump to the address currently held in the HL register (JP (HL))
    JP(JumpTest),
    JR(JumpTest),
    JPI,
    //Now I'm getting to reading and writing to memory. LD copies a value from
    //a source to a target. The source and target can be anything from a register
    //to an immediate value baked into instruction to a place in memory.
    //LoadType accepts all the different shapes a load can take
    LD(LoadType),
    //Now for stack stuff. PUSH writes a 16bit register pair onto it, moving SP
    //down 2, and POP reads a pair back off, moving SP up 2. These are the
    //building blocks for CALL and RET which will come next
    PUSH(StackTarget),
    POP(StackTarget),
    //Function calls, CALL is a jump taht pushes the address of the following
    //instruction onto the stack and RET pops that address back to the pc to return
    CALL(JumpTest),
    RET(JumpTest),
    DI,
    EI,
    RETI,
}

//jump can be unconditional or gated on the state of a flag. If the condition
//is false, the CPU just moves past the jump to the next instruction
#[derive(Clone, Copy)]
pub enum JumpTest {
    NotZero,
    Zero,
    NotCarry,
    Carry,
    Always,
}

//16 bit register pairs that push and pop operate on
#[derive(Clone, Copy)]
pub enum StackTarget {
    BC,
    DE,
    HL,
    AF,
}


//The different types of load the CPU supports
#[derive(Clone, Copy)]
pub enum LoadType {
    //8 bit load between two registers, an immediate byte, or the byte at (HL)
    Byte(LoadByteTarget, LoadByteSource),
    //16 bit load of an immediate word into a 16 bit register (BC/DE/HL/SP)
    Word(LoadWordTarget),
    //Load A from a byte in memory whose address comes from a register pair/immediate
    AFromIndirect(Indirect),
    //Store A into a byte in memory whose address comes from a register pair/immediate
    IndirectFromA(Indirect),
    //Load A from the high page: memory at 0xFF00 + an immediate byte
    AFromByteAddress,
    //Store A into the high page: memory at 0xFF00 + an immediate byte
    ByteAddressFromA,
    //LD SP, HL. copy HL into stack ptr
    SPFromHL,
    //LD(a16), SP. write 16 bit SP out to immediate memory address
    IndirectFromSP,
    //LD HL, SP + r8. load HL with SP plus a signed byte
    HLFromSPPlus,
}

//HLI here means HL Indirect, or the byte in memory that HL points at
#[derive(Clone, Copy)]
pub enum LoadByteTarget {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
    HLI,
}

#[derive(Clone, Copy)]
pub enum LoadByteSource {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
    //D8 is an immediate 8 bit value that follows the opcode in memory
    D8,
    HLI,
}

#[derive(Clone, Copy)]
pub enum LoadWordTarget {
    BC,
    DE,
    HL,
    SP,
}

//These are the ways a load can name a place in memory to read A from or write A to
#[derive(Clone, Copy)]
pub enum Indirect {
    BCIndirect, //address in BC
    DEIndirect, //address in DE
    HLIndirectPlus, //address in HL, then HL is incremented afterwards
    HLIndirectMinus, //address in HL, then HL is decremented afterwards
    WordIndirect, //address is an immediate 16 bit value after the opcode
    LastByteIndirect, //address is 0xFF00 + register C
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
        let target = Instruction::prefixed_target(byte);
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
    fn prefixed_target(byte: u8) -> ArithmeticTarget {
        match byte & 0x07 {
            0 => ArithmeticTarget::B,
            1 => ArithmeticTarget::C,
            2 => ArithmeticTarget::D,
            3 => ArithmeticTarget::E,
            4 => ArithmeticTarget::H,
            5 => ArithmeticTarget::L,
            6 => ArithmeticTarget::HLI,
            _ => ArithmeticTarget::A,
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

    //This decodes the LD opcodes. Like the 0xCB table, the big block of register
    //to register loads is regular enough to decode from the bits, so
    //we do that first and fall back to an explicit table for the irregular loads
    fn from_byte_load(byte: u8) -> Option<Instruction> {
        if (0x40..=0x7F).contains(&byte) && byte != 0x76 {
            let target = Instruction::load_byte_target((byte >> 3) & 0x07);
            let source = Instruction::load_byte_source(byte & 0x07);
            return Some(Instruction::LD(LoadType::Byte(target, source)));
        }

        let load_type = match byte {
            0x06 => LoadType::Byte(LoadByteTarget::B, LoadByteSource::D8),
            0x0E => LoadType::Byte(LoadByteTarget::C, LoadByteSource::D8),
            0x16 => LoadType::Byte(LoadByteTarget::D, LoadByteSource::D8),
            0x1E => LoadType::Byte(LoadByteTarget::E, LoadByteSource::D8),
            0x26 => LoadType::Byte(LoadByteTarget::H, LoadByteSource::D8),
            0x2E => LoadType::Byte(LoadByteTarget::L, LoadByteSource::D8),
            0x36 => LoadType::Byte(LoadByteTarget::HLI, LoadByteSource::D8),
            0x3E => LoadType::Byte(LoadByteTarget::A, LoadByteSource::D8),

            //LD rr, d16 - load an immediate word into a 16 bit register
            0x01 => LoadType::Word(LoadWordTarget::BC),
            0x11 => LoadType::Word(LoadWordTarget::DE),
            0x21 => LoadType::Word(LoadWordTarget::HL),
            0x31 => LoadType::Word(LoadWordTarget::SP),

            //LD A, (indirect) - read A from a byte in memory
            0x0A => LoadType::AFromIndirect(Indirect::BCIndirect),
            0x1A => LoadType::AFromIndirect(Indirect::DEIndirect),
            0x2A => LoadType::AFromIndirect(Indirect::HLIndirectPlus),
            0x3A => LoadType::AFromIndirect(Indirect::HLIndirectMinus),
            0xFA => LoadType::AFromIndirect(Indirect::WordIndirect),
            0xF2 => LoadType::AFromIndirect(Indirect::LastByteIndirect),

            //LD (indirect), A - write A to a byte in memory
            0x02 => LoadType::IndirectFromA(Indirect::BCIndirect),
            0x12 => LoadType::IndirectFromA(Indirect::DEIndirect),
            0x22 => LoadType::IndirectFromA(Indirect::HLIndirectPlus),
            0x32 => LoadType::IndirectFromA(Indirect::HLIndirectMinus),
            0xEA => LoadType::IndirectFromA(Indirect::WordIndirect),
            0xE2 => LoadType::IndirectFromA(Indirect::LastByteIndirect),

            //LD A, (0xFF00 + a8) and LD (0xFF00 + a8), A - the high page loads
            0xF0 => LoadType::AFromByteAddress,
            0xE0 => LoadType::ByteAddressFromA,
            //Stack pointer loads
            0xF9 => LoadType::SPFromHL, //LD SP, HL 1 byte
            0x08 => LoadType::IndirectFromSP, //LD (a16), SP 3 bytes
            0xF8 => LoadType::HLFromSPPlus,   //LD HL, SP + r8 2 bytes
            
            _ => return None,
        };
        Some(Instruction::LD(load_type))
    }

    //This maps the 3 bit register index used by the byte loads to a load target. Index 6
    //is (HL), unlike the arithmetic tables, loads actually do support the (HL) operand
    fn load_byte_target(index: u8) -> LoadByteTarget {
        match index {
            0 => LoadByteTarget::B,
            1 => LoadByteTarget::C,
            2 => LoadByteTarget::D,
            3 => LoadByteTarget::E,
            4 => LoadByteTarget::H,
            5 => LoadByteTarget::L,
            6 => LoadByteTarget::HLI,
            _ => LoadByteTarget::A,
        }
    }

    fn load_byte_source(index: u8) -> LoadByteSource {
        match index {
            0 => LoadByteSource::B,
            1 => LoadByteSource::C,
            2 => LoadByteSource::D,
            3 => LoadByteSource::E,
            4 => LoadByteSource::H,
            5 => LoadByteSource::L,
            6 => LoadByteSource::HLI,
            _ => LoadByteSource::A,
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

            //ALU op A, (HL), the same operations but with the byte HL points at
            0x86 => Some(Instruction::ADD(ArithmeticTarget::HLI)),
            0x8E => Some(Instruction::ADC(ArithmeticTarget::HLI)),
            0x96 => Some(Instruction::SUB(ArithmeticTarget::HLI)),
            0x9E => Some(Instruction::SBC(ArithmeticTarget::HLI)),
            0xA6 => Some(Instruction::AND(ArithmeticTarget::HLI)),
            0xAE => Some(Instruction::XOR(ArithmeticTarget::HLI)),
            0xB6 => Some(Instruction::OR(ArithmeticTarget::HLI)),
            0xBE => Some(Instruction::CP(ArithmeticTarget::HLI)),

            //ALU op A, d8, the same operations but with an immediate byte (2 bytes)
            0xC6 => Some(Instruction::ADD(ArithmeticTarget::D8)),
            0xCE => Some(Instruction::ADC(ArithmeticTarget::D8)),
            0xD6 => Some(Instruction::SUB(ArithmeticTarget::D8)),
            0xDE => Some(Instruction::SBC(ArithmeticTarget::D8)),
            0xE6 => Some(Instruction::AND(ArithmeticTarget::D8)),
            0xEE => Some(Instruction::XOR(ArithmeticTarget::D8)),
            0xF6 => Some(Instruction::OR(ArithmeticTarget::D8)),
            0xFE => Some(Instruction::CP(ArithmeticTarget::D8)),

            //INC r (8 bit)
            0x04 => Some(Instruction::INC(ArithmeticTarget::B)),
            0x0C => Some(Instruction::INC(ArithmeticTarget::C)),
            0x14 => Some(Instruction::INC(ArithmeticTarget::D)),
            0x1C => Some(Instruction::INC(ArithmeticTarget::E)),
            0x24 => Some(Instruction::INC(ArithmeticTarget::H)),
            0x2C => Some(Instruction::INC(ArithmeticTarget::L)),
            0x3C => Some(Instruction::INC(ArithmeticTarget::A)),
            0x34 => Some(Instruction::INC(ArithmeticTarget::HLI)),   

            //DEC r (8 bit)
            0x05 => Some(Instruction::DEC(ArithmeticTarget::B)),
            0x0D => Some(Instruction::DEC(ArithmeticTarget::C)),
            0x15 => Some(Instruction::DEC(ArithmeticTarget::D)),
            0x1D => Some(Instruction::DEC(ArithmeticTarget::E)),
            0x25 => Some(Instruction::DEC(ArithmeticTarget::H)),
            0x2D => Some(Instruction::DEC(ArithmeticTarget::L)),
            0x3D => Some(Instruction::DEC(ArithmeticTarget::A)),
            0x35 => Some(Instruction::DEC(ArithmeticTarget::HLI)),

            //INC rr/DEC rr(16 bit), no flags affected
            0x03 => Some(Instruction::INC16(WordRegister::BC)),
            0x13 => Some(Instruction::INC16(WordRegister::DE)),
            0x23 => Some(Instruction::INC16(WordRegister::HL)),
            0x33 => Some(Instruction::INC16(WordRegister::SP)),
            0x0B => Some(Instruction::DEC16(WordRegister::BC)),
            0x1B => Some(Instruction::DEC16(WordRegister::DE)),
            0x2B => Some(Instruction::DEC16(WordRegister::HL)),
            0x3B => Some(Instruction::DEC16(WordRegister::SP)),

            //ADD SP, r8, add a signed byte to SP (2 bytes)
            0xE8 => Some(Instruction::ADDSP),

            //Control ops
            0x00 => Some(Instruction::NOP),
            0x76 => Some(Instruction::HALT),
            0x10 => Some(Instruction::STOP),
            0x27 => Some(Instruction::DAA),

            //RST n, one byte call to a fixed restart vector
            0xC7 => Some(Instruction::RST(0x00)),
            0xCF => Some(Instruction::RST(0x08)),
            0xD7 => Some(Instruction::RST(0x10)),
            0xDF => Some(Instruction::RST(0x18)),
            0xE7 => Some(Instruction::RST(0x20)),
            0xEF => Some(Instruction::RST(0x28)),
            0xF7 => Some(Instruction::RST(0x30)),
            0xFF => Some(Instruction::RST(0x38)),
        
            //Single-byte accumulator/flag operations
            0x07 => Some(Instruction::RLCA),
            0x0F => Some(Instruction::RRCA),
            0x17 => Some(Instruction::RLA),
            0x1F => Some(Instruction::RRA),
            0x2F => Some(Instruction::CPL),
            0x37 => Some(Instruction::SCF),
            0x3F => Some(Instruction::CCF),

            //JP a16 - absolute jump (opcode + 2 byte address = 3 bytes)
            0xC2 => Some(Instruction::JP(JumpTest::NotZero)),
            0xCA => Some(Instruction::JP(JumpTest::Zero)),
            0xD2 => Some(Instruction::JP(JumpTest::NotCarry)),
            0xDA => Some(Instruction::JP(JumpTest::Carry)),
            0xC3 => Some(Instruction::JP(JumpTest::Always)),

            //JR r8 - relative jump (opcode + signed 8 bit offset = 2 bytes)
            0x20 => Some(Instruction::JR(JumpTest::NotZero)),
            0x28 => Some(Instruction::JR(JumpTest::Zero)),
            0x30 => Some(Instruction::JR(JumpTest::NotCarry)),
            0x38 => Some(Instruction::JR(JumpTest::Carry)),
            0x18 => Some(Instruction::JR(JumpTest::Always)),

            //JP (HL) - jump to the address held in HL (1 byte)
            0xE9 => Some(Instruction::JPI),
            
            //PUSH rr - push a register pair onto the stack (1 byte)
            0xC5 => Some(Instruction::PUSH(StackTarget::BC)),
            0xD5 => Some(Instruction::PUSH(StackTarget::DE)),
            0xE5 => Some(Instruction::PUSH(StackTarget::HL)),
            0xF5 => Some(Instruction::PUSH(StackTarget::AF)),

            //POP rr - pop a register pair off the stack (1 byte)
            0xC1 => Some(Instruction::POP(StackTarget::BC)),
            0xD1 => Some(Instruction::POP(StackTarget::DE)),
            0xE1 => Some(Instruction::POP(StackTarget::HL)),
            0xF1 => Some(Instruction::POP(StackTarget::AF)),

            //CALL a16 - call a function (opcode + 2 byte address = 3 bytes)
            0xC4 => Some(Instruction::CALL(JumpTest::NotZero)),
            0xCC => Some(Instruction::CALL(JumpTest::Zero)),
            0xD4 => Some(Instruction::CALL(JumpTest::NotCarry)),
            0xDC => Some(Instruction::CALL(JumpTest::Carry)),
            0xCD => Some(Instruction::CALL(JumpTest::Always)),

            //RET - return from a function (1 byte)
            0xC0 => Some(Instruction::RET(JumpTest::NotZero)),
            0xC8 => Some(Instruction::RET(JumpTest::Zero)),
            0xD0 => Some(Instruction::RET(JumpTest::NotCarry)),
            0xD8 => Some(Instruction::RET(JumpTest::Carry)),
            0xC9 => Some(Instruction::RET(JumpTest::Always)),
            
            0xF3 => Some(Instruction::DI),
            0xFB => Some(Instruction::EI),
            0xD9 => Some(Instruction::RETI),
            
            _ => Instruction::from_byte_load(byte),
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
    //HLI is the byte in memory that HL points at, (HL) operand shared by ALU ops, INC/DEC, and 
    //the whole 0xCB table
    HLI, 
    //D8 is an immediate byte that follows the opcode used by A, d8, ALU ops
    D8,
}

//ADDHL adds a 16 bit register pair into HL, so it needs its own target list instead of ArithmeticTarget
#[derive(Clone, Copy)]
pub enum ADDHLTarget {
    BC,
    DE,
    HL,
    SP,
}

//The 16 bit register pairs that the 16 bit INC/DEC instructions operate on
#[derive(Clone, Copy)]
pub enum WordRegister {
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

