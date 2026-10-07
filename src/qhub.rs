use adaptuner::engine::{IntonationEngine, IntonationEvent, MidiEvent};

pub struct IntonationProcessor {
    engine: IntonationEngine,
}

impl IntonationProcessor {
    pub fn new() -> Self {
        Self {
            engine: IntonationEngine::from_template(),
        }
    }

    pub fn process(&mut self, event: MidiEvent) -> Vec<IntonationOutput> {
        self.engine
            .process(event)
            .into_iter()
            .map(IntonationOutput::from)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum IntonationOutput {
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
    Retune {
        time: u64,
        note: u8,
        tuning: Semitones,
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
}

impl From<IntonationEvent> for IntonationOutput {
    fn from(event: IntonationEvent) -> Self {
        match event {
            IntonationEvent::TunedNoteOn {
                time,
                channel,
                note,
                velocity,
                tuning,
            } => Self::TunedNoteOn {
                time,
                channel,
                note,
                velocity,
                tuning,
            },

            IntonationEvent::NoteOn {
                time,
                channel,
                note,
                velocity,
            } => Self::NoteOn {
                time,
                channel,
                note,
                velocity,
            },

            IntonationEvent::NoteOff {
                time,
                channel,
                note,
                velocity,
            } => Self::NoteOff {
                time,
                channel,
                note,
                velocity,
            },

            IntonationEvent::Retune { time, note, tuning } => Self::Retune { time, note, tuning },

            IntonationEvent::ControlChange {
                time,
                channel,
                controller,
                value,
            } => Self::ControlChange {
                time,
                channel,
                controller,
                value,
            },

            IntonationEvent::ProgramChange {
                time,
                channel,
                program,
            } => Self::ProgramChange {
                time,
                channel,
                program,
            },
        }
    }
}
