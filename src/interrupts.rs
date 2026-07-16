//the gameboy has 5 interrupt sources. Each owns 1 bit, shared between 2 registers. 
//IE(interrupt enable) says which interrupts the program cares about, and IF(interrupt flag)
//says which have actually been requested. WHen the master switch(IME, cpu internal) is on 
//and a source is enabled and requested, the CPU will jump to that sources fixed handler
//address. A lower bit number means higher priority

pub const INTERRUPT_ENABLE_ADDRESS: u16 = 0xFFFF; // IE
pub const INTERRUPT_FLAG_ADDRESS: u16 = 0xFF0F; // IF

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interrupt {
    VBlank,
    LcdStat,
    Timer,
    Serial,
    Joypad,
}

impl Interrupt {
    //Highest priority first, the CPU services the earliest source in this list that is
    //both enabled (IE) and requested (IF)
    pub const PRIORITY: [Interrupt; 5] = [
        Interrupt::VBlank,
        Interrupt::LcdStat,
        Interrupt::Timer,
        Interrupt::Serial,
        Interrupt::Joypad,
    ];

    //The single bit this interrupt occupies in the IE and IF registers
    pub fn bit(self) -> u8 {
        match self {
            Interrupt::VBlank => 1 << 0,
            Interrupt::LcdStat => 1 << 1,
            Interrupt::Timer => 1 << 2,
            Interrupt::Serial => 1 << 3,
            Interrupt::Joypad => 1 << 4,
        }
    }

    //The address the CPU jumps to when it services this interrupt
    pub fn vector(self) -> u16 {
        match self {
            Interrupt::VBlank => 0x40,
            Interrupt::LcdStat => 0x48,
            Interrupt::Timer => 0x50,
            Interrupt::Serial => 0x58,
            Interrupt::Joypad => 0x60,
        }
    }
}
