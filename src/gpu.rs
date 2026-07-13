//All of the stuff you see on the screen of a gameboy is an 8x8 pixel tile. VRAM stores these
//tiles in a 2 bit per pixel form that is kind of awkward to read 1 pixel at a time. So instead of
//decoding that packed form everytime I want to draw, the GPU keeps a decoded copy of every tile and
//re decodes a tile row each time the CPU writes to the tile data area of VRAM. This module owns VRAM
//and that cache and Ill implement actual rendering next.

//VRAM mapped into CPU address space, memory bus routes read and write in this range here
pub const VRAM_BEGIN: usize = 0x8000;
pub const VRAM_END: usize = 0x9FFF;
pub const VRAM_SIZE: usize = VRAM_END - VRAM_BEGIN + 1; // 0x2000 = 8192 bytes

//The packed pixel data lives in the first 0x1800 bytes of VRAM
//Above that are the tile maps
const TILE_DATA_SIZE: usize = 0x1800;

//The tile-data region holds 384 tiles: 0x1800 bytes / 16 bytes per tile
const TILE_COUNT: usize = TILE_DATA_SIZE / 16; 

//A single pixel is 2 bits, so it is one of four values. The Game Boy is greyscale and
//these get mapped to actual shades through a palette at render time and here I just keep
//the raw 0-3 value
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TilePixelValue {
    Zero,
    One,
    Two,
    Three,
}

//A tile is 8x8 pixels
pub type Tile = [[TilePixelValue; 8]; 8];

//A blank tile
fn empty_tile() -> Tile {
    [[TilePixelValue::Zero; 8]; 8]
}

//LCD registers are at 0xFF40 to 0xFF4B. Memory bus routes this change here
pub const LCDC_ADDRESS: u16 = 0xFF40; // LCD control
pub const STAT_ADDRESS: u16 = 0xFF41; // LCD status
pub const SCY_ADDRESS: u16 = 0xFF42; // background scroll Y
pub const SCX_ADDRESS: u16 = 0xFF43; // background scroll X
pub const LY_ADDRESS: u16 = 0xFF44; // current scanline (read-only)
pub const LYC_ADDRESS: u16 = 0xFF45; // scanline compare
pub const DMA_ADDRESS: u16 = 0xFF46; // OAM DMA (handled by the bus, not here)
pub const BGP_ADDRESS: u16 = 0xFF47; // background palette
pub const OBP0_ADDRESS: u16 = 0xFF48; // object palette 0
pub const OBP1_ADDRESS: u16 = 0xFF49; // object palette 1
pub const WY_ADDRESS: u16 = 0xFF4A; // window Y
pub const WX_ADDRESS: u16 = 0xFF4B; // window X (minus 7)

//This is for how long each PPU mode lasts in T cycles
const OAM_CYCLES: u16 = 80; // mode 2
const DRAWING_CYCLES: u16 = 172; // mode 3
const HBLANK_CYCLES: u16 = 204; // mode 0
const SCANLINE_CYCLES: u16 = 456; // one full VBlank line (mode 1)

const VISIBLE_LINES: u8 = 144; // scanlines 0..=143 are drawn
const TOTAL_LINES: u8 = 154; // plus 10 VBlank lines (144..=153)

//The four PPU modes, in the numeric order the STAT register uses for its low two bits.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    HBlank, // 0
    VBlank, // 1
    OamScan, // 2
    Drawing, // 3
}

impl Mode {
    fn bits(self) -> u8 {
        match self {
            Mode::HBlank => 0,
            Mode::VBlank => 1,
            Mode::OamScan => 2,
            Mode::Drawing => 3,
        }
    }
}

//What the PPU wants to raise after a step, bus turns these into IF requests
#[derive(Default)]
pub struct PpuInterrupts {
    pub vblank: bool,
    pub stat: bool,
}

