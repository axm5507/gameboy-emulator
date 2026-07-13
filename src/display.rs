//This turns the PPUs framebuffer into the 0RGB pixel buffer the window wants.
use crate::gpu::TilePixelValue;

const SHADES: [u32; 4] = [0x00FFFFFF, 0x00AAAAAA, 0x00555555, 0x00000000];

pub fn shade_to_rgb(shade: TilePixelValue) -> u32 {
    let index = match shade {
        TilePixelValue::Zero => 0,
        TilePixelValue::One => 1,
        TilePixelValue::Two => 2,
        TilePixelValue::Three => 3,
    };
    SHADES[index]
}

//Convert an entire framebuffer into a window pixel buffer, laid out row major
pub fn to_rgb(framebuffer: &[TilePixelValue], out: &mut [u32]) {
    for (pixel, slot) in framebuffer.iter().zip(out.iter_mut()) {
        *slot = shade_to_rgb(*pixel);
    }
}
