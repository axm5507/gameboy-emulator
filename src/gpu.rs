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

pub struct GPU {
    //raw VRAM bytes
    vram: [u8; VRAM_SIZE],
    //A decoded copy of every tile
    pub tile_set: [Tile; TILE_COUNT],
}

impl GPU {
    pub fn new() -> Self {
        Self {
            vram: [0; VRAM_SIZE],
            tile_set: [empty_tile(); TILE_COUNT],
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use TilePixelValue::*;

    #[test]
    fn decodes_the_classic_tile_row() {
        //The canonical example from the Pandocs
        let mut gpu = GPU::new();
        gpu.write_vram(0, 0x3C); // low bits of tile 0, row 0
        gpu.write_vram(1, 0x7E); // high bits of tile 0, row 0

        assert_eq!(
            gpu.tile_set[0][0],
            [Zero, Two, Three, Three, Three, Three, Two, Zero]
        );
    }

    #[test]
    fn each_colour_decodes_from_the_right_bit_pair() {
        //low=1/high=0 everywhere -> all One; low=0/high=1 -> all Two, both set -> Three
        let mut gpu = GPU::new();

        gpu.write_vram(0, 0xFF); // low bits all 1
        gpu.write_vram(1, 0x00); // high bits all 0
        assert_eq!(gpu.tile_set[0][0], [One; 8]);

        gpu.write_vram(0, 0x00);
        gpu.write_vram(1, 0xFF);
        assert_eq!(gpu.tile_set[0][0], [Two; 8]);

        gpu.write_vram(0, 0xFF);
        gpu.write_vram(1, 0xFF);
        assert_eq!(gpu.tile_set[0][0], [Three; 8]);
    }

    #[test]
    fn write_targets_the_correct_tile_and_row() {
        //Byte index 0x22 is tile 2 (0x22 / 16), row 1 ((0x22 % 16) / 2). Writing there
        //should update only that tile/row and leave tile 0 blank
        let mut gpu = GPU::new();
        gpu.write_vram(0x22, 0xFF);
        gpu.write_vram(0x23, 0xFF);

        assert_eq!(gpu.tile_set[2][1], [Three; 8], "the addressed tile row is decoded");
        assert_eq!(gpu.tile_set[0][0], [Zero; 8], "an unrelated tile stays blank");
    }

    #[test]
    fn read_vram_returns_the_raw_byte() {
        let mut gpu = GPU::new();
        gpu.write_vram(0x10, 0xAB);
        assert_eq!(gpu.read_vram(0x10), 0xAB, "read_vram returns exactly what was stored");
    }

    #[test]
    fn writes_to_the_tile_map_region_are_stored_but_do_not_decode() {
        //0x1800 is the first byte of the tile map region
        let mut gpu = GPU::new();
        gpu.write_vram(0x1800, 0xCD);

        assert_eq!(gpu.read_vram(0x1800), 0xCD, "map bytes are still stored in VRAM");
        assert_eq!(gpu.tile_set[0][0], [Zero; 8], "map writes leave the tile set alone");
    }
}