pub struct GPU {
    //raw VRAM bytes
    vram: [u8; VRAM_SIZE],
    //A decoded copy of every tile
    pub tile_set: [Tile; TILE_COUNT],
    //LCD registers
    lcdc: u8,
    //stat holds writable interrupt enable bits. Mode and lyc coincidence bits
    //are derived on read
    stat: u8,
    scy: u8,
    scx: u8,
    ly: u8,
    lyc: u8,
    bgp: u8,
    obp0: u8,
    obp1: u8,
    wy: u8,
    wx: u8,

    //Scanline timing state
    mode: Mode,
    mode_clock: u16,
}

impl GPU {
    pub fn new() -> Self {
        Self {
            vram: [0; VRAM_SIZE],
            tile_set: [empty_tile(); TILE_COUNT],
            //post reset hte LCD is off, game turns it on by setting LCDC bit 7
            lcdc: 0,
            stat: 0,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            bgp: 0,
            obp0: 0,
            obp1: 0,
            wy: 0,
            wx: 0,
            mode: Mode::OamScan,
            mode_clock: 0,
        }
    }

    //Read a raw VRAM byte
    pub fn read_vram(&self, index: usize) -> u8 {
        self.vram[index]
    }

    //Write a raw VRAM byte. If it landed in the tile data region, re decode the tile
    //row that byte belongs to so the cached tile_set stays in sync
    pub fn write_vram(&mut self, index: usize, value: u8) {
        self.vram[index] = value;

        //Writes at or above the tile data region are tile maps, not pixel data, so
        //theres no cached tile row to rebuild
        if index >= TILE_DATA_SIZE {
            return;
        }

        //Each tile row is encoded in two consecutive bytes, and the first byte of a row
        //always sits at an even address. Clearing the low bit of the index gives us the
        //address of that first byte
        let normalized_index = index & 0xFFFE;
        let byte1 = self.vram[normalized_index];
        let byte2 = self.vram[normalized_index + 1];


        let tile_index = index / 16;
        let row_index = (index % 16) / 2;

        //Decode all 8 pixels of the row
        for pixel_index in 0..8 {
            let mask = 1 << (7 - pixel_index);
            let low = byte1 & mask != 0;
            let high = byte2 & mask != 0;
            let value = match (high, low) {
                (false, false) => TilePixelValue::Zero,
                (false, true) => TilePixelValue::One,
                (true, false) => TilePixelValue::Two,
                (true, true) => TilePixelValue::Three,
            };
            self.tile_set[tile_index][row_index][pixel_index] = value;
        }
    }
    //Read one of the LCD registers (0xFF40..=0xFF4B, excluding DMA).
    pub fn read_register(&self, address: u16) -> u8 {
        match address {
            LCDC_ADDRESS => self.lcdc,
            STAT_ADDRESS => self.read_stat(),
            SCY_ADDRESS => self.scy,
            SCX_ADDRESS => self.scx,
            LY_ADDRESS => self.ly,
            LYC_ADDRESS => self.lyc,
            BGP_ADDRESS => self.bgp,
            OBP0_ADDRESS => self.obp0,
            OBP1_ADDRESS => self.obp1,
            WY_ADDRESS => self.wy,
            WX_ADDRESS => self.wx,
            _ => 0xFF,
        }
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        match address {
            LCDC_ADDRESS => self.set_lcdc(value),
            STAT_ADDRESS => self.write_stat(value),
            SCY_ADDRESS => self.scy = value,
            SCX_ADDRESS => self.scx = value,
            //LY is read-only,  writes are ignored
            LY_ADDRESS => {}
            LYC_ADDRESS => self.lyc = value,
            BGP_ADDRESS => self.bgp = value,
            OBP0_ADDRESS => self.obp0 = value,
            OBP1_ADDRESS => self.obp1 = value,
            WY_ADDRESS => self.wy = value,
            WX_ADDRESS => self.wx = value,
            _ => {}
        }
    }

