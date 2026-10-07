use crate::interval::base::Semitones;
use midi_msg::Channel;

#[derive(Debug, Clone, PartialEq)]
pub struct MidiEvent {
    /// Zeit seit Beginn des Streams in Mikrosekunden.
    pub time: u64,
    pub channel: Channel,
    pub message: MidiMessage,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MidiMessage {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8, velocity: u8 },
    ControlChange { controller: u8, value: u8 },
    ProgramChange { program: u8 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum IntonationEvent {
    TunedNoteOn {
        time: u64,
        channel: Channel,
        note: u8,
        velocity: u8,
        tuning: Semitones,
    },

    NoteOn {
        time: u64,
        channel: Channel,
        note: u8,
        velocity: u8,
    },

    NoteOff {
        time: u64,
        channel: Channel,
        note: u8,
        velocity: u8,
    },

    ControlChange {
        time: u64,
        channel: Channel,
        controller: u8,
        value: u8,
    },

    ProgramChange {
        time: u64,
        channel: Channel,
        program: u8,
    },

    Retune {
        time: u64,
        note: u8,
        tuning: Semitones,
    },
}
