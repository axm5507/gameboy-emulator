//The memory bus is what the CPU talks to whenever it needs to read or write memory.
//The Game Boy has a 16 bit address space, so addresses run from 0x0000 to 0xFFFF
//inclusive. That is 0x10000 (65,536) distinct addresses, which means the backing
//array needs 0x10000 bytes.
//we route VRAM to the GPU so it can keep its decoded tile set in sync
use crate::cartridge::Cartridge;
use crate::gpu::{BGP_ADDRESS, DMA_ADDRESS, GPU, LCDC_ADDRESS, LYC_ADDRESS, OAM_BEGIN, OAM_END, VRAM_BEGIN,
    VRAM_END, WX_ADDRESS};
use crate::interrupts::{INTERRUPT_FLAG_ADDRESS, Interrupt};
use crate::joypad::{Button, JOYPAD_ADDRESS, Joypad};
use crate::timer::{DIV_ADDRESS, TAC_ADDRESS, Timer};

//how many bytes an OAM DMA transfer copies
const OAM_SIZE_BYTES: usize = (OAM_END - OAM_BEGIN + 1) as usize;

pub struct MemoryBus {
    memory: [u8; 0x10000],
    pub gpu: GPU,
    pub timer: Timer,
    pub joypad: Joypad,
    cartridge: Option<Cartridge>,
}

impl MemoryBus {
    //we start with all of memory zeroed out
    pub fn new() -> Self {
        Self {
            memory: [0; 0x10000],
            gpu: GPU::new(),
            timer: Timer::new(),
            joypad: Joypad::new(),
            cartridge: None,
        }
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        match address {
            //ROM banks and cartridge RAM
            0x0000..=0x7FFF | 0xA000..=0xBFFF => match &self.cartridge {
                Some(cart) => cart.read(address),
                None => self.memory[address as usize],
            },
            OAM_BEGIN..=OAM_END => self.gpu.read_oam((address - OAM_BEGIN) as usize),
            JOYPAD_ADDRESS => self.joypad.read(),
            DIV_ADDRESS..=TAC_ADDRESS => self.timer.read(address),
            //The LCD registers, minus DMA
            LCDC_ADDRESS..=LYC_ADDRESS | BGP_ADDRESS..=WX_ADDRESS => self.gpu.read_register(address),
            _ => {
                let address = address as usize;
                match address {
                    //VRAM reads come from the GPU
                    VRAM_BEGIN..=VRAM_END => self.gpu.read_vram(address - VRAM_BEGIN),
                    _ => self.memory[address],
                }
            }
        }
    }
    pub fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            //Writes into ROM space poke the MBC's control registers; cart-RAM writes go
            //to the RAM
            0x0000..=0x7FFF | 0xA000..=0xBFFF => match &mut self.cartridge {
                Some(cart) => cart.write(address, value),
                None => self.memory[address as usize] = value,
            },
            OAM_BEGIN..=OAM_END => self.gpu.write_oam((address - OAM_BEGIN) as usize, value),
            JOYPAD_ADDRESS => self.joypad.write(value),
            DIV_ADDRESS..=TAC_ADDRESS => self.timer.write(address, value),
            //Writing DMA kicks off a bulk copy into OAM 
            DMA_ADDRESS => self.oam_dma(value),
            LCDC_ADDRESS..=LYC_ADDRESS | BGP_ADDRESS..=WX_ADDRESS => {
                self.gpu.write_register(address, value)
            }
            _ => {
                let address = address as usize;
                match address {
                    //VRAM writes go to the GPU
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

    //writing the high byte of a source address to 0xFF46, copying 160 bytes from that page into OAM
    fn oam_dma(&mut self, source_page: u8) {
        let source = (source_page as u16) << 8;
        for i in 0..OAM_SIZE_BYTES {
            let byte = self.read_byte(source + i as u16);
            self.gpu.write_oam(i, byte);
        }
    }
    
    //Flag an interrupt as requested by setting its bit in the IF register. This is how
    //hardware signals the CPU that it needs attention
    pub fn request_interrupt(&mut self, interrupt: Interrupt) {
        let flags = self.read_byte(INTERRUPT_FLAG_ADDRESS);
        self.write_byte(INTERRUPT_FLAG_ADDRESS, flags | interrupt.bit());
    }

    //Install a cartridge from its raw ROM bytes. From now on the ROM/RAM ranges are
    //served by the cartridge instead of flat memory
    pub fn load_rom(&mut self, data: &[u8]) {
        self.cartridge = Some(Cartridge::new(data.to_vec()));
    }

    //Restore battery backed cartridge RAM from a save file
    pub fn load_battery_ram(&mut self, data: &[u8]) {
        if let Some(cart) = &mut self.cartridge {
            cart.load_ram(data);
        }
    }

    //The cartridge RAM to write out to a save file, but only for battery-backed carts
    //that actually have RAM
    pub fn battery_ram(&self) -> Option<&[u8]> {
        match &self.cartridge {
            Some(cart) if cart.has_battery() && !cart.ram().is_empty() => Some(cart.ram()),
            _ => None,
        }
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
