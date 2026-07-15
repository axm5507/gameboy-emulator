//A cartridge is the game ROM(read only memory) plus some RAM. Anything that is
//larger than 32kb cannot sit in the CPU's address space all at once, so in real
//cartridges there is an MBC(memory bank controller) that swaps 16kb ROM banks and 
//8kb RAM banks in and out in response to writes to the ROM address range
//I'm implementing MBC1, the most common controller. Its logic covers plain 32kb 
//ROM only carts, which never write to the banking registers. 

const ROM_BANK_SIZE: usize = 0x4000; //16kb
const RAM_BANK_SIZE: usize = 0x2000; //8kb

//Header offset that declares how much cartridge RAM there is
const RAM_SIZE_HEADER: usize = 0x0149;
const CART_TYPE_HEADER: usize = 0x0147;

pub struct Cartridge {
    rom: Vec<u8>,
    ram: Vec<u8>,
    mbc: Mbc,
}

impl Cartridge {
    pub fn new(rom: Vec<u8>) -> Self {
        let cart_type = rom.get(CART_TYPE_HEADER).copied().unwrap_or(0);
        let ram = vec![0; ram_size(rom.get(RAM_SIZE_HEADER).copied().unwrap_or(0))];
        let mbc = Mbc::for_cart_type(cart_type);
        Self { rom, ram, mbc }
    }

    pub fn read(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x3FFF => self.read_rom(self.mbc.rom_bank_0(), address as usize),
            0x4000..=0x7FFF => self.read_rom(self.mbc.rom_bank_n(), address as usize - 0x4000),
            0xA000..=0xBFFF => self.mbc.read_ram(&self.ram, address),
            _ => 0xFF,
        }
    }

    //Writes into ROM space poke the MBC's control register
    pub fn write(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x7FFF => self.mbc.write_control(address, value),
            0xA000..=0xBFFF => self.mbc.write_ram(&mut self.ram, address, value),
            _ => {}
        }
    }

    //Read a byte from a given ROM bank, wrapping the bank number to the banks that exist
    fn read_rom(&self, bank: usize, offset_in_bank: usize) -> u8 {
        let banks = (self.rom.len() / ROM_BANK_SIZE).max(1);
        self.rom
            .get((bank % banks) * ROM_BANK_SIZE + offset_in_bank)
            .copied()
            .unwrap_or(0xFF)
    }
}

//Cartridge RAM size
fn ram_size(header: u8) -> usize {
    match header {
        0x02 => 0x2000,  // 8 KB (1 bank)
        0x03 => 0x8000,  // 32 KB (4 banks)
        0x04 => 0x20000, // 128 KB (16 banks)
        0x05 => 0x10000, // 64 KB (8 banks)
        _ => 0,
    }
}

//The banking state for whichever controller a cartridge uses
enum Mbc {
    None,
    Mbc1 {
        ram_enabled: bool,
        bank1: u8, // 5-bit low ROM bank
        bank2: u8, // 2-bit high ROM bank / RAM bank
        mode: bool,
    },
    Mbc3 {
        ram_enabled: bool,
        rom_bank: u8,  // 7 bits
        ram_bank: u8,  // 0-3 selects RAM; 0x08-0x0C selects an RTC register
        rtc: Rtc,
    },
    Mbc5 {
        ram_enabled: bool,
        rom_bank: u16, // 9 bits
        ram_bank: u8,  // 4 bits (up to 16 banks)
    },
}

impl Mbc {
    fn for_cart_type(cart_type: u8) -> Mbc {
        match cart_type {
            0x00 | 0x08 | 0x09 => Mbc::None,
            0x01..=0x03 => Mbc::Mbc1 {
                ram_enabled: false,
                bank1: 1,
                bank2: 0,
                mode: false,
            },
            0x0F..=0x13 => Mbc::Mbc3 {
                ram_enabled: false,
                rom_bank: 1,
                ram_bank: 0,
                rtc: Rtc::default(),
            },
            0x19..=0x1E => Mbc::Mbc5 {
                ram_enabled: false,
                rom_bank: 1,
                ram_bank: 0,
            },
            //Unknown controller, fall back to MBC1 as a best effort.
            _ => Mbc::Mbc1 {
                ram_enabled: false,
                bank1: 1,
                bank2: 0,
                mode: false,
            },
        }
    }

    //Which ROM bank appears at 0x0000-0x3FFF
    fn rom_bank_0(&self) -> usize {
        match self {
            Mbc::Mbc1 {
                bank2, mode: true, ..
            } => (*bank2 as usize) << 5,
            _ => 0,
        }
    }

    //Which ROM bank appears at 0x4000-0x7FFF
    fn rom_bank_n(&self) -> usize {
        match self {
            Mbc::None => 1,
            //MBC1 combines the two registers
            Mbc::Mbc1 { bank1, bank2, .. } => {
                let low = if *bank1 == 0 { 1 } else { *bank1 } as usize;
                ((*bank2 as usize) << 5) | low
            }
            //MBC3 only translates a plain 0 to 1
            Mbc::Mbc3 { rom_bank, .. } => {
                if *rom_bank == 0 {
                    1
                } else {
                    *rom_bank as usize
                }
            }
            //MBC5 has no bank-0 quirk
            Mbc::Mbc5 { rom_bank, .. } => *rom_bank as usize,
        }
    }

