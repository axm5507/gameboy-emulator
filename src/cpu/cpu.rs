use crate::cpu::instruction::{ADDHLTarget, ArithmeticTarget, BitPosition, Instruction};
use crate::cpu::memory_bus::MemoryBus;
use crate::cpu::registers::Registers;

pub struct CPU {
    pub registers: Registers,
    pub bus: MemoryBus,
}

impl CPU {
    pub fn new() -> Self {
        Self {
            registers: Registers::new(),
            bus: MemoryBus::new(),
        }
    }


    //if you're familiar with how a CPU works, you'll recognize this as a fetch-decode-execute
    //cycle. First, we fetch(read the opcode byte that the program counter points at), then we
    //decode(turn the byte into an Instruction), then we execute(run it, which also tells us where
    //the program counter should go next), and finally, we advance by moving the program counter to
    //the next address.
    pub fn step(&mut self) {
        let mut instruction_byte = self.bus.read_byte(self.registers.pc);


        //0xcb is a prefix opcode. When the processor encounters it, it knows to read the
        //next byte in memory and execute an extended bitwise operation. When we read the
        //following byte we remember we're prefixed so we decode against the right table
        //and account for the extra byte later.
        let prefixed = instruction_byte == 0xCB;
        if prefixed {
            instruction_byte = self.bus.read_byte(self.registers.pc.wrapping_add(1));
        }

        let next_pc = if let Some(instruction) = Instruction::from_byte(instruction_byte, prefixed)
        {
            self.execute(instruction)
        } else {
            let description = format!("0x{}{:02x}", if prefixed { "cb" } else { "" }, instruction_byte);
            panic!("Unknown instruction found for: {}", description);
        };

        self.registers.pc = next_pc;
    }

