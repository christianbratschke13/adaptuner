use wasm_bindgen::prelude::*;

use crate::{
    engine::{IntonationEngine, IntonationEvent, MidiEvent, MidiMessage},
    interval::{
        stack::Stack,
        stacktype::{fivelimit::TheFiveLimitStackType, r#trait::IntervalBasis},
    },
    neighbourhood::PeriodicComplete,
    reference::Reference,
    strategy::static_config::StaticTuningConfig,
};

const TEMPLATE_CONFIG: &str = include_str!("../configs/template.yaml");

#[wasm_bindgen(start)]
pub fn wasm_start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct AdaptunerEngine {
    inner: IntonationEngine,
}

#[wasm_bindgen]
impl AdaptunerEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<AdaptunerEngine, JsValue> {
        let config = load_template_config()?;

        Ok(Self {
            inner: IntonationEngine::from_static_tuning(config),
        })
    }

    pub fn process(&mut self, events: JsValue) -> Result<JsValue, JsValue> {
        let events: Vec<JsMidiEvent> = serde_wasm_bindgen::from_value(events)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;

        let mut output = Vec::new();

        for event in events {
            let event = convert_event(event)?;

            output.extend(self.inner.process(event).into_iter().map(convert_output));
        }

        serde_wasm_bindgen::to_value(&output).map_err(|error| JsValue::from_str(&error.to_string()))
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
struct WasmTemplate {
    strategies: Vec<serde_yml::Value>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
struct WasmStrategy<T: IntervalBasis> {
    config: WasmStrategyConfig<T>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
enum WasmStrategyConfig<T: IntervalBasis> {
    StaticTuning(WasmStaticTuningConfig<T>),
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
struct WasmStaticTuningConfig<T: IntervalBasis> {
    neighbourhoods: Vec<WasmNamedCompleteNeighbourhood<T>>,
    tuning_reference: Reference<T>,
    reference: Stack<T>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
struct WasmNamedCompleteNeighbourhood<T: IntervalBasis> {
    name: String,
    entries: PeriodicComplete<T>,
}

#[derive(serde::Deserialize)]
struct TemplateConfig {
    strategies: Vec<TemplateStrategy>,
}

#[derive(serde::Deserialize)]
struct TemplateStrategy {
    config: StaticTuningConfig<TheFiveLimitStackType>,
}

fn load_template_config() -> Result<StaticTuningConfig<TheFiveLimitStackType>, JsValue> {
    let template: WasmTemplate = serde_yml::from_str(TEMPLATE_CONFIG)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;

    let strategy = template
        .strategies
        .into_iter()
        .next()
        .ok_or_else(|| JsValue::from_str("template config has no strategies"))?;

    let strategy: WasmStrategy<TheFiveLimitStackType> =
        serde_yml::from_value(strategy).map_err(|error| JsValue::from_str(&error.to_string()))?;

    let WasmStrategyConfig::StaticTuning(config) = strategy.config;

    Ok(StaticTuningConfig {
        neighbourhoods: config
            .neighbourhoods
            .into_iter()
            .map(|neighbourhood| neighbourhood.entries.into())
            .collect(),
        tuning_reference: config.tuning_reference,
        reference: config.reference,
    })
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsMidiEvent {
    time: u64,
    channel: u8,
    message: JsMidiMessage,
}

#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum JsMidiMessage {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8, velocity: u8 },
    ControlChange { controller: u8, value: u8 },
    ProgramChange { program: u8 },
}

fn parse_channel(channel: u8) -> Result<midi_msg::Channel, JsValue> {
    match channel {
        0 => Ok(midi_msg::Channel::Ch1),
        1 => Ok(midi_msg::Channel::Ch2),
        2 => Ok(midi_msg::Channel::Ch3),
        3 => Ok(midi_msg::Channel::Ch4),
        4 => Ok(midi_msg::Channel::Ch5),
        5 => Ok(midi_msg::Channel::Ch6),
        6 => Ok(midi_msg::Channel::Ch7),
        7 => Ok(midi_msg::Channel::Ch8),
        8 => Ok(midi_msg::Channel::Ch9),
        9 => Ok(midi_msg::Channel::Ch10),
        10 => Ok(midi_msg::Channel::Ch11),
        11 => Ok(midi_msg::Channel::Ch12),
        12 => Ok(midi_msg::Channel::Ch13),
        13 => Ok(midi_msg::Channel::Ch14),
        14 => Ok(midi_msg::Channel::Ch15),
        15 => Ok(midi_msg::Channel::Ch16),
        _ => Err(JsValue::from_str("MIDI channel must be between 0 and 15")),
    }
}

fn convert_event(event: JsMidiEvent) -> Result<MidiEvent, JsValue> {
    let channel = parse_channel(event.channel)?;

    let message = match event.message {
        JsMidiMessage::NoteOn { note, velocity } => MidiMessage::NoteOn { note, velocity },
        JsMidiMessage::NoteOff { note, velocity } => MidiMessage::NoteOff { note, velocity },
        JsMidiMessage::ControlChange { controller, value } => {
            MidiMessage::ControlChange { controller, value }
        }
        JsMidiMessage::ProgramChange { program } => MidiMessage::ProgramChange { program },
    };

    Ok(MidiEvent {
        time: event.time,
        channel,
        message,
    })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct JsIntonationEvent {
    time: u64,
    channel: Option<u8>,
    event: JsIntonationEventKind,
}

#[derive(serde::Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum JsIntonationEventKind {
    NoteOn { note: u8, velocity: u8 },
    TunedNoteOn { note: u8, velocity: u8, tuning: f64 },
    NoteOff { note: u8, velocity: u8 },
    ControlChange { controller: u8, value: u8 },
    ProgramChange { program: u8 },
    Retune { note: u8, tuning: f64 },
}

fn convert_output(event: IntonationEvent) -> JsIntonationEvent {
    match event {
        IntonationEvent::NoteOn {
            time,
            channel,
            note,
            velocity,
        } => JsIntonationEvent {
            time,
            channel: Some(channel as u8),
            event: JsIntonationEventKind::NoteOn { note, velocity },
        },

        IntonationEvent::TunedNoteOn {
            time,
            channel,
            note,
            velocity,
            tuning,
        } => JsIntonationEvent {
            time,
            channel: Some(channel as u8),
            event: JsIntonationEventKind::TunedNoteOn {
                note,
                velocity,
                tuning,
            },
        },

        IntonationEvent::NoteOff {
            time,
            channel,
            note,
            velocity,
        } => JsIntonationEvent {
            time,
            channel: Some(channel as u8),
            event: JsIntonationEventKind::NoteOff { note, velocity },
        },

        IntonationEvent::ControlChange {
            time,
            channel,
            controller,
            value,
        } => JsIntonationEvent {
            time,
            channel: Some(channel as u8),
            event: JsIntonationEventKind::ControlChange { controller, value },
        },

        IntonationEvent::ProgramChange {
            time,
            channel,
            program,
        } => JsIntonationEvent {
            time,
            channel: Some(channel as u8),
            event: JsIntonationEventKind::ProgramChange { program },
        },

        IntonationEvent::Retune { time, note, tuning } => JsIntonationEvent {
            time,
            channel: None,
            event: JsIntonationEventKind::Retune { note, tuning },
        },
    }
}
