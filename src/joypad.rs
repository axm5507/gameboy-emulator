//The joypad is 1 register, P1/JOYP at 0xFF00. It is shared by all 8 buttons
//via a matrix. The program selects 1 of 2 button groups by writing bits 4 to 5
//and then reading the 4 input lines in bits 0 to 3.
//bit 0-3: input lines for selected group
//bit 4: select direction buttons
//bit 5: select action buttons
//bit 6-7: unused
pub const JOYPAD_ADDRESS: u16 = 0xFF00;

const SELECT_MASK: u8 = 0x30; // bits 4-5
const SELECT_DIRECTIONS: u8 = 1 << 4;
const SELECT_ACTIONS: u8 = 1 << 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    //Action group
    A,
    B,
    Select,
    Start,
    //Direction group
    Right,
    Left,
    Up,
    Down,
}

impl Button {
    fn is_direction(self) -> bool {
        matches!(self, Button::Right | Button::Left | Button::Up | Button::Down)
    }

    //Which of the four input lines (bits 0-3) this button drives within its group
    fn line(self) -> u8 {
        match self {
            Button::Right | Button::A => 1 << 0,
            Button::Left | Button::B => 1 << 1,
            Button::Up | Button::Select => 1 << 2,
            Button::Down | Button::Start => 1 << 3,
        }
    }
}

pub struct Joypad {
    select: u8,
    //Pressed state, 1 = pressed, laid out to match the input lines of each group
    directions: u8,
    actions: u8,
}

impl Joypad {
    pub fn new() -> Self {
        //Both groups selected, nothing pressed 
        Self {
            select: 0,
            directions: 0,
            actions: 0,
        }
    }

    pub fn read(&self) -> u8 {
        //Input lines default to 1 
        let mut lines = 0x0F;
        if self.select & SELECT_DIRECTIONS == 0 {
            lines &= !self.directions & 0x0F;
        }
        if self.select & SELECT_ACTIONS == 0 {
            lines &= !self.actions & 0x0F;
        }
        //Unused top bits read 1, then select bits, then lines
        0xC0 | (self.select & SELECT_MASK) | lines
    }

    pub fn write(&mut self, value: u8) {
        //Only the two select bits are writable
        self.select = value & SELECT_MASK;
    }

    //Press a button. Returns true if this was a fresh press 
    pub fn press(&mut self, button: Button) -> bool {
        let line = button.line();
        let group = self.group_mut(button);
        let was_pressed = *group & line != 0;
        *group |= line;
        !was_pressed
    }

    pub fn release(&mut self, button: Button) {
        let line = button.line();
        *self.group_mut(button) &= !line;
    }

    fn group_mut(&mut self, button: Button) -> &mut u8 {
        if button.is_direction() {
            &mut self.directions
        } else {
            &mut self.actions
        }
    }
}
