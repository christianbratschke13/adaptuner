use std::time::{Duration, Instant};

use crate::{
    bindable::Bindings,
    config::{Config, FromConfigAndState, ProcessConfig, StrategyConfig},
    engine::event::{IntonationEvent, MidiEvent, MidiMessage},
    interval::stacktype::fivelimit::TheFiveLimitStackType,
    msg::{FromProcess, FromStrategy},
    process::fromstrategy::ProcessFromStrategy,
    strategy::r#static::StaticTuningConfig,
};

pub struct IntonationEngine {
    process: ProcessFromStrategy<TheFiveLimitStackType>,
    origin: Instant,
}

impl IntonationEngine {
    pub fn from_config(config: Config<TheFiveLimitStackType>) -> Self {
        let (process_config, _, _) = config.split();

        Self::new(process_config)
    }

    pub fn from_template() -> Self {
        let config: Config<TheFiveLimitStackType> =
            serde_yml::from_str(include_str!("../../configs/template.yaml"))
                .expect("template config must be valid");

        let static_config = config
            .first_static_tuning_config()
            .expect("template config must contain a static tuning strategy");

        Self::from_static_tuning(static_config)
    }

    fn new(process_config: ProcessConfig<TheFiveLimitStackType>) -> Self {
        Self {
            process: ProcessFromStrategy::initialise(process_config, ()),
            origin: Instant::now(),
        }
    }

    pub fn from_static_tuning(config: StaticTuningConfig<TheFiveLimitStackType>) -> Self {
        let process_config = ProcessConfig {
            strategies: vec![(StrategyConfig::StaticTuning(config), Bindings::empty())],
        };

        Self::new(process_config)
    }

    pub fn process(&mut self, event: MidiEvent) -> Vec<IntonationEvent> {
        let time = self.origin + Duration::from_micros(event.time);

        let messages = match event.message {
            MidiMessage::NoteOn { note, velocity } => {
                self.process.note_on(event.channel, note, velocity, time)
            }

            MidiMessage::NoteOff { note, velocity } => {
                self.process.note_off(event.channel, note, velocity, time)
            }

            MidiMessage::ControlChange {
                controller: 64,
                value,
            } => self.process.pedal_hold(event.channel, value, time),

            MidiMessage::ControlChange { .. } => Vec::new(),

            MidiMessage::ProgramChange { program } => {
                self.process.program_change(event.channel, program, time)
            }
        };

        messages
            .into_iter()
            .filter_map(|event| self.map_event(event))
            .collect()
    }

    fn timestamp(&self, time: Instant) -> u64 {
        time.duration_since(self.origin).as_micros() as u64
    }

