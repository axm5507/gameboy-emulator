use crate::cpu::instruction::{
    ADDHLTarget, ArithmeticTarget, BitPosition, Indirect, Instruction, JumpTest, LoadByteSource,
    LoadByteTarget, LoadType, LoadWordTarget, StackTarget, WordRegister,
};
use crate::cpu::memory_bus::MemoryBus;
use crate::cpu::registers::Registers;
use crate::interrupts::{INTERRUPT_ENABLE_ADDRESS, INTERRUPT_FLAG_ADDRESS, Interrupt};

pub struct CPU {
    pub registers: Registers,
    pub bus: MemoryBus,
    //When HALT is true, the CPU is paused. In real hardware an interrupt clears this
    //but I have yet to implement that so nothing clears it back for now
    pub is_halted: bool,
    //the IME is the interrupt master enable, which is a switch that gates whether any 
    //interrupt gets serviced. Its toggled by DI, EI, or RETI
    pub ime: bool,
    //EI enables interrupts one instruction late so it schedules enable here and 
    //step() applies it after the following instruction runs
    ime_pending: bool,
    
}

impl CPU {
    pub fn new() -> Self {
        Self {
            registers: Registers::new(),
            bus: MemoryBus::new(),
            is_halted: false,
            ime: false,
            ime_pending: false,
        }
    }

    //This is to put the CPU into the state the DMG boot ROM leaves behind so a cartridge can start
    //executing at 0x100 as if the boot sequence already ran. I'm not emulating the boot ROM itself,
    //these are its handoff values. This is called after loading a real ROM
    pub fn skip_boot_rom(&mut self) {
        self.registers.a = 0x01;
        self.registers.f = 0xB0; // Z and H and C set, N clear
        self.registers.b = 0x00;
        self.registers.c = 0x13;
        self.registers.d = 0x00;
        self.registers.e = 0xD8;
        self.registers.h = 0x01;
        self.registers.l = 0x4D;
        self.registers.sp = 0xFFFE;
        self.registers.pc = 0x0100; // the cartridge entry point

        //A few I/O registers the boot ROM leaves set 
        self.bus.write_byte(0xFF40, 0x91); // LCDC: LCD + BG on
        self.bus.write_byte(0xFF47, 0xFC); // BGP
        self.bus.write_byte(0xFF48, 0xFF); // OBP0
        self.bus.write_byte(0xFF49, 0xFF); // OBP1
    }


    //run one CPU step and keep rest of machine in sync
    pub fn step(&mut self) -> u8 {
        let cycles = self.run_next_step();
        self.bus.tick(cycles);
        cycles
    }


    
    //if you're familiar with how a CPU works, you'll recognize this as a fetch-decode-execute
    //cycle. First, we fetch(read the opcode byte that the program counter points at), then we
    //decode(turn the byte into an Instruction), then we execute(run it, which also tells us where
    //the program counter should go next), and finally, we advance by moving the program counter to
    //the next address.
    //I now made it so it returns the number of T cycles(4.19 MHz clock ticks) the instruction took,
    //which is what the rest of the stuff I am going to implement(PPU, timer) will be clocked against
    fn run_next_step(&mut self) -> u8 {
        let pending = self.pending_interrupt();

        //A halted CPU wakes as soon as any enabled interrupt is requested, even when
        //the master enable (IME) is off
        if self.is_halted {
            if pending.is_some() {
                self.is_halted = false;
            } else {
                return 4;
            }
        }

        //If interrupts are globally enabled and one is pending, service it instead of
        //running a normal instruction this step
        if self.ime {
            if let Some(interrupt) = pending {
                return self.service_interrupt(interrupt);
            }
        }

        //An EI on the previous step enables IME after the following instruction(the
        //one-instruction delay)
        let enable_ime_after = self.ime_pending;

        let mut instruction_byte = self.bus.read_byte(self.registers.pc);


        //0xcb is a prefix opcode. When the processor encounters it, it knows to read the
        //next byte in memory and execute an extended bitwise operation. When we read the
        //following byte we remember we're prefixed so we decode against the right table
        //and account for the extra byte later
        let prefixed = instruction_byte == 0xCB;
        if prefixed {
            instruction_byte = self.bus.read_byte(self.registers.pc.wrapping_add(1));
        }

        let (next_pc, cycles) =
            if let Some(instruction) = Instruction::from_byte(instruction_byte, prefixed) {
                //Work out the timing before executing. The only instructions whose cost
                //depends on runtime state are conditional branches, and those don't touch
                //the flags they test, so measuring first gives the same answer.
                let cycles = self.instruction_cycles(instruction);
                (self.execute(instruction), cycles)
            } else {
                let description =
                    format!("0x{}{:02x}", if prefixed { "cb" } else { "" }, instruction_byte);
                panic!("Unknown instruction found for: {}", description);
            };

        self.registers.pc = next_pc;

        //Apply the delayed EI
        if enable_ime_after && self.ime_pending {
            self.ime = true;
            self.ime_pending = false;
        }

        cycles
    }

