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

pub struct Cartridge {
    rom: Vec<u8>,
    ram: Vec<u8>,
    num_rom_banks: usize,
    num_ram_banks: usize,

    ram_enabled: bool,
    //2 bank registers, 5bit bank1 with low ROM bank bits and 2bit bank2 with either
    //the high ROM bank bits or RAM bank, depending on the mode
    bank1: u8,
    bank2: u8,
    //false = simple ROM banking(default), true = RAM/large ROM banking mode
    mode: bool,
}

impl Cartridge {
    pub fn new(rom: Vec<u8>) -> Self {
        let num_rom_banks = (rom.len() / ROM_BANK_SIZE).max(2);
        let ram_size = match rom.get(RAM_SIZE_HEADER).copied().unwrap_or(0) {
            0x02 => 0x2000, // 8kb(1 bank)
            0x03 => 0x8000, // 32kb(4 banks)
            _ => 0,
        };
        let num_ram_banks = (ram_size / RAM_BANK_SIZE).max(1);
        Self {
            rom,
            ram: vec![0; ram_size],
            num_rom_banks,
            num_ram_banks,
            ram_enabled: false,
            bank1: 1, // never 0
            bank2: 0,
            mode: false,
        }
    }

    pub fn read(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x3FFF => self.read_rom(self.rom_bank_lower(), address as usize),
            0x4000..=0x7FFF => self.read_rom(self.rom_bank_upper(), address as usize - 0x4000),
            0xA000..=0xBFFF => {
                if self.ram_enabled && !self.ram.is_empty() {
                    self.ram[self.ram_offset(address)]
                } else {
                    0xFF //disabled or absent RAM reads back as open bus
                }
            }
            _ => 0xFF,
        }
    }

    //Writes into ROM space don't change ROM, they poke the MBC's control registers
    pub fn write(&mut self, address: u16, value: u8) {
        match address {
            //RAM enable, only the value 0x0A turns it on
            0x0000..=0x1FFF => self.ram_enabled = value & 0x0F == 0x0A,
            //Low 5 bits of the ROM bank number
            0x2000..=0x3FFF => self.bank1 = value & 0x1F,
            //2 bit RAM bank/high ROM bank bits
            0x4000..=0x5FFF => self.bank2 = value & 0x03,
            //Banking mode select
            0x6000..=0x7FFF => self.mode = value & 0x01 != 0,
            0xA000..=0xBFFF => {
                if self.ram_enabled && !self.ram.is_empty() {
                    let offset = self.ram_offset(address);
                    self.ram[offset] = value;
                }
            }
            _ => {}
        }
    }

    //The switchable ROM bank shown at 0x4000-0x7FFF: high bits from bank2, low from bank1
    //bank1 is forced to at least 1 
    fn rom_bank_upper(&self) -> usize {
        let low = if self.bank1 == 0 { 1 } else { self.bank1 } as usize;
        ((self.bank2 as usize) << 5) | low
    }

    //The bank shown at 0x0000-0x3FFF: normally bank 0, but in mode 1 the high bits apply
    //here too
    fn rom_bank_lower(&self) -> usize {
        if self.mode {
            (self.bank2 as usize) << 5
        } else {
            0
        }
    }

    fn read_rom(&self, bank: usize, offset_in_bank: usize) -> u8 {
        let bank = bank % self.num_rom_banks;
        self.rom
            .get(bank * ROM_BANK_SIZE + offset_in_bank)
            .copied()
            .unwrap_or(0xFF)
    }

    fn ram_offset(&self, address: u16) -> usize {
        //Only mode 1 uses a switchable RAM bank, mode 0 is always bank 0
        let bank = if self.mode { self.bank2 as usize } else { 0 };
        (bank % self.num_ram_banks) * RAM_BANK_SIZE + (address as usize - 0xA000)
    }
}
