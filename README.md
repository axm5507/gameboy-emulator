# Gameboy Emulator

I've recently been getting back into playing video games, so I decided to create a gameboy emulator in rust, drawing from [this article](https://rylev.github.io/DMG-01/public/book/introduction.html) and the [Pan Docs](https://gbdev.io/pandocs/). Before this, I knew absolutely nothing about how a gameboy works, so I thoroughly commented through my code to  understand everything I learned. This was a great project because I got to work on CPU architecture, memory management, graphics, and hardware emulation. It ended up being the coolest thing I have ever built so far, and getting to run Pokemon on this made the countless hours of writing and testing worth it.  

Some limitations right now are the lack of audio and no color. Furthermore, the real time clock ticks are consistent within a session but don't persist across runs, but this doesn't really matter because it's just an emulator. Maybe in the future I'll make a gameboy color emulator!  

##Project structure

```
src/
  ├── main.rs             window, run loop, input, save files
  ├── display.rs          framebuffer, RGB pixels
  ├── gpu.rs              VRAM, tiles, LCD registers, scanline timing, rendering
  ├── cartridge.rs        ROM/RAM + MBC1/MBC3/MBC5 banking, RTC, battery saves
  ├── timer.rs            DIV/TIMA/TMA/TAC timer
  ├── joypad.rs           P1 input register
  ├── interrupts.rs       interrupt kinds, IE/IF registers and their vectors
  ├── cpu/
  |   ├── cpu.rs          fetch-decode-execute, interrupt dispatch, cycle counting
  |   ├── instruction.rs  instruction set and opcode decoder
  |   ├── registers.rs    registers and the flags register
  |   └── memory_bus.rs   address routing to all the hardware above
```

## How it works:

The Game Boy is a small computer made up of several chips connected to a 16bit address bus, and my emulator mirrors this structure. Basically everything in the Game Boy is driven by the CPU. It executes one instruction at a time, and after each instruction the rest of the hardware advances by the number of clock cycles the instruction consumed.   

The **CPU** ([src/cpu/](src/cpu)) is responsible for executing a game's program. It runs a fetch-decode-execute loop where it:  
1. Fetches the next opcode from memory using the current program counter
2. Decodes the opcode into an Instruction ([`instruction.rs`](src/cpu/instruction.rs)) and determines what operation to perform
3. Executes the instruction([`cpu.rs`](src/cpu/cpu.rs)), updating registers, flags([`registers.rs`](src/cpu/registers.rs)), memory, and the program counter
4. Returns the number of T Cycles(4.19 MHz clock ticks) the instruction consumed  

Before it executes each instruction, the CPU also checks whether any hardware component has requested an interrupt. If an interrupt is enabled, the CPU pauses the current program, jumps to the necessary interrupt handler, services the event, and then picks back where it left off.  

The CPU never actually directly interacts with the other pieces of hardware. All memory access goes through the **memory bus** ([`memory_bus.rs`](src/cpu/memory_bus.rs)), which serves as a communication network between all of the Game Boy components. The memory bus looks at the address being accessed and forwards the request to the correct device.  

`0x0000–0x7FFF` and `0xA000–0xBFFF` goes to the cartridge, which contains ROM banks, memory bank controller registers, and save RAM.  
`0x8000–0x9FFF` and OAM goes to the PPU, which stores graphics data and sprite information.  
the I/O registers go to the timer, joypad, and PPU registers.  
everything else goes to plain RAM.  

The **PPU**([`gpu.rs`](src/gpu.rs)) is the chip responsible for generating the display. It does this by rendering an image one horizontal scanline at a time, with 144 visible lines total followed by 10 lines of VBlank, during which the completed frame is shown while the next frame begings preparing. For each visible line, the PPU first scans sprite information stored in the OAM, then fetches tile and sprite graphics from VRAm, draws the completed scanline into the framebuffer, and finally enters HBlank before moving to the next line. When all visible lines have been rendered, the PPU enters VBlank, raises a VBlank interrupt to the CPU, and begins the next frame. Tile graphics stored in VRAM are decoded into a cache whenever VRAM is modified, while allows rendering to perform simple table lookups instead of repeatedly decoding tile data.  

The **cartridge**([`cartridge.rs`](src/cartridge.rs)) is responsible for storing the game you are running. A smaller game will fit entirely in the ROM but many cartridges include a memory bank controller that allows games that are larger than the address space to be played. Writes to certain ROM addresses are interpreted as commands to the MBC rather than normal memory writes, which allows the game to switch between 8kb ROM and 16kb RAM banks. The battery backed RAM in the cartridge is exposed so save files can be preserved between emulator sessions.  

The frontend([`main.rs`](src/main.rs)) loads the cartridge, initializes the CPU to the same state it would have after the Game Boy's boot ROM finishes executing, and then loops. It runs one frame's worth of cycles, reads the keyboard into the joypad, and converts the framebuffer to greyscale pixels([`display.rs`](src/display.rs)). Finally, it displays the complete image and repeats the process.  

##How to run:

**Prerequisites:** [Rust](https://rustup.rs/) 2024  

Setup/Initial Build:  
```sh
git clone https://github.com/axm5507/gameboy-emulator
cd gameboy-emulator
cargo build --release
```  

To run a game:  
First, find a Game Boy ROM file(.gb). [This is a cool option:](https://vimm.net/vault/GB) Then, download it and run this:  
```sh
cargo run --release -- path/to/game.gb
```



