use adaptuner::{
    config::Config,
    engine::{IntonationEngine, IntonationEvent, MidiEvent, MidiMessage},
    interval::stacktype::fivelimit::TheFiveLimitStackType,
};
use midi_msg::Channel;

const TEMPLATE_CONFIG: &str = include_str!("../configs/template.yaml");

fn engine_from_template() -> IntonationEngine {
    let config: Config<TheFiveLimitStackType> = serde_yml::from_str(TEMPLATE_CONFIG).unwrap();

    IntonationEngine::from_config(config)
}

#[test]
fn engine_can_be_used_through_public_api() {
    let mut engine = engine_from_template();

    let events = engine.process(MidiEvent {
        time: 123,
        channel: Channel::Ch1,
        message: MidiMessage::NoteOn {
            note: 60,
            velocity: 100,
        },
    });

    assert_eq!(events.len(), 2);

    assert!(matches!(
        events[0],
        IntonationEvent::TunedNoteOn {
            time: 123,
            channel: Channel::Ch1,
            note: 60,
            velocity: 100,
            ..
        }
    ));

    assert!(matches!(
        events[1],
        IntonationEvent::Retune {
            time: 123,
            note: 60,
            ..
        }
    ));
}

#[test]
fn static_tuning_constructor_matches_config_constructor() {
    let config: Config<TheFiveLimitStackType> = serde_yml::from_str(TEMPLATE_CONFIG).unwrap();

    let static_config = config
        .first_static_tuning_config()
        .expect("template should contain a static tuning strategy");

    let mut from_config = IntonationEngine::from_config(config);
    let mut from_static = IntonationEngine::from_static_tuning(static_config);

    let input = MidiEvent {
        time: 123,
        channel: Channel::Ch1,
        message: MidiMessage::NoteOn {
            note: 60,
            velocity: 100,
        },
    };

    assert_eq!(
        from_config.process(input.clone()),
        from_static.process(input),
    );
}

#[test]
fn qhub_chord_input_produces_stable_intonation_events() {
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
            time: 100_000,
            channel: Channel::Ch1,
            message: MidiMessage::NoteOn {
                note: 64,
                velocity: 100,
            },
        },
        MidiEvent {
            time: 200_000,
            channel: Channel::Ch1,
            message: MidiMessage::NoteOn {
                note: 67,
                velocity: 100,
            },
        },
        MidiEvent {
            time: 300_000,
            channel: Channel::Ch1,
            message: MidiMessage::NoteOff {
                note: 64,
                velocity: 0,
            },
        },
    ];

    let output: Vec<_> = events
        .into_iter()
        .flat_map(|event| engine.process(event))
        .collect();

    assert!(matches!(
        output[0],
        IntonationEvent::TunedNoteOn {
            note: 60,
            time: 0,
            ..
        }
    ));

    assert!(matches!(
        output[2],
        IntonationEvent::TunedNoteOn {
            note: 64,
            time: 100_000,
            ..
        }
    ));

    assert!(matches!(
        output[4],
        IntonationEvent::TunedNoteOn {
            note: 67,
            time: 200_000,
            ..
        }
    ));

    assert!(matches!(
        output.last(),
        Some(IntonationEvent::NoteOff {
            note: 64,
            time: 300_000,
            ..
        })
    ));
}

#[test]
fn qhub_chord_lifecycle_produces_expected_events() {
    let mut engine = engine_from_template();

    let input = [
        MidiEvent {
            time: 0,
            channel: Channel::Ch1,
            message: MidiMessage::NoteOn {
                note: 60,
                velocity: 100,
            },
        },
        MidiEvent {
            time: 100_000,
            channel: Channel::Ch1,
            message: MidiMessage::NoteOn {
                note: 64,
                velocity: 100,
            },
        },
        MidiEvent {
            time: 200_000,
            channel: Channel::Ch1,
            message: MidiMessage::NoteOn {
                note: 67,
                velocity: 100,
            },
        },
        MidiEvent {
            time: 300_000,
            channel: Channel::Ch1,
            message: MidiMessage::NoteOff {
                note: 64,
                velocity: 0,
            },
        },
        MidiEvent {
            time: 400_000,
            channel: Channel::Ch1,
            message: MidiMessage::NoteOff {
                note: 67,
                velocity: 0,
            },
        },
        MidiEvent {
            time: 500_000,
            channel: Channel::Ch1,
            message: MidiMessage::NoteOff {
                note: 60,
                velocity: 0,
            },
        },
    ];

    let output: Vec<_> = input
        .into_iter()
        .flat_map(|event| engine.process(event))
        .collect();

    let tuned_notes: Vec<_> = output
        .iter()
        .filter_map(|event| match event {
            IntonationEvent::TunedNoteOn {
                time,
                channel,
                note,
                velocity,
                tuning,
            } => Some((*time, *channel, *note, *velocity, *tuning)),
            _ => None,
        })
        .collect();

    assert_eq!(tuned_notes.len(), 3);

    assert_eq!(tuned_notes[0].0, 0);
    assert_eq!(tuned_notes[0].2, 60);
    assert_eq!(tuned_notes[0].3, 100);

    assert_eq!(tuned_notes[1].0, 100_000);
    assert_eq!(tuned_notes[1].2, 64);
    assert_eq!(tuned_notes[1].3, 100);

    assert_eq!(tuned_notes[2].0, 200_000);
    assert_eq!(tuned_notes[2].2, 67);
    assert_eq!(tuned_notes[2].3, 100);

    let note_offs: Vec<_> = output
        .iter()
        .filter_map(|event| match event {
            IntonationEvent::NoteOff {
                time,
                channel,
                note,
                velocity,
            } => Some((*time, *channel, *note, *velocity)),
            _ => None,
        })
        .collect();

    assert_eq!(
        note_offs,
        vec![
            (300_000, Channel::Ch1, 64, 0),
            (400_000, Channel::Ch1, 67, 0),
            (500_000, Channel::Ch1, 60, 0),
        ]
    );
}

#[test]
fn template_engine_has_public_api() {
    let mut engine = IntonationEngine::from_template();

    let events = engine.process(MidiEvent {
        time: 0,
        channel: Channel::Ch1,
        message: MidiMessage::NoteOn {
            note: 60,
            velocity: 100,
        },
    });

    assert!(matches!(
        events.first(),
        Some(IntonationEvent::TunedNoteOn { note: 60, .. })
    ));
}
