use midi_msg::Channel;

pub struct KeyState {
    on_channels: u16,
    held_channels: u16,
}

impl KeyState {
    pub fn new() -> Self {
        Self {
            on_channels: 0,
            held_channels: 0,
        }
    }

    pub fn is_sounding(&self) -> bool {
        (self.on_channels != 0) | (self.held_channels != 0)
    }

    pub fn note_on(&mut self, channel: Channel) -> bool {
        let state_change = !self.is_sounding();

        self.on_channels |= 1 << channel as u8;

        state_change
    }

    pub fn note_off(&mut self, channel: Channel, pedal_hold: bool) -> bool {
        let was_sounding = self.is_sounding();

        if pedal_hold {
            self.held_channels |= self.on_channels & (1 << channel as u8);
        }

        self.on_channels &= !(1 << channel as u8);

        was_sounding && !self.is_sounding()
    }

    pub fn pedal_off(&mut self, channel: Channel) -> bool {
        let was_sounding = self.is_sounding();

        self.held_channels &= !(1 << channel as u8);

        was_sounding && !self.is_sounding()
    }
}

#[cfg(test)]
mod test {
    use super::KeyState;
    use midi_msg::Channel::*;

    #[test]
    fn pedal_off_test() {
        let mut s = KeyState::new();

        s.note_on(Ch1);
        s.note_off(Ch1, true);

        assert!(s.is_sounding());

        s.pedal_off(Ch1);

        assert!(!s.is_sounding());
    }
}