    //This finds the highest priority interrupt that is both enabled(IE) and requested(IF),
    //or None if there is nothing to service
    fn pending_interrupt(&self) -> Option<Interrupt> {
        let enabled = self.bus.read_byte(INTERRUPT_ENABLE_ADDRESS);
        let requested = self.bus.read_byte(INTERRUPT_FLAG_ADDRESS);
        let active = enabled & requested;
        if active == 0 {
            return None;
        }
        Interrupt::PRIORITY
            .into_iter()
            .find(|interrupt| active & interrupt.bit() != 0)
    }

    //This services an interrupt. turn off IME(so the handler runs uninterrupted until it
    //chooses otherwise), clear this interrupt's request bit so it isn't handled twice,
    //then push the current pc and jump to the vector. The whole sequence costs 20 T-cycles
    fn service_interrupt(&mut self, interrupt: Interrupt) -> u8 {
        self.ime = false;
        let flags = self.bus.read_byte(INTERRUPT_FLAG_ADDRESS);
        self.bus
            .write_byte(INTERRUPT_FLAG_ADDRESS, flags & !interrupt.bit());

        self.push(self.registers.pc);
        self.registers.pc = interrupt.vector();
        20
    }

    //This is for how many T cycles an instruction takes. Most are fixed by the opcode, but a few
    //vary. (HL)/immediate operands are slower than register operands, and conditional branches
    //cost more when the branch is actually taken
    fn instruction_cycles(&self, instruction: Instruction) -> u8 {
        match instruction {
            //Accumulator ALU: 4 for a register operand, 8 for (HL) or an immediate byte
            Instruction::ADD(t)
            | Instruction::ADC(t)
            | Instruction::SUB(t)
            | Instruction::SBC(t)
            | Instruction::AND(t)
            | Instruction::OR(t)
            | Instruction::XOR(t)
            | Instruction::CP(t) => match t {
                ArithmeticTarget::HLI | ArithmeticTarget::D8 => 8,
                _ => 4,
            },
            Instruction::ADDHL(_) => 8,
            //8bit INC/DEC, 4 on a register, 12 on (HL) (read, modify, write back)
            Instruction::INC(t) | Instruction::DEC(t) => match t {
                ArithmeticTarget::HLI => 12,
                _ => 4,
            },
            Instruction::INC16(_) | Instruction::DEC16(_) => 8,
            Instruction::ADDSP => 16,
            Instruction::CCF
            | Instruction::SCF
            | Instruction::CPL
            | Instruction::DAA
            | Instruction::RRA
            | Instruction::RLA
            | Instruction::RRCA
            | Instruction::RLCA
            | Instruction::NOP
            | Instruction::HALT
            | Instruction::STOP => 4,
            Instruction::RST(_) => 16,
            //CB ops, 8 on a register, 16 on (HL). BIT is the exception, it only reads and
            //never writes back, so BIT b, (HL) is 12 rather than 16
            Instruction::BIT(t, _) => match t {
                ArithmeticTarget::HLI => 12,
                _ => 8,
            },
            Instruction::RESET(t, _)
            | Instruction::SET(t, _)
            | Instruction::SRL(t)
            | Instruction::RR(t)
            | Instruction::RL(t)
            | Instruction::RRC(t)
            | Instruction::RLC(t)
            | Instruction::SRA(t)
            | Instruction::SLA(t)
            | Instruction::SWAP(t) => match t {
                ArithmeticTarget::HLI => 16,
                _ => 8,
            },
            //For jumps and calls the taken path costs more than falling through
            Instruction::JP(test) => {
                if self.should_jump(test) {
                    16
                } else {
                    12
                }
            }
            Instruction::JR(test) => {
                if self.should_jump(test) {
                    12
                } else {
                    8
                }
            }
            Instruction::JPI => 4,
            Instruction::CALL(test) => {
                if self.should_jump(test) {
                    24
                } else {
                    12
                }
            }
            //Unconditional RET (0xC9) is 16. a conditional RET is 20 taken/8 not taken
            Instruction::RET(test) => match test {
                JumpTest::Always => 16,
                _ => {
                    if self.should_jump(test) {
                        20
                    } else {
                        8
                    }
                }
            },
            Instruction::PUSH(_) => 16,
            Instruction::POP(_) => 12,
            Instruction::LD(load_type) => Self::load_cycles(load_type),
        }
    }

