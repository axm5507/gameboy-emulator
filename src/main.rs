mod cpu;
mod gpu;

use cpu::Registers;


fn main() {
    let regs = Registers::new();
    println!("Game Boy emulator");
}