    //Now to write the execute function that contains the logic for each instruction mentioned in the instructions file
    //It now returns the address the program counter should move to after this instruction
    pub fn execute(&mut self, instruction: Instruction) -> u16 {
        match instruction {
            Instruction::ADD(target) => {
                let value = self.get_register(target);
                let new_value = self.add(value);
                self.registers.a = new_value;
                self.registers.pc.wrapping_add(1)
            }
            Instruction::ADDHL(target) => {
                let value = match target {
                    ADDHLTarget::BC => self.registers.bc(),
                    ADDHLTarget::DE => self.registers.de(),
                    ADDHLTarget::HL => self.registers.hl(),
                    ADDHLTarget::SP => self.registers.sp,
                };
                let new_value = self.add_hl(value);
                self.registers.set_hl(new_value);
                self.registers.pc.wrapping_add(1)
            }
            Instruction::ADC(target) => {
                let value = self.get_register(target);
                let new_value = self.adc(value);
                self.registers.a = new_value;
                self.registers.pc.wrapping_add(1)
            }
            Instruction::SUB(target) => {
                let value = self.get_register(target);
                let new_value = self.sub(value);
                self.registers.a = new_value;
                self.registers.pc.wrapping_add(1)
            }
            Instruction::SBC(target) => {
                let value = self.get_register(target);
                let new_value = self.sbc(value);
                self.registers.a = new_value;
                self.registers.pc.wrapping_add(1)
            }
            Instruction::AND(target) => {
                let value = self.get_register(target);
                let new_value = self.and(value);
                self.registers.a = new_value;
                self.registers.pc.wrapping_add(1)
            }
            Instruction::OR(target) => {
                let value = self.get_register(target);
                let new_value = self.or(value);
                self.registers.a = new_value;
                self.registers.pc.wrapping_add(1)
            }
            Instruction::XOR(target) => {
                let value = self.get_register(target);
                let new_value = self.xor(value);
                self.registers.a = new_value;
                self.registers.pc.wrapping_add(1)
            }
            Instruction::CP(target) => {
                let value = self.get_register(target);
                self.sub(value);
                self.registers.pc.wrapping_add(1)
            }
            Instruction::INC(target) => {
                let value = self.get_register(target);
                let new_value = self.inc(value);
                self.set_register(target, new_value);
                self.registers.pc.wrapping_add(1)
            }
            Instruction::DEC(target) => {
                let value = self.get_register(target);
                let new_value = self.dec(value);
                self.set_register(target, new_value);
                self.registers.pc.wrapping_add(1)
            }
            Instruction::CCF => {
                self.ccf();
                self.registers.pc.wrapping_add(1)
            }
            Instruction::SCF => {
                self.scf();
                self.registers.pc.wrapping_add(1)
            }
            Instruction::RRA => {
                self.rra();
                self.registers.pc.wrapping_add(1)
            }
            Instruction::RLA => {
                self.rla();
                self.registers.pc.wrapping_add(1)
            }
            Instruction::RRCA => {
                self.rrca();
                self.registers.pc.wrapping_add(1)
            }
            Instruction::RLCA => {
                self.rlca();
                self.registers.pc.wrapping_add(1)
            }
            Instruction::CPL => {
                self.cpl();
                self.registers.pc.wrapping_add(1)
            }
            //Everything below comes from the 0xCB prefixed table, so each of these
            //instructions is two bytes long(0xCB prefix + opcode) and the program
            //counter advances by 2
            Instruction::BIT(target, bit) => {
                let value = self.get_register(target);
                self.bit(value, bit);
                self.registers.pc.wrapping_add(2)
            }
            Instruction::RESET(target, bit) => {
                let value = self.get_register(target);
                let new_value = self.reset(value, bit);
                self.set_register(target, new_value);
                self.registers.pc.wrapping_add(2)
            }
            Instruction::SET(target, bit) => {
                let value = self.get_register(target);
                let new_value = self.set_bit(value, bit);
                self.set_register(target, new_value);
                self.registers.pc.wrapping_add(2)
            }
            Instruction::SRL(target) => {
                let value = self.get_register(target);
                let new_value = self.srl(value);
                self.set_register(target, new_value);
                self.registers.pc.wrapping_add(2)
            }
            Instruction::RR(target) => {
                let value = self.get_register(target);
                let new_value = self.rr(value);
                self.set_register(target, new_value);
                self.registers.pc.wrapping_add(2)
            }
            Instruction::RL(target) => {
                let value = self.get_register(target);
                let new_value = self.rl(value);
                self.set_register(target, new_value);
                self.registers.pc.wrapping_add(2)
            }
            Instruction::RRC(target) => {
                let value = self.get_register(target);
                let new_value = self.rrc(value);
                self.set_register(target, new_value);
                self.registers.pc.wrapping_add(2)
            }
            Instruction::RLC(target) => {
                let value = self.get_register(target);
                let new_value = self.rlc(value);
                self.set_register(target, new_value);
                self.registers.pc.wrapping_add(2)
            }
            Instruction::SRA(target) => {
                let value = self.get_register(target);
                let new_value = self.sra(value);
                self.set_register(target, new_value);
                self.registers.pc.wrapping_add(2)
            }
            Instruction::SLA(target) => {
                let value = self.get_register(target);
                let new_value = self.sla(value);
                self.set_register(target, new_value);
                self.registers.pc.wrapping_add(2)
            }
            Instruction::SWAP(target) => {
                let value = self.get_register(target);
                let new_value = self.swap(value);
                self.set_register(target, new_value);
                self.registers.pc.wrapping_add(2)
            }
            //jumps compute the next program counter directly instead of just
            //advancing past themselves, so each helper returns new pc
            Instruction::JP(test) => {
                let should_jump = self.should_jump(test);
                self.jump(should_jump)
            }
            Instruction::JR(test) => {
                let should_jump = self.should_jump(test);
                self.jump_relative(should_jump)
            }
            Instruction::JPI => self.registers.hl(), 
            Instruction::LD(load_type) => self.execute_load(load_type),
        }
    }

