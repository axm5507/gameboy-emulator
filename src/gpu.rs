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

//Visible screen, background is a larger 256x256 area scrolled behind this window
pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 144;

//Object attribute memory. 40 sprites * 4 bytes
pub const OAM_BEGIN: u16 = 0xFE00;
pub const OAM_END: u16 = 0xFE9F;
const OAM_SIZE: usize = 0xA0; // 160 bytes
const SPRITE_COUNT: usize = 40;
const MAX_SPRITES_PER_LINE: usize = 10;

//Two background tile maps
const TILE_MAP_0: usize = 0x1800;
const TILE_MAP_1: usize = 0x1C00;
const TILES_PER_ROW: usize = 32;

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

//Single sprite(object) pulled out of OAM for rendering with screen coords already adjusted
#[derive(Clone, Copy, Default)]
struct Sprite {
    y: i16,
    x: i16,
    tile: u8,
    attributes: u8,
    oam_index: u8,
}

pub struct GPU {
    //raw VRAM bytes
    vram: [u8; VRAM_SIZE],
    //A decoded copy of every tile
    pub tile_set: [Tile; TILE_COUNT],
    //LCD registers
    lcdc: u8,
    //object attribute memory
    oam: [u8; OAM_SIZE],
    //rendered picture, one palette resolved shade per pixel, row major
    pub framebuffer: [TilePixelValue; SCREEN_WIDTH * SCREEN_HEIGHT],
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
    //Window has its own vertical counter advancing on scanlines where window
    //was actually drawn
    window_line: u8,
}

impl GPU {
    pub fn new() -> Self {
        Self {
            vram: [0; VRAM_SIZE],
            tile_set: [empty_tile(); TILE_COUNT],
            oam: [0; OAM_SIZE],
            framebuffer: [TilePixelValue::Zero; SCREEN_WIDTH * SCREEN_HEIGHT],
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
            window_line: 0,
        }
    }

    //Read a raw VRAM byte
    pub fn read_vram(&self, index: usize) -> u8 {
        self.vram[index]
    }

    //OAM access, index relative to 0xFE00.
    pub fn read_oam(&self, index: usize) -> u8 {
        self.oam[index]
    }

