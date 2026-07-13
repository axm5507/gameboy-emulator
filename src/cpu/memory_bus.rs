//The memory bus is what the CPU talks to whenever it needs to read or write memory.
//The Game Boy has a 16 bit address space, so addresses run from 0x0000 to 0xFFFF
//inclusive. That is 0x10000 (65,536) distinct addresses, which means the backing
//array needs 0x10000 bytes.
//we route VRAM to the GPU so it can keep its decoded tile set in sync
use crate::gpu::{BGP_ADDRESS, GPU, LCDC_ADDRESS, LYC_ADDRESS, VRAM_BEGIN, VRAM_END, WX_ADDRESS,};
use crate::interrupts::{INTERRUPT_FLAG_ADDRESS, Interrupt};
use crate::joypad::{Button, JOYPAD_ADDRESS, Joypad};
use crate::timer::{DIV_ADDRESS, TAC_ADDRESS, Timer};

pub struct MemoryBus {
    memory: [u8; 0x10000],
    pub gpu: GPU,
    pub timer: Timer,
    pub joypad: Joypad,
}

impl MemoryBus {
    //we start with all of memory zeroed out
    pub fn new() -> Self {
        Self {
            memory: [0; 0x10000],
            gpu: GPU::new(),
            timer: Timer::new(),
            joypad: Joypad::new(),
        }
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        match address {
            JOYPAD_ADDRESS => self.joypad.read(),
            DIV_ADDRESS..=TAC_ADDRESS => self.timer.read(address),
            //The LCD registers, minus DMA (0xFF46), which stays in flat memory for now
            LCDC_ADDRESS..=LYC_ADDRESS | BGP_ADDRESS..=WX_ADDRESS => self.gpu.read_register(address),
            _ => {
                let address = address as usize;
                match address {
                    //VRAM reads come from the gpu
                    VRAM_BEGIN..=VRAM_END => self.gpu.read_vram(address - VRAM_BEGIN),
                    _ => self.memory[address],
                }
            }
        }
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            JOYPAD_ADDRESS => self.joypad.write(value),
            DIV_ADDRESS..=TAC_ADDRESS => self.timer.write(address, value),
            LCDC_ADDRESS..=LYC_ADDRESS | BGP_ADDRESS..=WX_ADDRESS => {
                self.gpu.write_register(address, value)
            }
            _ => {
                let address = address as usize;
                match address {
                    //VRAM writes go to the gpu
                    VRAM_BEGIN..=VRAM_END => self.gpu.write_vram(address - VRAM_BEGIN, value),
                    _ => self.memory[address] = value,
                }
            }
        }
    }

    //This advanced cycle driven peripherals by cycles T cycles, raising interrupts they might generate
    //The CPU calls this after every step so timer and PPU stays in lockstep with instruction execution
        pub fn tick(&mut self, cycles: u8) {
        if self.timer.step(cycles) {
            self.request_interrupt(Interrupt::Timer);
        }
        let ppu = self.gpu.step(cycles);
        if ppu.vblank {
            self.request_interrupt(Interrupt::VBlank);
        }
        if ppu.stat {
            self.request_interrupt(Interrupt::LcdStat);
        }
    }
    
    //Flag an interrupt as requested by setting its bit in the IF register. This is how
    //hardware signals the CPU that it needs attention
    pub fn request_interrupt(&mut self, interrupt: Interrupt) {
        let flags = self.read_byte(INTERRUPT_FLAG_ADDRESS);
        self.write_byte(INTERRUPT_FLAG_ADDRESS, flags | interrupt.bit());
    }

    pub fn load_rom(&mut self, data: &[u8]) {
        let len = data.len().min(0x8000);
        self.memory[..len].copy_from_slice(&data[..len]);
    }
    
    pub fn press_button(&mut self, button: Button) {
        if self.joypad.press(button) {
            self.request_interrupt(Interrupt::Joypad);
        }
    }

    pub fn release_button(&mut self, button: Button) {
        self.joypad.release(button);
    }

    
}