    //Advance the PPU by `cycles` T-cycles, walking the scanline/mode schedule and
    //returning any interrupts that came due. When the LCD is off the PPU is idle.
    pub fn step(&mut self, cycles: u8) -> PpuInterrupts {
        let mut interrupts = PpuInterrupts::default();
        if !self.lcd_enabled() {
            return interrupts;
        }

        self.mode_clock += cycles as u16;
        match self.mode {
            Mode::OamScan => {
                if self.mode_clock >= OAM_CYCLES {
                    self.mode_clock -= OAM_CYCLES;
                    self.mode = Mode::Drawing;
                }
            }
            Mode::Drawing => {
                if self.mode_clock >= DRAWING_CYCLES {
                    self.mode_clock -= DRAWING_CYCLES;
                    //need to draw scanline `self.ly` into the framebuffer here.
                    self.mode = Mode::HBlank;
                    if self.stat_enabled(STAT_HBLANK) {
                        interrupts.stat = true;
                    }
                }
            }
            Mode::HBlank => {
                if self.mode_clock >= HBLANK_CYCLES {
                    self.mode_clock -= HBLANK_CYCLES;
                    self.ly += 1;
                    if self.ly == VISIBLE_LINES {
                        //Falling off the last visible line starts VBlank, the moment the
                        //frame is done and games do most of their VRAM work
                        self.mode = Mode::VBlank;
                        interrupts.vblank = true;
                        if self.stat_enabled(STAT_VBLANK) {
                            interrupts.stat = true;
                        }
                    } else {
                        self.mode = Mode::OamScan;
                        if self.stat_enabled(STAT_OAM) {
                            interrupts.stat = true;
                        }
                    }
                    if self.lyc_interrupt() {
                        interrupts.stat = true;
                    }
                }
            }
            Mode::VBlank => {
                if self.mode_clock >= SCANLINE_CYCLES {
                    self.mode_clock -= SCANLINE_CYCLES;
                    self.ly += 1;
                    if self.ly >= TOTAL_LINES {
                        //Wrap back to the top and start a fresh frame.
                        self.ly = 0;
                        self.mode = Mode::OamScan;
                        if self.stat_enabled(STAT_OAM) {
                            interrupts.stat = true;
                        }
                    }
                    if self.lyc_interrupt() {
                        interrupts.stat = true;
                    }
                }
            }
        }

        interrupts
    }

    pub fn lcd_enabled(&self) -> bool {
        self.lcdc & LCDC_ENABLE != 0
    }

    //Handle a write to LCDC, catching the LCD being switched on or off. Turning it off
    //blanks the PPU (LY = 0, back to HBlank); turning it on restarts a fresh frame.
    fn set_lcdc(&mut self, value: u8) {
        let was_on = self.lcd_enabled();
        self.lcdc = value;
        let now_on = self.lcd_enabled();
        if was_on && !now_on {
            self.ly = 0;
            self.mode_clock = 0;
            self.mode = Mode::HBlank;
        } else if !was_on && now_on {
            self.ly = 0;
            self.mode_clock = 0;
            self.mode = Mode::OamScan;
        }
    }

    //STAT read: bit 7 reads 1, bits 3-6 are the stored enables, bit 2 is the live
    //LYC-coincidence flag, bits 1-0 are the current mode.
    fn read_stat(&self) -> u8 {
        let coincidence = if self.ly == self.lyc { 0x04 } else { 0x00 };
        0x80 | (self.stat & 0x78) | coincidence | self.mode.bits()
    }

    //STAT write: only the interrupt-enable bits (3-6) are writable.
    fn write_stat(&mut self, value: u8) {
        self.stat = value & 0x78;
    }

    fn stat_enabled(&self, source: u8) -> bool {
        self.stat & source != 0
    }

    //A STAT interrupt from LYC fires only when LY has just come to equal LYC and the
    //LYC-coincidence source is enabled.
    fn lyc_interrupt(&self) -> bool {
        self.ly == self.lyc && self.stat_enabled(STAT_LYC)
    }
    
}

//LCDC bit 7 enables the LCD/PPU
const LCDC_ENABLE: u8 = 1 << 7;

//stat interrupt source enable bits
const STAT_HBLANK: u8 = 1 << 3;
const STAT_VBLANK: u8 = 1 << 4;
const STAT_OAM: u8 = 1 << 5;
const STAT_LYC: u8 = 1 << 6;