    fn write_control(&mut self, address: u16, value: u8) {
        match self {
            Mbc::None => {}
            Mbc::Mbc1 {
                ram_enabled,
                bank1,
                bank2,
                mode,
            } => match address {
                0x0000..=0x1FFF => *ram_enabled = value & 0x0F == 0x0A,
                0x2000..=0x3FFF => *bank1 = value & 0x1F,
                0x4000..=0x5FFF => *bank2 = value & 0x03,
                0x6000..=0x7FFF => *mode = value & 0x01 != 0,
                _ => {}
            },
            Mbc::Mbc3 {
                ram_enabled,
                rom_bank,
                ram_bank,
                rtc,
            } => match address {
                0x0000..=0x1FFF => *ram_enabled = value & 0x0F == 0x0A,
                0x2000..=0x3FFF => *rom_bank = value & 0x7F,
                0x4000..=0x5FFF => *ram_bank = value, // 0x00-0x03 RAM, 0x08-0x0C RTC register
                0x6000..=0x7FFF => rtc.latch(value),
                _ => {}
            },
            Mbc::Mbc5 {
                ram_enabled,
                rom_bank,
                ram_bank,
            } => match address {
                0x0000..=0x1FFF => *ram_enabled = value & 0x0F == 0x0A,
                //The ROM bank is split, 0x2000-0x2FFF is the low 8 bits, 0x3000-0x3FFF bit 8
                0x2000..=0x2FFF => *rom_bank = (*rom_bank & 0x100) | value as u16,
                0x3000..=0x3FFF => *rom_bank = (*rom_bank & 0x0FF) | (((value & 1) as u16) << 8),
                0x4000..=0x5FFF => *ram_bank = value & 0x0F,
                _ => {}
            },
        }
    }

    fn read_ram(&self, ram: &[u8], address: u16) -> u8 {
        match self {
            //ROM-only carts with RAM expose it directly
            Mbc::None => ram.get(address as usize - 0xA000).copied().unwrap_or(0xFF),
            Mbc::Mbc1 {
                ram_enabled,
                bank2,
                mode,
                ..
            } => {
                if !*ram_enabled {
                    return 0xFF;
                }
                let bank = if *mode { *bank2 as usize } else { 0 };
                read_ram_at(ram, bank, address)
            }
            Mbc::Mbc3 {
                ram_enabled,
                ram_bank,
                rtc,
                ..
            } => {
                if !*ram_enabled {
                    return 0xFF;
                }
                match *ram_bank {
                    0x00..=0x03 => read_ram_at(ram, *ram_bank as usize, address),
                    0x08..=0x0C => rtc.read(*ram_bank),
                    _ => 0xFF,
                }
            }
            Mbc::Mbc5 {
                ram_enabled,
                ram_bank,
                ..
            } => {
                if !*ram_enabled {
                    return 0xFF;
                }
                read_ram_at(ram, *ram_bank as usize, address)
            }
        }
    }

    fn write_ram(&mut self, ram: &mut [u8], address: u16, value: u8) {
        match self {
            Mbc::None => {
                if let Some(cell) = ram.get_mut(address as usize - 0xA000) {
                    *cell = value;
                }
            }
            Mbc::Mbc1 {
                ram_enabled,
                bank2,
                mode,
                ..
            } => {
                if *ram_enabled {
                    let bank = if *mode { *bank2 as usize } else { 0 };
                    write_ram_at(ram, bank, address, value);
                }
            }
            Mbc::Mbc3 {
                ram_enabled,
                ram_bank,
                rtc,
                ..
            } => {
                if *ram_enabled {
                    match *ram_bank {
                        0x00..=0x03 => write_ram_at(ram, *ram_bank as usize, address, value),
                        0x08..=0x0C => rtc.write(*ram_bank, value),
                        _ => {}
                    }
                }
            }
            Mbc::Mbc5 {
                ram_enabled,
                ram_bank,
                ..
            } => {
                if *ram_enabled {
                    write_ram_at(ram, *ram_bank as usize, address, value);
                }
            }
        }
    }
}

//Translate an (address, bank) into an index into the RAM vector, wrapping the bank to
//the banks that actually exist
fn ram_offset(ram_len: usize, bank: usize, address: u16) -> Option<usize> {
    if ram_len == 0 {
        return None;
    }
    let banks = (ram_len / RAM_BANK_SIZE).max(1);
    Some((bank % banks) * RAM_BANK_SIZE + (address as usize - 0xA000))
}

fn read_ram_at(ram: &[u8], bank: usize, address: u16) -> u8 {
    match ram_offset(ram.len(), bank, address) {
        Some(offset) => ram.get(offset).copied().unwrap_or(0xFF),
        None => 0xFF,
    }
}

fn write_ram_at(ram: &mut [u8], bank: usize, address: u16, value: u8) {
    if let Some(offset) = ram_offset(ram.len(), bank, address) {
        if let Some(cell) = ram.get_mut(offset) {
            *cell = value;
        }
    }
}

//MBC3's real-time clock. The five registers are seconds/minutes/hours/day-low/day-high.
//A game reads them via the 0xA000-0xBFFF window (after selecting an RTC register with a
//write to 0x4000-0x5FFF) but only after writing 0x00 then 0x01 to 0x6000-0x7FFF copies
//the live registers into the readable, latched copy.
#[derive(Clone, Copy, Default)]
struct Rtc {
    registers: [u8; 5],
    latched: [u8; 5],
    last_latch_write: u8,
}

impl Rtc {
    fn latch(&mut self, value: u8) {
        if self.last_latch_write == 0x00 && value == 0x01 {
            self.latched = self.registers;
        }
        self.last_latch_write = value;
    }

    fn read(&self, select: u8) -> u8 {
        self.latched[(select - 0x08) as usize]
    }

    fn write(&mut self, select: u8, value: u8) {
        self.registers[(select - 0x08) as usize] = value;
    }
}