    pub fn write_oam(&mut self, index: usize, value: u8) {
        self.oam[index] = value;
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
 
    //Paint the current scanline (self.ly): first the background and window layers, then
    //sprites on top. render_bg_and_window hands back the raw background colour indices so
    //sprite priority can tell "over background colour 0" (always visible) from "behind
    //background colours 1-3".
    fn render_scanline(&mut self) {
        let bg_colors = self.render_bg_and_window();
        if self.lcdc & LCDC_OBJ_ENABLE != 0 {
            self.render_sprites(&bg_colors);
        }
    }

    //Render the background and, where it's active on this line, the window - which is the
    //same tile machinery but a second map layer that ISN'T scrolled, positioned by
    //WX/WY. Returns the raw (pre-palette) colour index of each pixel, for sprite priority.
    fn render_bg_and_window(&mut self) -> [TilePixelValue; SCREEN_WIDTH] {
        let row_start = self.ly as usize * SCREEN_WIDTH;
        let mut colors = [TilePixelValue::Zero; SCREEN_WIDTH];

        //With BG/Window disabled (LCDC bit 0), the line is blank white on the DMG.
        if self.lcdc & LCDC_BG_ENABLE == 0 {
            for pixel in &mut self.framebuffer[row_start..row_start + SCREEN_WIDTH] {
                *pixel = TilePixelValue::Zero;
            }
            return colors;
        }

        //LCDC bit 4 picks the tile-data addressing (shared by BG and window): set =
        //unsigned from tile 0 (0x8000); clear = signed, where tile number 0 means tile
        //256 (0x9000) and the number is an i8 offset from there.
        let signed_tiles = self.lcdc & LCDC_TILE_DATA == 0;

        let bg_map = self.map_offset(LCDC_BG_MAP);
        //Vertical position on the (scrolled, wrapping) background surface.
        let bg_y = self.ly.wrapping_add(self.scy) as usize;
        let bg_tile_row = bg_y / 8;
        let bg_pixel_y = bg_y % 8;

        //The window is drawn on this line if enabled and LY has reached WY.
        let window_active = self.lcdc & LCDC_WINDOW_ENABLE != 0 && self.ly >= self.wy;
        let window_map = self.map_offset(LCDC_WINDOW_MAP);
        let window_start = self.wx as i16 - 7; // screen X of the window's left edge
        let window_pixel_y = self.window_line as usize;
        let mut window_drawn = false;

        for x in 0..SCREEN_WIDTH {
            let color = if window_active && (x as i16) >= window_start {
                //Window pixel. Its coordinates are its own, not the scrolled ones.
                window_drawn = true;
                let win_x = (x as i16 - window_start) as usize;
                let tile_number =
                    self.vram[window_map + (window_pixel_y / 8) * TILES_PER_ROW + win_x / 8];
                let tile = resolve_tile(signed_tiles, tile_number);
                self.tile_set[tile][window_pixel_y % 8][win_x % 8]
            } else {
                //Background pixel.
                let bg_x = (x as u8).wrapping_add(self.scx) as usize;
                let tile_number = self.vram[bg_map + bg_tile_row * TILES_PER_ROW + bg_x / 8];
                let tile = resolve_tile(signed_tiles, tile_number);
                self.tile_set[tile][bg_pixel_y][bg_x % 8]
            };

            colors[x] = color;
            self.framebuffer[row_start + x] = Self::apply_palette(self.bgp, color);
        }

        //The window's own line counter only advances on lines it was actually shown.
        if window_drawn {
            self.window_line = self.window_line.wrapping_add(1);
        }

        colors
    }
    //Draw the sprites that fall on the current scanline, on top of the background.
    fn render_sprites(&mut self, bg_colors: &[TilePixelValue; SCREEN_WIDTH]) {
        let height: i16 = if self.lcdc & LCDC_OBJ_SIZE != 0 { 16 } else { 8 };
        let ly = self.ly as i16;

        //OAM scan: gather the sprites covering this line, in OAM order, up to the
        //hardware limit of 10.
        let mut visible = [Sprite::default(); MAX_SPRITES_PER_LINE];
        let mut count = 0;
        for index in 0..SPRITE_COUNT {
            let base = index * 4;
            let y = self.oam[base] as i16 - 16;
            if ly >= y && ly < y + height {
                visible[count] = Sprite {
                    y,
                    x: self.oam[base + 1] as i16 - 8,
                    tile: self.oam[base + 2],
                    attributes: self.oam[base + 3],
                    oam_index: index as u8,
                };
                count += 1;
                if count == MAX_SPRITES_PER_LINE {
                    break;
                }
            }
        }

        //DMG priority: the sprite with the smaller X (ties broken by lower OAM index) is
        //drawn on top. Sort into priority order, then draw lowest priority first so the
        //highest ends up on top.
        let sprites = &mut visible[..count];
        sprites.sort_by_key(|s| (s.x, s.oam_index));
        for sprite in sprites.iter().rev() {
            self.draw_sprite(sprite, height, bg_colors);
        }
    }

    fn draw_sprite(
        &mut self,
        sprite: &Sprite,
        height: i16,
        bg_colors: &[TilePixelValue; SCREEN_WIDTH],
    ) {
        //Which row of the sprite this scanline hits, flipped vertically if requested.
        let mut row = self.ly as i16 - sprite.y;
        if sprite.attributes & OBJ_Y_FLIP != 0 {
            row = height - 1 - row;
        }
        //For 8x16 sprites the tile number's low bit is ignored: the top half is the even
        //tile, the bottom half the odd one.
        let (tile, tile_row) = if height == 16 {
            if row < 8 {
                (sprite.tile & 0xFE, row as usize)
            } else {
                (sprite.tile | 0x01, (row - 8) as usize)
            }
        } else {
            (sprite.tile, row as usize)
        };

        let palette = if sprite.attributes & OBJ_PALETTE != 0 {
            self.obp1
        } else {
            self.obp0
        };
        let behind_bg = sprite.attributes & OBJ_PRIORITY != 0;

        for col in 0..8i16 {
            //Sprites always use unsigned tile addressing (0x8000 base).
            let pixel_col = if sprite.attributes & OBJ_X_FLIP != 0 {
                7 - col
            } else {
                col
            } as usize;
            let color = self.tile_set[tile as usize][tile_row][pixel_col];

            //Colour 0 is transparent for sprites.
            if color == TilePixelValue::Zero {
                continue;
            }
            let screen_x = sprite.x + col;
            if screen_x < 0 || screen_x >= SCREEN_WIDTH as i16 {
                continue;
            }
            let sx = screen_x as usize;
            //If this sprite is flagged "behind background", it only shows over BG colour 0.
            if behind_bg && bg_colors[sx] != TilePixelValue::Zero {
                continue;
            }
            self.framebuffer[self.ly as usize * SCREEN_WIDTH + sx] =
                Self::apply_palette(palette, color);
        }
    }

    //VRAM offset of one of the two tile maps, chosen by the given LCDC map-select bit.
    fn map_offset(&self, select_bit: u8) -> usize {
        if self.lcdc & select_bit != 0 {
            TILE_MAP_1
        } else {
            TILE_MAP_0
        }
    }

    

    //Map a tile's raw 2-bit color through a palette register to an actual shade. The
    //palette packs four shades into its byte: bits 1-0 are the shade for color 0, bits
    //3-2 for color 1, and so on.
    fn apply_palette(palette: u8, color: TilePixelValue) -> TilePixelValue {
        let color_index = match color {
            TilePixelValue::Zero => 0,
            TilePixelValue::One => 1,
            TilePixelValue::Two => 2,
            TilePixelValue::Three => 3,
        };
        let shade = (palette >> (color_index * 2)) & 0b11;
        match shade {
            0 => TilePixelValue::Zero,
            1 => TilePixelValue::One,
            2 => TilePixelValue::Two,
            _ => TilePixelValue::Three,
        }
    }
    
}
fn resolve_tile(signed: bool, tile_number: u8) -> usize {
    if signed {
        (256i16 + tile_number as i8 as i16) as usize
    } else {
        tile_number as usize
    }
}

//LCDC bits.
const LCDC_ENABLE: u8 = 1 << 7; // LCD/PPU on
const LCDC_WINDOW_MAP: u8 = 1 << 6; // window tile map select
const LCDC_WINDOW_ENABLE: u8 = 1 << 5; // window on
const LCDC_TILE_DATA: u8 = 1 << 4; // BG/window tile-data area & addressing
const LCDC_BG_MAP: u8 = 1 << 3; // BG tile map select
const LCDC_OBJ_SIZE: u8 = 1 << 2; // sprites are 8x8 (0) or 8x16 (1)
const LCDC_OBJ_ENABLE: u8 = 1 << 1; // sprites on
const LCDC_BG_ENABLE: u8 = 1 << 0; // background (and window) on

//Sprite (OAM) attribute bits.
const OBJ_PRIORITY: u8 = 1 << 7; // 1 = behind background colours 1-3
const OBJ_Y_FLIP: u8 = 1 << 6;
const OBJ_X_FLIP: u8 = 1 << 5;
const OBJ_PALETTE: u8 = 1 << 4; // 0 = OBP0, 1 = OBP1

//STAT interrupt-source enable bits.
const STAT_HBLANK: u8 = 1 << 3;
const STAT_VBLANK: u8 = 1 << 4;
const STAT_OAM: u8 = 1 << 5;
const STAT_LYC: u8 = 1 << 6;
