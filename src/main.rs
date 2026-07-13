mod cpu;
mod display;
mod gpu;
mod interrupts;
mod joypad;
mod timer;

use cpu::CPU;
use gpu::{SCREEN_HEIGHT, SCREEN_WIDTH};
use joypad::Button;
use minifb::{Key, Scale, Window, WindowOptions};

//One frame is 154 scanlines * 456 T cycles
const CYCLES_PER_FRAME: u32 = 70224;

//Keyboard -> Game Boy button layout
const KEY_MAP: [(Key, Button); 8] = [
    (Key::Right, Button::Right),
    (Key::Left, Button::Left),
    (Key::Up, Button::Up),
    (Key::Down, Button::Down),
    (Key::Z, Button::A),
    (Key::X, Button::B),
    (Key::Enter, Button::Start),
    (Key::RightShift, Button::Select),
];

fn main() {
    let mut cpu = CPU::new();

    //A ROM path on the command line is loaded into the cartridge region
    match std::env::args().nth(1) {
        Some(path) => {
            let rom = std::fs::read(&path).unwrap_or_else(|e| panic!("could not read '{path}': {e}"));
            cpu.bus.load_rom(&rom);
        }
        None => load_demo(&mut cpu),
    }

    let mut window = Window::new(
        "Game Boy",
        SCREEN_WIDTH,
        SCREEN_HEIGHT,
        WindowOptions {
            scale: Scale::X4,
            ..WindowOptions::default()
        },
    )
    .expect("failed to open a window");
    window.set_target_fps(60);

    let mut buffer = vec![0u32; SCREEN_WIDTH * SCREEN_HEIGHT];
    while window.is_open() && !window.is_key_down(Key::Escape) {
        run_frame(&mut cpu);
        update_joypad(&mut cpu, &window);
        display::to_rgb(&cpu.bus.gpu.framebuffer, &mut buffer);
        window
            .update_with_buffer(&buffer, SCREEN_WIDTH, SCREEN_HEIGHT)
            .expect("failed to present frame");
    }
}

//Run the CPU (which keeps the timer and PPU in lockstep) until about one frame of
//T cycles has elapsed
fn run_frame(cpu: &mut CPU) {
    let mut cycles = 0u32;
    while cycles < CYCLES_PER_FRAME {
        cycles += cpu.step() as u32;
    }
}

//Push the current keyboard state into the joypad each frame
fn update_joypad(cpu: &mut CPU, window: &Window) {
    for (key, button) in KEY_MAP {
        if window.is_key_down(key) {
            cpu.bus.press_button(button);
        } else {
            cpu.bus.release_button(button);
        }
    }
}

//temp stuff for now
fn load_demo(cpu: &mut CPU) {
    for color in 0u8..4 {
        let low = if color & 0b01 != 0 { 0xFF } else { 0x00 };
        let high = if color & 0b10 != 0 { 0xFF } else { 0x00 };
        for row in 0..8u16 {
            let base = 0x8000 + (color as u16) * 16 + row * 2;
            cpu.bus.write_byte(base, low);
            cpu.bus.write_byte(base + 1, high);
        }
    }

    //Fill the background map (0x9800) so column c shows tile (c % 4): repeating bands
    for row in 0..32u16 {
        for col in 0..32u16 {
            cpu.bus.write_byte(0x9800 + row * 32 + col, (col % 4) as u8);
        }
    }

    cpu.bus.write_byte(0xFF47, 0xE4); // BGP: identity palette
    cpu.bus.write_byte(0xFF40, 0x91); // LCDC: LCD on, BG on, tile data at 0x8000

    //Park the CPU in a tight `JR -2` self-loop at 0x0000 so it doesn't wander off into
    //VRAM/data and hit an undefined opcode
    cpu.bus.write_byte(0x0000, 0x18);
    cpu.bus.write_byte(0x0001, 0xFE);
}
