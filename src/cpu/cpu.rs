use crate::cpu::instruction::{ADDHLTarget, ArithmeticTarget, BitPosition, Instruction};
use crate::cpu::registers::Registers;

pub struct CPU {
    pub registers: Registers,
}

impl CPU {
    pub fn new() -> Self {
        Self {
            registers: Registers::new(),
        }
    }
//Now to write the execute function that contains the logic for each instruction mentioned in the instructions file
    pub fn execute(&mut self, instruction: Instruction) {
        match instruction {
            Instruction::ADD(target) => {
                let value = self.get_register(target);
                let new_value = self.add(value);
                self.registers.a = new_value;
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
            }
            Instruction::ADC(target) => {
                let value = self.get_register(target);
                let new_value = self.adc(value);
                self.registers.a = new_value;
            }
            Instruction::SUB(target) => {
                let value = self.get_register(target);
                let new_value = self.sub(value);
                self.registers.a = new_value;
            }
            Instruction::SBC(target) => {
                let value = self.get_register(target);
                let new_value = self.sbc(value);
                self.registers.a = new_value;
            }
            Instruction::AND(target) => {
                let value = self.get_register(target);
                let new_value = self.and(value);
                self.registers.a = new_value;
            }
            Instruction::OR(target) => {
                let value = self.get_register(target);
                let new_value = self.or(value);
                self.registers.a = new_value;
            }
            Instruction::XOR(target) => {
                let value = self.get_register(target);
                let new_value = self.xor(value);
                self.registers.a = new_value;
            }
            Instruction::CP(target) => {
                let value = self.get_register(target);
                self.sub(value);
            }
            Instruction::INC(target) => {
                let value = self.get_register(target);
                let new_value = self.inc(value);
                self.set_register(target, new_value);
            }
            Instruction::DEC(target) => {
                let value = self.get_register(target);
                let new_value = self.dec(value);
                self.set_register(target, new_value);
            }
            Instruction::CCF => self.ccf(),
            Instruction::SCF => self.scf(),
            Instruction::RRA => self.rra(),
            Instruction::RLA => self.rla(),
            Instruction::RRCA => self.rrca(),
            Instruction::RLCA => self.rlca(),
            Instruction::CPL => self.cpl(),
            Instruction::BIT(target, bit) => {
                let value = self.get_register(target);
                self.bit(value, bit);
            }
            Instruction::RESET(target, bit) => {
                let value = self.get_register(target);
                let new_value = self.reset(value, bit);
                self.set_register(target, new_value);
            }
            Instruction::SET(target, bit) => {
                let value = self.get_register(target);
                let new_value = self.set_bit(value, bit);
                self.set_register(target, new_value);
            }
            Instruction::SRL(target) => {
                let value = self.get_register(target);
                let new_value = self.srl(value);
                self.set_register(target, new_value);
            }
            Instruction::RR(target) => {
                let value = self.get_register(target);
                let new_value = self.rr(value);
                self.set_register(target, new_value);
            }
            Instruction::RL(target) => {
                let value = self.get_register(target);
                let new_value = self.rl(value);
                self.set_register(target, new_value);
            }
            Instruction::RRC(target) => {
                let value = self.get_register(target);
                let new_value = self.rrc(value);
                self.set_register(target, new_value);
            }
            Instruction::RLC(target) => {
                let value = self.get_register(target);
                let new_value = self.rlc(value);
                self.set_register(target, new_value);
            }
            Instruction::SRA(target) => {
                let value = self.get_register(target);
                let new_value = self.sra(value);
                self.set_register(target, new_value);
            }
            Instruction::SLA(target) => {
                let value = self.get_register(target);
                let new_value = self.sla(value);
                self.set_register(target, new_value);
            }
            Instruction::SWAP(target) => {
                let value = self.get_register(target);
                let new_value = self.swap(value);
                self.set_register(target, new_value);
            }
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