    //This runs a load and returns the next program counter. Each shape of load advances
    //the pc by a different amount depending on how many bytes the opcode occupies
    fn execute_load(&mut self, load_type: LoadType) -> u16 {
        match load_type {
            LoadType::Byte(target, source) => {
                let source_value = match source {
                    LoadByteSource::A => self.registers.a,
                    LoadByteSource::B => self.registers.b,
                    LoadByteSource::C => self.registers.c,
                    LoadByteSource::D => self.registers.d,
                    LoadByteSource::E => self.registers.e,
                    LoadByteSource::H => self.registers.h,
                    LoadByteSource::L => self.registers.l,
                    LoadByteSource::D8 => self.read_next_byte(),
                    LoadByteSource::HLI => self.bus.read_byte(self.registers.hl()),
                };
                match target {
                    LoadByteTarget::A => self.registers.a = source_value,
                    LoadByteTarget::B => self.registers.b = source_value,
                    LoadByteTarget::C => self.registers.c = source_value,
                    LoadByteTarget::D => self.registers.d = source_value,
                    LoadByteTarget::E => self.registers.e = source_value,
                    LoadByteTarget::H => self.registers.h = source_value,
                    LoadByteTarget::L => self.registers.l = source_value,
                    LoadByteTarget::HLI => self.bus.write_byte(self.registers.hl(), source_value),
                }
                //Only an immediate (D8) source makes this a 2 byte instruction, every
                //other byte load is a single byte
                match source {
                    LoadByteSource::D8 => self.registers.pc.wrapping_add(2),
                    _ => self.registers.pc.wrapping_add(1),
                }
            }
            LoadType::Word(target) => {
                let value = self.read_next_word();
                match target {
                    LoadWordTarget::BC => self.registers.set_bc(value),
                    LoadWordTarget::DE => self.registers.set_de(value),
                    LoadWordTarget::HL => self.registers.set_hl(value),
                    LoadWordTarget::SP => self.registers.sp = value,
                }
                //opcode + 2 byte immediate word = 3 bytes
                self.registers.pc.wrapping_add(3)
            }
            LoadType::AFromIndirect(indirect) => {
                let address = self.indirect_address(indirect);
                self.registers.a = self.bus.read_byte(address);
                self.apply_indirect_hl_delta(indirect);
                self.indirect_next_pc(indirect)
            }
            LoadType::IndirectFromA(indirect) => {
                let address = self.indirect_address(indirect);
                self.bus.write_byte(address, self.registers.a);
                self.apply_indirect_hl_delta(indirect);
                self.indirect_next_pc(indirect)
            }
            LoadType::AFromByteAddress => {
                let offset = self.read_next_byte() as u16;
                self.registers.a = self.bus.read_byte(0xFF00 + offset);
                self.registers.pc.wrapping_add(2)
            }
            LoadType::ByteAddressFromA => {
                let offset = self.read_next_byte() as u16;
                self.bus.write_byte(0xFF00 + offset, self.registers.a);
                self.registers.pc.wrapping_add(2)
            }
        }
    }

    //this works out the memory address an indirect load reads from or writes to.
    fn indirect_address(&self, indirect: Indirect) -> u16 {
        match indirect {
            Indirect::BCIndirect => self.registers.bc(),
            Indirect::DEIndirect => self.registers.de(),
            //HL+ and HL- both use the *current* HL as the address; the adjustment
            //happens afterwards in apply_indirect_hl_delta
            Indirect::HLIndirectPlus => self.registers.hl(),
            Indirect::HLIndirectMinus => self.registers.hl(),
            Indirect::WordIndirect => self.read_next_word(),
            Indirect::LastByteIndirect => 0xFF00 | (self.registers.c as u16),
        }
    }

    //The auto-increment/auto-decrement side effect of the (HL+) and (HL-) loads
    fn apply_indirect_hl_delta(&mut self, indirect: Indirect) {
        match indirect {
            Indirect::HLIndirectPlus => {
                let hl = self.registers.hl();
                self.registers.set_hl(hl.wrapping_add(1));
            }
            Indirect::HLIndirectMinus => {
                let hl = self.registers.hl();
                self.registers.set_hl(hl.wrapping_sub(1));
            }
            _ => {}
        }
    }

