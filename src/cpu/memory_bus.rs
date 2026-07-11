//The memory bus is what the CPU talks to whenever it needs to read or write memory.
//The Game Boy has a 16 bit address space, so addresses run from 0x0000 to 0xFFFF
//inclusive. That is 0x10000 (65,536) distinct addresses, which means the backing
//array needs 0x10000 bytes.
//I'm keeping this simple for now on purpose, with it being just a flat block of RAM. Later
//this is where cartridge ROM, video memory, I/O registers, and other stuff will get 
//mapped into their proper regions of the address space.
pub struct MemoryBus {
    memory: [u8; 0x10000],
}

impl MemoryBus {
    //we start with all of memory zeroed out
    pub fn new() -> Self {
        Self {
            memory: [0; 0x10000],
        }
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        self.memory[address as usize]
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        self.memory[address as usize] = value;
    }
}