    //Timing for the load family, I split out since there are so many shapes
    fn load_cycles(load_type: LoadType) -> u8 {
        match load_type {
            LoadType::Byte(target, source) => match (target, source) {
                (LoadByteTarget::HLI, LoadByteSource::D8) => 12, // LD (HL), d8
                (LoadByteTarget::HLI, _) => 8, // LD (HL), r
                (_, LoadByteSource::HLI) => 8, // LD r, (HL)
                (_, LoadByteSource::D8) => 8, // LD r, d8
                _ => 4, // LD r, r'
            },
            LoadType::Word(_) => 12,
            //Register pair indirects are 8 and going through a full 16 bit address is 16
            LoadType::AFromIndirect(indirect) | LoadType::IndirectFromA(indirect) => match indirect {
                Indirect::WordIndirect => 16,
                _ => 8,
            },
            LoadType::AFromByteAddress | LoadType::ByteAddressFromA => 12,
            LoadType::SPFromHL => 8,
            LoadType::IndirectFromSP => 20,
            LoadType::HLFromSPPlus => 12,
        }
    }


    
    //Now to write the execute function that contains the logic for each instruction mentioned in the instructions file
    //It now returns the address the program counter should move to after this instruction
    pub fn execute(&mut self, instruction: Instruction) -> u16 {
        match instruction {
            Instruction::ADD(target) => {
                let value = self.get_register(target);
                let new_value = self.add(value);
                self.registers.a = new_value;
                //self.registers.pc.wrapping_add(1)
                //replaced the code above with the code below for ADD and a few more ops below because
                //when they gained the d8 operand form, their length became variable so the result can
                //also be 2 bytes now
                self.arithmetic_next_pc(target)
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
                self.arithmetic_next_pc(target)
            }
            Instruction::SUB(target) => {
                let value = self.get_register(target);
                let new_value = self.sub(value);
                self.registers.a = new_value;
                self.arithmetic_next_pc(target)
            }
            Instruction::SBC(target) => {
                let value = self.get_register(target);
                let new_value = self.sbc(value);
                self.registers.a = new_value;
                self.arithmetic_next_pc(target)
            }
            Instruction::AND(target) => {
                let value = self.get_register(target);
                let new_value = self.and(value);
                self.registers.a = new_value;
                self.arithmetic_next_pc(target)
            }
            Instruction::OR(target) => {
                let value = self.get_register(target);
                let new_value = self.or(value);
                self.registers.a = new_value;
                self.arithmetic_next_pc(target)
            }
            Instruction::XOR(target) => {
                let value = self.get_register(target);
                let new_value = self.xor(value);
                self.registers.a = new_value;
                self.arithmetic_next_pc(target)
            }
            Instruction::CP(target) => {
                let value = self.get_register(target);
                self.sub(value);
                self.arithmetic_next_pc(target)
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
            Instruction::INC16(target) => {
                let new_value = self.get_word_register(target).wrapping_add(1);
                self.set_word_register(target, new_value);
                self.registers.pc.wrapping_add(1)
            }
            Instruction::DEC16(target) => {
                let new_value = self.get_word_register(target).wrapping_sub(1);
                self.set_word_register(target, new_value);
                self.registers.pc.wrapping_add(1)
            }
            Instruction::ADDSP => {
                let offset = self.read_next_byte() as i8;
                self.registers.sp = self.add_sp_offset(offset);
                self.registers.pc.wrapping_add(2)
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
            Instruction::DAA => {
                self.daa();
                self.registers.pc.wrapping_add(1)
            }
            Instruction::NOP => self.registers.pc.wrapping_add(1),
            Instruction::HALT => {
                self.is_halted = true;
                self.registers.pc.wrapping_add(1)
            }
            //STOP is really a 2-byte opcode (0x10 0x00). I'm modelling it like HALT for now
            //since I have neither the joypad nor the speed switch it interacts with
            Instruction::STOP => {
                self.is_halted = true;
                self.registers.pc.wrapping_add(2)
            }
            //RST is a one byte call to a fixed vector: push the return address (pc + 1)
            //and jump to the vector
            Instruction::RST(address) => {
                let next_pc = self.registers.pc.wrapping_add(1);
                self.push(next_pc);
                address
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
            Instruction::PUSH(target) => {
                let value = match target {
                    StackTarget::BC => self.registers.bc(),
                    StackTarget::DE => self.registers.de(),
                    StackTarget::HL => self.registers.hl(),
                    StackTarget::AF => self.registers.af(),
                };
                self.push(value);
                self.registers.pc.wrapping_add(1)
            }
            Instruction::POP(target) => {
                let value = self.pop();
                match target {
                    StackTarget::BC => self.registers.set_bc(value),
                    StackTarget::DE => self.registers.set_de(value),
                    StackTarget::HL => self.registers.set_hl(value),
                    //set_af masks off the low nibble of F, keeping those bits 0 as the hardware does
                    StackTarget::AF => self.registers.set_af(value),
                }
                self.registers.pc.wrapping_add(1)
            }
            Instruction::POP(target) => {
                let value = self.pop();
                match target {
                    StackTarget::BC => self.registers.set_bc(value),
                    StackTarget::DE => self.registers.set_de(value),
                    StackTarget::HL => self.registers.set_hl(value),
                    //set_af masks off the low nibble of F, keeping those bits 0 as
                    //the hardware does even if a bogus value was popped in.
                    StackTarget::AF => self.registers.set_af(value),
                }
                self.registers.pc.wrapping_add(1)
            }
            Instruction::CALL(test) => {
                let should_jump = self.should_jump(test);
                self.call(should_jump)
            }
            Instruction::RET(test) => {
                let should_jump = self.should_jump(test);
                self.return_(should_jump)
            }
            //DI disables interrupts immediately and cancels any EI still waiting
            //to take effect
            Instruction::DI => {
                self.ime = false;
                self.ime_pending = false;
                self.registers.pc.wrapping_add(1)
            }
            //EI schedules the enable, which step() applies after the following instruction
            Instruction::EI => {
                self.ime_pending = true;
                self.registers.pc.wrapping_add(1)
            }
            //RETI is RET plus an immediate re enable of interrupts
            Instruction::RETI => {
                self.ime = true;
                self.pop()
            }
            
        }
    }

    //This is to call a function. Its a jump that first saves where to come back to. it
    //pushes the address of the instruction right after the CALL (pc + 3, since CALL
    //is 3 bytes) onto the stack, then jumps to the target address that follows the
    //opcode. If the condition fails we just step over the whole 3-byte instruction
    fn call(&mut self, should_jump: bool) -> u16 {
        let next_pc = self.registers.pc.wrapping_add(3);
        if should_jump {
            self.push(next_pc);
            self.read_next_word()
        } else {
            next_pc
        }
    }

    //This returns from a function by popping the saved return address back into the pc.
    //If the condition fails we simply move on past the 1-byte RET
    fn return_(&mut self, should_jump: bool) -> u16 {
        if should_jump {
            self.pop()
        } else {
            self.registers.pc.wrapping_add(1)
        }
    }


    
    //This is to push a 16 bit value onto stack. Since the game boy grows downwards
    //we move SP down before each write and store the high byte first so after both
    //writes the low byte sits at the lower addres(little endian)
    fn push(&mut self, value: u16) {
        self.registers.sp = self.registers.sp.wrapping_sub(1);
        self.bus.write_byte(self.registers.sp, (value >> 8) as u8);

        self.registers.sp = self.registers.sp.wrapping_sub(1);
        self.bus.write_byte(self.registers.sp, value as u8);
    }

    //This is to pop a 16 bit value off the stack, the mirror image of push. The low byte is on
    //top at the lower address, so we read it first, then the high byte, moving SP up as we go
    fn pop(&mut self) -> u16 {
        let low = self.bus.read_byte(self.registers.sp) as u16;
        self.registers.sp = self.registers.sp.wrapping_add(1);

        let high = self.bus.read_byte(self.registers.sp) as u16;
        self.registers.sp = self.registers.sp.wrapping_add(1);

        (high << 8) | low
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
            LoadType::SPFromHL => {
                self.registers.sp = self.registers.hl();
                self.registers.pc.wrapping_add(1)
            }
            LoadType::IndirectFromSP => {
                let address = self.read_next_word();
                let sp = self.registers.sp;
                //write SP out little endian. low byte first, then high byte
                self.bus.write_byte(address, sp as u8);
                self.bus.write_byte(address.wrapping_add(1), (sp >> 8) as u8);
                self.registers.pc.wrapping_add(3)
            }
            LoadType::HLFromSPPlus => {
                let offset = self.read_next_byte() as i8;
                let value = self.add_sp_offset(offset);
                self.registers.set_hl(value);
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
            //(HL) reads the byte HL points at and D8 reads the immediate byte after the opcode
            ArithmeticTarget::HLI => self.bus.read_byte(self.registers.hl()),
            ArithmeticTarget::D8 => self.read_next_byte(),
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
            //(HL) writes back to the byte HL points at. Writing to D8 is meaningless because
            //an immediate is never a destination so no instruction ever reaches it
            ArithmeticTarget::HLI => self.bus.write_byte(self.registers.hl(), value),
            ArithmeticTarget::D8 => unreachable!("an immediate byte is never a write target"),
        }
    }
    //This is how far the pc advances after an accumulator ALU op, it
    //depends only on whether the operand was an immediate byte or not
    fn arithmetic_next_pc(&self, target: ArithmeticTarget) -> u16 {
        match target {
            ArithmeticTarget::D8 => self.registers.pc.wrapping_add(2),
            _ => self.registers.pc.wrapping_add(1),
        }
    }

    fn get_word_register(&self, target: WordRegister) -> u16 {
        match target {
            WordRegister::BC => self.registers.bc(),
            WordRegister::DE => self.registers.de(),
            WordRegister::HL => self.registers.hl(),
            WordRegister::SP => self.registers.sp,
        }
    }

    fn set_word_register(&mut self, target: WordRegister, value: u16) {
        match target {
            WordRegister::BC => self.registers.set_bc(value),
            WordRegister::DE => self.registers.set_de(value),
            WordRegister::HL => self.registers.set_hl(value),
            WordRegister::SP => self.registers.sp = value,
        }
    }

    //This adds a signed byte to SP and computes the flags the way the hardware does.
    //both the half carry and carry come from adding unsigned low byte of SP to the 
    //offset byte while the actual result uses the sign extended offset
    fn add_sp_offset(&mut self, offset: i8) -> u16 {
        let sp = self.registers.sp;
        let offset_byte = offset as u8 as u16; // raw unsigned byte
        self.registers.set_zero(false);
        self.registers.set_subtract(false);
        self.registers
            .set_half_carry((sp & 0x0F) + (offset_byte & 0x0F) > 0x0F);
        self.registers.set_carry((sp & 0xFF) + (offset_byte & 0xFF) > 0xFF);
        //offset as u16 sign extends the i8 so the result can handle negative offsets
        sp.wrapping_add(offset as u16)
    }

    //DAA adjusts A into a valid binary coded decimal result after an add or subtract,
    //using the subtract/half-carry/carry flags left behind by that operation
    fn daa(&mut self) {
        let mut a = self.registers.a;
        let mut carry = self.registers.carry();
        let mut adjust = 0u8;

        //A half carry or low nibble that overflowed 9 during an add needs +/- 0x06
        if self.registers.half_carry()
            || (!self.registers.subtract() && (a & 0x0F) > 0x09)
        {
            adjust |= 0x06;
        }
        //A carry or value that overflowed 0x99 during an add needs +/- 0x60, and
        //that sets carry flag for the result
        if carry || (!self.registers.subtract() && a > 0x99) {
            adjust |= 0x60;
            carry = true;
        }

        //After a subtract we undo the adjustment and after an add we apply it
        a = if self.registers.subtract() {
            a.wrapping_sub(adjust)
        } else {
            a.wrapping_add(adjust)
        };

        self.registers.a = a;
        self.registers.set_zero(a == 0);
        self.registers.set_half_carry(false);
        self.registers.set_carry(carry);
        //the subtract (N) flag is left as it was
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