    //This is a word-indirect load carries a 2 byte address (3 bytes total), the rest are
    //single-byte opcodes that get their address from a register.=
    fn indirect_next_pc(&self, indirect: Indirect) -> u16 {
        match indirect {
            Indirect::WordIndirect => self.registers.pc.wrapping_add(3),
            _ => self.registers.pc.wrapping_add(1),
        }
    }

    //This reads the immediate byte that follows the current opcode
    fn read_next_byte(&self) -> u8 {
        self.bus.read_byte(self.registers.pc.wrapping_add(1))
    }

    //This reads the immediate 16 bit word that follows the current opcode. The Game Boy
    //is little endian, so the byte at pc+1 is the low half and pc+2 is the high half
    fn read_next_word(&self) -> u16 {
        let low = self.bus.read_byte(self.registers.pc.wrapping_add(1)) as u16;
        let high = self.bus.read_byte(self.registers.pc.wrapping_add(2)) as u16;
        (high << 8) | low
    }

    

    //This is to evaluate a jump's condition against current flags
    //An unconditional jump is always true
    fn should_jump(&self, test: JumpTest) -> bool {
        match test {
            JumpTest::NotZero => !self.registers.zero(),
            JumpTest::Zero => self.registers.zero(),
            JumpTest::NotCarry => !self.registers.carry(),
            JumpTest::Carry => self.registers.carry(),
            JumpTest::Always => true,
        }
    }

    //Absolute jump. The 16 bit target address sits in the two bytes right after
    //the opcode. The Game Boy is little-endian, which means the byte at pc+1 is the low
    //half of the address and the byte at pc+2 is the high half
    fn jump(&self, should_jump: bool) -> u16 {
        if should_jump {
            let low = self.bus.read_byte(self.registers.pc.wrapping_add(1)) as u16;
            let high = self.bus.read_byte(self.registers.pc.wrapping_add(2)) as u16;
            (high << 8) | low
        } else {
            //JP is 3 bytes wide (1 opcode + 2 address bytes), so skip past all of it
            self.registers.pc.wrapping_add(3)
        }
    }

    //Relative jump. The single byte after the opcode is a signed offset applied
    //to the address of the instruction that follows this JR (pc + 2, since JR is
    //2 bytes wide)
    fn jump_relative(&self, should_jump: bool) -> u16 {
        let next_pc = self.registers.pc.wrapping_add(2);
        if should_jump {
            let offset = self.bus.read_byte(self.registers.pc.wrapping_add(1)) as i8;
            //wrapping_add handles negative offsets correctly
            next_pc.wrapping_add(offset as u16)
        } else {
            next_pc
        }
    }

    fn get_register(&self, target: ArithmeticTarget) -> u8 {
        match target {
            ArithmeticTarget::A => self.registers.a,
            ArithmeticTarget::B => self.registers.b,
            ArithmeticTarget::C => self.registers.c,
            ArithmeticTarget::D => self.registers.d,
            ArithmeticTarget::E => self.registers.e,
            ArithmeticTarget::H => self.registers.h,
            ArithmeticTarget::L => self.registers.l,
        }
    }

    fn set_register(&mut self, target: ArithmeticTarget, value: u8) {
        match target {
            ArithmeticTarget::A => self.registers.a = value,
            ArithmeticTarget::B => self.registers.b = value,
            ArithmeticTarget::C => self.registers.c = value,
            ArithmeticTarget::D => self.registers.d = value,
            ArithmeticTarget::E => self.registers.e = value,
            ArithmeticTarget::H => self.registers.h = value,
            ArithmeticTarget::L => self.registers.l = value,
        }
    }