    fn map_event(&self, event: FromProcess<TheFiveLimitStackType>) -> Option<IntonationEvent> {
        match event {
            FromProcess::TunedNoteOn {
                channel,
                note,
                velocity,
                tuning,
                time,
                ..
            } => Some(IntonationEvent::TunedNoteOn {
                time: self.timestamp(time),
                channel,
                note,
                velocity,
                tuning,
            }),

            FromProcess::NoteOn {
                channel,
                note,
                velocity,
                time,
            } => Some(IntonationEvent::NoteOn {
                time: self.timestamp(time),
                channel,
                note,
                velocity,
            }),

            FromProcess::NoteOff {
                channel,
                note,
                velocity,
                time,
            } => Some(IntonationEvent::NoteOff {
                time: self.timestamp(time),
                channel,
                note,
                velocity,
            }),

            FromProcess::PedalHold {
                channel,
                value,
                time,
            } => Some(IntonationEvent::ControlChange {
                time: self.timestamp(time),
                channel,
                controller: 64,
                value,
            }),

            FromProcess::ProgramChange {
                channel,
                program,
                time,
            } => Some(IntonationEvent::ProgramChange {
                time: self.timestamp(time),
                channel,
                program,
            }),

            FromProcess::FromStrategy(FromStrategy::Retune {
                note, tuning, time, ..
            }) => Some(IntonationEvent::Retune {
                time: self.timestamp(time),
                note,
                tuning,
            }),

            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use midi_msg::Channel;

    const TEMPLATE_CONFIG: &str = include_str!("../../configs/template.yaml");

    fn engine_from_template() -> IntonationEngine {
        let config: Config<TheFiveLimitStackType> = serde_yml::from_str(TEMPLATE_CONFIG).unwrap();

        IntonationEngine::from_config(config)
    }

    fn process(engine: &mut IntonationEngine, message: MidiMessage) -> Vec<IntonationEvent> {
        engine.process(MidiEvent {
            time: 0,
            channel: Channel::Ch1,
            message,
        })
    }

    #[test]
    fn note_on_produces_tuned_note_on() {
        let mut engine = engine_from_template();

        let events = process(
            &mut engine,
            MidiMessage::NoteOn {
                note: 60,
                velocity: 100,
            },
        );

        assert!(events.iter().any(|event| matches!(
            event,
            IntonationEvent::TunedNoteOn {
                channel: Channel::Ch1,
                note: 60,
                velocity: 100,
                ..
            }
        )));
    }

    #[test]
    fn note_on_produces_retune_event() {
        let mut engine = engine_from_template();

        let events = process(
            &mut engine,
            MidiMessage::NoteOn {
                note: 60,
                velocity: 100,
            },
        );

        assert!(events
            .iter()
            .any(|event| matches!(event, IntonationEvent::Retune { note: 60, .. })));
    }

    #[test]
    fn note_off_produces_note_off() {
        let mut engine = engine_from_template();

        process(
            &mut engine,
            MidiMessage::NoteOn {
                note: 60,
                velocity: 100,
            },
        );

        let events = process(
            &mut engine,
            MidiMessage::NoteOff {
                note: 60,
                velocity: 64,
            },
        );

        assert!(events.iter().any(|event| matches!(
            event,
            IntonationEvent::NoteOff {
                channel: Channel::Ch1,
                note: 60,
                velocity: 64,
                ..
            }
        )));
    }

    #[test]
    fn sustain_pedal_is_forwarded() {
        let mut engine = engine_from_template();

        let events = process(
            &mut engine,
            MidiMessage::ControlChange {
                controller: 64,
                value: 127,
            },
        );

        assert!(events.iter().any(|event| matches!(
            event,
            IntonationEvent::ControlChange {
                channel: Channel::Ch1,
                controller: 64,
                value: 127,
                ..
            }
        )));
    }

    #[test]
    fn program_change_is_forwarded() {
        let mut engine = engine_from_template();

        let events = process(&mut engine, MidiMessage::ProgramChange { program: 12 });

        assert!(events.iter().any(|event| matches!(
            event,
            IntonationEvent::ProgramChange {
                channel: Channel::Ch1,
                program: 12,
                ..
            }
        )));
    }

    #[test]
    fn second_note_on_is_tuned_independently() {
        let mut engine = engine_from_template();

        process(
            &mut engine,
            MidiMessage::NoteOn {
                note: 60,
                velocity: 100,
            },
        );

        let events = process(
            &mut engine,
            MidiMessage::NoteOn {
                note: 64,
                velocity: 100,
            },
        );

        assert!(events.iter().any(|event| matches!(
            event,
            IntonationEvent::TunedNoteOn {
                note: 64,
                velocity: 100,
                ..
            }
        )));
    }

    #[test]
    fn same_note_on_while_already_pressed_is_not_tuned_again() {
        let mut engine = engine_from_template();

        process(
            &mut engine,
            MidiMessage::NoteOn {
                note: 60,
                velocity: 100,
            },
        );

        let events = process(
            &mut engine,
            MidiMessage::NoteOn {
                note: 60,
                velocity: 80,
            },
        );

        assert!(events.iter().any(|event| matches!(
            event,
            IntonationEvent::NoteOn {
                note: 60,
                velocity: 80,
                ..
            }
        )));

        assert!(!events
            .iter()
            .any(|event| matches!(event, IntonationEvent::TunedNoteOn { note: 60, .. })));
    }

    #[test]
    fn note_can_be_played_again_after_note_off() {
        let mut engine = engine_from_template();

        process(
            &mut engine,
            MidiMessage::NoteOn {
                note: 60,
                velocity: 100,
            },
        );

        process(
            &mut engine,
            MidiMessage::NoteOff {
                note: 60,
                velocity: 64,
            },
        );

        let events = process(
            &mut engine,
            MidiMessage::NoteOn {
                note: 60,
                velocity: 90,
            },
        );

        assert!(events.iter().any(|event| matches!(
            event,
            IntonationEvent::TunedNoteOn {
                note: 60,
                velocity: 90,
                ..
            }
        )));
    }

    #[test]
    fn chord_notes_are_individually_tuned() {
        let mut engine = engine_from_template();

        let mut tunings = Vec::new();

        for note in [60, 64, 67] {
            let events = process(
                &mut engine,
                MidiMessage::NoteOn {
                    note,
                    velocity: 100,
                },
            );

            let tuning = events.iter().find_map(|event| match event {
                IntonationEvent::TunedNoteOn {
                    note: event_note,
                    tuning,
                    ..
                } if *event_note == note => Some(*tuning),
                _ => None,
            });

            assert!(tuning.is_some(), "note {note} was not tuned");

            tunings.push((note, tuning.unwrap()));
        }

        assert_ne!(tunings[0].1, 60.0);
        assert_ne!(tunings[1].1, 64.0);
        assert_ne!(tunings[2].1, 67.0);
    }

    #[test]
    fn event_time_is_preserved() {
        let mut engine = engine_from_template();

        let events = engine.process(MidiEvent {
            time: 1_500_000,
            channel: Channel::Ch1,
            message: MidiMessage::NoteOn {
                note: 60,
                velocity: 100,
            },
        });

        assert!(events.iter().any(|event| matches!(
            event,
            IntonationEvent::TunedNoteOn {
                time: 1_500_000,
                note: 60,
                ..
            }
        )));
    }

    #[test]
    fn chord_sequence_preserves_event_order_and_time() {
        let mut engine = engine_from_template();

        let events = [
            MidiEvent {
                time: 0,
                channel: Channel::Ch1,
                message: MidiMessage::NoteOn {
                    note: 60,
                    velocity: 100,
                },
            },
            MidiEvent {
                time: 100,
                channel: Channel::Ch1,
                message: MidiMessage::NoteOn {
                    note: 64,
                    velocity: 100,
                },
            },
            MidiEvent {
                time: 200,
                channel: Channel::Ch1,
                message: MidiMessage::NoteOn {
                    note: 67,
                    velocity: 100,
                },
            },
            MidiEvent {
                time: 500,
                channel: Channel::Ch1,
                message: MidiMessage::NoteOff {
                    note: 60,
                    velocity: 0,
                },
            },
            MidiEvent {
                time: 600,
                channel: Channel::Ch1,
                message: MidiMessage::NoteOff {
                    note: 64,
                    velocity: 0,
                },
            },
            MidiEvent {
                time: 700,
                channel: Channel::Ch1,
                message: MidiMessage::NoteOff {
                    note: 67,
                    velocity: 0,
                },
            },
        ];

        let mut output = Vec::new();

        for event in events {
            output.extend(engine.process(event));
        }

        assert_eq!(output.len(), 9);

        assert!(matches!(
            output[0],
            IntonationEvent::TunedNoteOn {
                time: 0,
                note: 60,
                ..
            }
        ));
        assert!(matches!(
            output[1],
            IntonationEvent::Retune {
                time: 0,
                note: 60,
                ..
            }
        ));

        assert!(matches!(
            output[2],
            IntonationEvent::TunedNoteOn {
                time: 100,
                note: 64,
                ..
            }
        ));
        assert!(matches!(
            output[3],
            IntonationEvent::Retune {
                time: 100,
                note: 64,
                ..
            }
        ));

        assert!(matches!(
            output[4],
            IntonationEvent::TunedNoteOn {
                time: 200,
                note: 67,
                ..
            }
        ));
        assert!(matches!(
            output[5],
            IntonationEvent::Retune {
                time: 200,
                note: 67,
                ..
            }
        ));

        assert!(matches!(
            output[6],
            IntonationEvent::NoteOff {
                time: 500,
                note: 60,
                ..
            }
        ));
        assert!(matches!(
            output[7],
            IntonationEvent::NoteOff {
                time: 600,
                note: 64,
                ..
            }
        ));
        assert!(matches!(
            output[8],
            IntonationEvent::NoteOff {
                time: 700,
                note: 67,
                ..
            }
        ));
    }

    #[test]
    fn identical_input_produces_identical_output() {
        let mut engine_a = engine_from_template();
        let mut engine_b = engine_from_template();

        let events = [
            MidiEvent {
                time: 0,
                channel: Channel::Ch1,
                message: MidiMessage::NoteOn {
                    note: 60,
                    velocity: 100,
                },
            },
            MidiEvent {
                time: 100,
                channel: Channel::Ch1,
                message: MidiMessage::NoteOn {
                    note: 64,
                    velocity: 100,
                },
            },
            MidiEvent {
                time: 200,
                channel: Channel::Ch1,
                message: MidiMessage::NoteOff {
                    note: 60,
                    velocity: 0,
                },
            },
        ];

        let output_a: Vec<_> = events
            .iter()
            .cloned()
            .flat_map(|event| engine_a.process(event))
            .collect();

        let output_b: Vec<_> = events
            .iter()
            .cloned()
            .flat_map(|event| engine_b.process(event))
            .collect();

        assert_eq!(output_a, output_b);
    }
}
