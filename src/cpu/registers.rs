//I did some research on the gameboy's cpu structure and found that it is an 8bit cpu,
//which means that each register can hold 8 bits(1 byte) of data. However, there are also
//some instructions that allow a game to read 16 bits(2 bytes) of data at a time, which is
//why registers are paired together to form virtual 16 bit registers. 
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Registers {
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub f: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
}

//the f register is called the flags register. It is unique because the lower 4 bits are always
//0s, and the CPU writes to the upper 4 bits when things happen. Bit 4 is carry, bit 5 is half carry,
//bit 6 is subtract, and bit 7 is zero. 
pub const FLAG_ZERO: u8 = 1 << 7;
pub const FLAG_SUBTRACT: u8 = 1 << 6;
pub const FLAG_HALF_CARRY: u8 = 1 << 5;
pub const FLAG_CARRY: u8 = 1 << 4;


//This is to ensure the lower 4 bits of the F register are always 0s
const FLAGS_MASK: u8 = 0xF0;


impl Registers {
    //This creates a new register file with every register zeroed
    pub fn new() -> Self {
        Self::default()
    }

    //for 16 bit registers, you treat the first register as a u16 which just adds a byte of all 
    //0s to the most significant position of the number. Then you shift the first register 8 
    //positions so it's occupying the most significant position, and then bitwise OR the second
    //register. Then you get a 2 byte number with the first register in the most significant 
    //position and the second register in the least significant position. 

    pub fn af(&self) -> u16 {
        (self.a as u16) << 8 | self.f as u16
    }

    pub fn bc(&self) -> u16 {
        (self.b as u16) << 8 | self.c as u16
    }

    pub fn de(&self) -> u16 {
        (self.d as u16) << 8 | self.e as u16
    }

    pub fn hl(&self) -> u16 {
        (self.h as u16) << 8 | self.l as u16
    }



    //when setting the AF register, the lower 4 bits of F need to be masked off to stay
    //in sync with the hardware when those bits always read back as 0
    pub fn set_af(&mut self, value: u16) {
        self.a = (value >> 8) as u8;
        self.f = (value as u8) & FLAGS_MASK;
    }

    pub fn set_bc(&mut self, value: u16) {
        self.b = (value >> 8) as u8;
        self.c = value as u8;
    }

    pub fn set_de(&mut self, value: u16) {
        self.d = (value >> 8) as u8;
        self.e = value as u8;
    }

    pub fn set_hl(&mut self, value: u16) {
        self.h = (value >> 8) as u8;
        self.l = value as u8;
    }

    //These are the flag getters for the F register

    pub fn zero(&self) -> bool {
        self.f & FLAG_ZERO != 0
    }

    pub fn subtract(&self) -> bool {
        self.f & FLAG_SUBTRACT != 0
    }

    pub fn half_carry(&self) -> bool {
        self.f & FLAG_HALF_CARRY != 0
    }

    pub fn carry(&self) -> bool {
        self.f & FLAG_CARRY != 0
    }

    //Flag setters for F register, either sets or clears the flag

    pub fn set_zero(&mut self, on: bool) {
        self.set_flag(FLAG_ZERO, on);
    }

    pub fn set_subtract(&mut self, on: bool) {
        self.set_flag(FLAG_SUBTRACT, on);
    }

    pub fn set_half_carry(&mut self, on: bool) {
        self.set_flag(FLAG_HALF_CARRY, on);
    }

    pub fn set_carry(&mut self, on: bool) {
        self.set_flag(FLAG_CARRY, on);
    }

    fn set_flag(&mut self, flag: u8, on: bool) {
        if on {
            self.f |= flag;
        } else {
            self.f &= !flag;
        }
    }
}