    fn add(&mut self, value: u8) -> u8 {
        let (new_value, did_overflow) = self.registers.a.overflowing_add(value);
        self.registers.set_zero(new_value == 0);
        self.registers.set_subtract(false);
        self.registers
            .set_half_carry((self.registers.a & 0xF) + (value & 0xF) > 0xF);
        self.registers.set_carry(did_overflow);
        new_value
    }

    fn add_hl(&mut self, value: u16) -> u16 {
        let hl = self.registers.hl();
        let (new_value, did_overflow) = hl.overflowing_add(value);
        // Zero flag is left untouched by ADD HL on real hardware.
        self.registers.set_subtract(false);
        self.registers
            .set_half_carry((hl & 0xFFF) + (value & 0xFFF) > 0xFFF);
        self.registers.set_carry(did_overflow);
        new_value
    }

    fn adc(&mut self, value: u8) -> u8 {
        let carry = self.registers.carry() as u8;
        let (partial, overflow1) = self.registers.a.overflowing_add(value);
        let (new_value, overflow2) = partial.overflowing_add(carry);
        self.registers.set_zero(new_value == 0);
        self.registers.set_subtract(false);
        self.registers
            .set_half_carry((self.registers.a & 0xF) + (value & 0xF) + carry > 0xF);
        self.registers.set_carry(overflow1 || overflow2);
        new_value
    }

    fn sub(&mut self, value: u8) -> u8 {
        let (new_value, did_overflow) = self.registers.a.overflowing_sub(value);
        self.registers.set_zero(new_value == 0);
        self.registers.set_subtract(true);
        self.registers
            .set_half_carry((self.registers.a & 0xF) < (value & 0xF));
        self.registers.set_carry(did_overflow);
        new_value
    }

    fn sbc(&mut self, value: u8) -> u8 {
        let carry = self.registers.carry() as u8;
        let (partial, overflow1) = self.registers.a.overflowing_sub(value);
        let (new_value, overflow2) = partial.overflowing_sub(carry);
        self.registers.set_zero(new_value == 0);
        self.registers.set_subtract(true);
        self.registers
            .set_half_carry((self.registers.a & 0xF) < (value & 0xF) + carry);
        self.registers.set_carry(overflow1 || overflow2);
        new_value
    }

    fn and(&mut self, value: u8) -> u8 {
        let new_value = self.registers.a & value;
        self.registers.set_zero(new_value == 0);
        self.registers.set_subtract(false);
        self.registers.set_half_carry(true);
        self.registers.set_carry(false);
        new_value
    }

    fn or(&mut self, value: u8) -> u8 {
        let new_value = self.registers.a | value;
        self.registers.set_zero(new_value == 0);
        self.registers.set_subtract(false);
        self.registers.set_half_carry(false);
        self.registers.set_carry(false);
        new_value
    }

    fn xor(&mut self, value: u8) -> u8 {
        let new_value = self.registers.a ^ value;
        self.registers.set_zero(new_value == 0);
        self.registers.set_subtract(false);
        self.registers.set_half_carry(false);
        self.registers.set_carry(false);
        new_value
    }

    fn inc(&mut self, value: u8) -> u8 {
        let new_value = value.wrapping_add(1);
        self.registers.set_zero(new_value == 0);
        self.registers.set_subtract(false);
        self.registers.set_half_carry((value & 0xF) == 0xF);
        //carry flag left untouched by INC on real hardware
        new_value
    }

    fn dec(&mut self, value: u8) -> u8 {
        let new_value = value.wrapping_sub(1);
        self.registers.set_zero(new_value == 0);
        self.registers.set_subtract(true);
        self.registers.set_half_carry((value & 0xF) == 0);
        //carry flag left untouched by DEC on real hardware
        new_value
    }

    fn ccf(&mut self) {
        self.registers.set_subtract(false);
        self.registers.set_half_carry(false);
        self.registers.set_carry(!self.registers.carry());
    }

    fn scf(&mut self) {
        self.registers.set_subtract(false);
        self.registers.set_half_carry(false);
        self.registers.set_carry(true);
    }

