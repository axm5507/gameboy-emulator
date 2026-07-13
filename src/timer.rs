//A timer is necessary because for a bunch of games there are some things
//they need to do regularly like spawn enemies, blink a cursor every half second,
//or so on and having a timer is better than the CPU constantly checking the clock
//It is 4 memory mapped registers driven by the system clock. DIV is a free running
//counter that always ticks at 16384 Hz, and reading it gives the upper 8 bits of 
//an internal 16 bit counter. Writing any value resets that counter to 0.
//TIMA is the timer counter, when enabled it ticks at the rate the TAC selects and when
//it overflows past 0xFF it reloads from TMA and requests a Timer interrupt. TMA is the 
//value TIMA reloads to on overflow and TAC is a tick rate and timer control

pub const DIV_ADDRESS: u16 = 0xFF04;
pub const TIMA_ADDRESS: u16 = 0xFF05;
pub const TMA_ADDRESS: u16 = 0xFF06;
pub const TAC_ADDRESS: u16 = 0xFF07;

//Bit 2 of TAC turns the timer on
const TAC_ENABLE: u8 = 1 << 2;

pub struct Timer {
    //16 bit internal counter, DIV is its high byte, ticks once per T cycle
    divider: u16,
    tima: u8,
    tma: u8,
    tac: u8,
    tima_cycles: u16,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            divider: 0,
            tima: 0,
            tma: 0,
            tac: 0,
            tima_cycles: 0,
        }
    }

    pub fn read(&self, address: u16) -> u8 {
        match address {
            DIV_ADDRESS => (self.divider >> 8) as u8,
            TIMA_ADDRESS => self.tima,
            TMA_ADDRESS => self.tma,
            TAC_ADDRESS => self.tac,
            _ => 0xFF,
        }
    }

    pub fn write(&mut self, address: u16, value: u8) {
        match address {
            //Writing DIV clears the whole internal counter
            DIV_ADDRESS => self.divider = 0,
            TIMA_ADDRESS => self.tima = value,
            TMA_ADDRESS => self.tma = value,
            TAC_ADDRESS => self.tac = value,
            _ => {}
        }
    }

    //Advance the timer by cycles T-cycles. Returns true if TIMA overflowed and a
    //Timer interrupt should be requested
    pub fn step(&mut self, cycles: u8) -> bool {
        //DIV runs unconditionally.
        self.divider = self.divider.wrapping_add(cycles as u16);

        if self.tac & TAC_ENABLE == 0 {
            return false;
        }

        let period = self.tima_period();
        self.tima_cycles += cycles as u16;

        let mut interrupt = false;
        while self.tima_cycles >= period {
            self.tima_cycles -= period;
            let (next, overflowed) = self.tima.overflowing_add(1);
            if overflowed {
                self.tima = self.tma; // reload from the modulo register
                interrupt = true;
            } else {
                self.tima = next;
            }
        }
        interrupt
    }

    //How many T cycles between TIMA ticks, per the low two bits of TAC
    fn tima_period(&self) -> u16 {
        match self.tac & 0b11 {
            0b00 => 1024, // 4096 Hz
            0b01 => 16, // 262144 Hz
            0b10 => 64, // 65536 Hz
            _ => 256, // 16384 Hz
        }
    }
}
