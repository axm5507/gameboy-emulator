//The memory bus is what the CPU talks to whenever it needs to read or write memory.
//The Game Boy has a 16 bit address space, so addresses run from 0x0000 to 0xFFFF
//inclusive. That is 0x10000 (65,536) distinct addresses, which means the backing
//array needs 0x10000 bytes.
//we route VRAM to the GPU so it can keep its decoded tile set in sync
use crate::gpu::{GPU, VRAM_BEGIN, VRAM_END};
use crate::interrupts::{Interrupt, INTERRUPT_FLAG_ADDRESS};

pub struct MemoryBus {
    memory: [u8; 0x10000],
    pub gpu: GPU,
}

impl MemoryBus {
    //we start with all of memory zeroed out
    pub fn new() -> Self {
        Self {
            memory: [0; 0x10000],
            gpu: GPU::new(),
        }
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        let address = address as usize;
        match address {
            //VRAM reads come from the GPU. The GPU works in VRAM relative indices, so
            //we subtract the base address before handing it over
            VRAM_BEGIN..=VRAM_END => self.gpu.read_vram(address - VRAM_BEGIN),
            _ => self.memory[address],
        }
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        let address = address as usize;
        match address {
            //VRAM writes go to the GPU, which also updates its cached tile set
            VRAM_BEGIN..=VRAM_END => self.gpu.write_vram(address - VRAM_BEGIN, value),
            _ => self.memory[address] = value,
        }
    }
    //Flag an interrupt as requested by setting its bit in the IF register. This is how
    //hardware signals the CPU that it needs attention
    pub fn request_interrupt(&mut self, interrupt: Interrupt) {
        let flags = self.read_byte(INTERRUPT_FLAG_ADDRESS);
        self.write_byte(INTERRUPT_FLAG_ADDRESS, flags | interrupt.bit());
    }

    
}