    fn rra(&mut self) {
        self.registers.a = self.rr(self.registers.a);
        //unlike RR r, RRA always clears the zero flag regardless of the result
        self.registers.set_zero(false);
    }

    fn rla(&mut self) {
        self.registers.a = self.rl(self.registers.a);
        self.registers.set_zero(false);
    }

    fn rrca(&mut self) {
        self.registers.a = self.rrc(self.registers.a);
        self.registers.set_zero(false);
    }

    fn rlca(&mut self) {
        self.registers.a = self.rlc(self.registers.a);
        self.registers.set_zero(false);
    }

    fn cpl(&mut self) {
        self.registers.a = !self.registers.a;
        self.registers.set_subtract(true);
        self.registers.set_half_carry(true);
    }

    fn bit(&mut self, value: u8, bit: BitPosition) {
        let bit_position: u8 = bit.into();
        let is_set = (value >> bit_position) & 0b1 != 0;
        self.registers.set_zero(!is_set);
        self.registers.set_subtract(false);
        self.registers.set_half_carry(true);
    }

    fn reset(&mut self, value: u8, bit: BitPosition) -> u8 {
        let bit_position: u8 = bit.into();
        value & !(1 << bit_position)
    }

    fn set_bit(&mut self, value: u8, bit: BitPosition) -> u8 {
        let bit_position: u8 = bit.into();
        value | (1 << bit_position)
    }

    fn srl(&mut self, value: u8) -> u8 {
        let new_value = value >> 1;
        self.registers.set_zero(new_value == 0);
        self.registers.set_subtract(false);
        self.registers.set_half_carry(false);
        self.registers.set_carry(value & 0b1 != 0);
        new_value
    }

    fn rr(&mut self, value: u8) -> u8 {
        let carry_in = self.registers.carry() as u8;
        let new_value = (value >> 1) | (carry_in << 7);
        self.registers.set_zero(new_value == 0);
        self.registers.set_subtract(false);
        self.registers.set_half_carry(false);
        self.registers.set_carry(value & 0b1 != 0);
        new_value
    }

    fn rl(&mut self, value: u8) -> u8 {
        let carry_in = self.registers.carry() as u8;
        let new_value = (value << 1) | carry_in;
        self.registers.set_zero(new_value == 0);
        self.registers.set_subtract(false);
        self.registers.set_half_carry(false);
        self.registers.set_carry(value & 0x80 != 0);
        new_value
    }

    fn rrc(&mut self, value: u8) -> u8 {
        let new_value = value.rotate_right(1);
        self.registers.set_zero(new_value == 0);
        self.registers.set_subtract(false);
        self.registers.set_half_carry(false);
        self.registers.set_carry(value & 0b1 != 0);
        new_value
    }

    fn rlc(&mut self, value: u8) -> u8 {
        let new_value = value.rotate_left(1);
        self.registers.set_zero(new_value == 0);
        self.registers.set_subtract(false);
        self.registers.set_half_carry(false);
        self.registers.set_carry(value & 0x80 != 0);
        new_value
    }

    fn sra(&mut self, value: u8) -> u8 {
        //arithmetic shift: bit 7 (the sign bit) is preserved rather than shifted in as 0
        let new_value = (value >> 1) | (value & 0x80);
        self.registers.set_zero(new_value == 0);
        self.registers.set_subtract(false);
        self.registers.set_half_carry(false);
        self.registers.set_carry(value & 0b1 != 0);
        new_value
    }

    fn sla(&mut self, value: u8) -> u8 {
        let new_value = value << 1;
        self.registers.set_zero(new_value == 0);
        self.registers.set_subtract(false);
        self.registers.set_half_carry(false);
        self.registers.set_carry(value & 0x80 != 0);
        new_value
    }

    fn swap(&mut self, value: u8) -> u8 {
        let new_value = (value << 4) | (value >> 4);
        self.registers.set_zero(new_value == 0);
        self.registers.set_subtract(false);
        self.registers.set_half_carry(false);
        self.registers.set_carry(false);
        new_value
    }
}
