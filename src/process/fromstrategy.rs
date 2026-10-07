use std::{collections::VecDeque, fmt, sync::mpsc, time::Instant};

use midi_msg::{
    Channel,
    ChannelVoiceMsg::*,
    ControlChange::{Hold, SoftPedal, Sostenuto},
    MidiMsg,
};

use crate::{
    bindable::{Bindings, MidiBindable},
    config::{ExtractConfig, FromConfigAndState, ProcessConfig},
    interval::{stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    msg::{FromProcess, FromStrategy, HandleMsg, ToProcess, ToStrategy},
    strategy::r#trait::Strategy,
};

pub struct ProcessFromStrategy<T: StackType> {
    strategies: Vec<(Box<dyn Strategy<T>>, Bindings<MidiBindable>)>,
    curr_strategy_index: Option<usize>,
    key_states: [KeyState; 128],
    tunings: [Stack<T>; 128],
    pedal_hold: [bool; 16],
    sostenuto_hold: [bool; 16],
    soft_hold: [bool; 16],
    queue: VecDeque<FromStrategy<T>>,
}

enum ProcessOutput<'a, T: StackType> {
    Channel(&'a mpsc::Sender<FromProcess<T>>),
    Vec(&'a mut Vec<FromProcess<T>>),
}

impl<T: StackType> ProcessOutput<'_, T> {
    fn send(&mut self, message: FromProcess<T>) {
        match self {
            Self::Channel(sender) => {
                let _ = sender.send(message);
            }
            Self::Vec(messages) => {
                messages.push(message);
            }
        }
    }
}

impl<T: StackType> ProcessFromStrategy<T> {
    pub fn new(strategies: Vec<(Box<dyn Strategy<T>>, Bindings<MidiBindable>)>) -> Self {
        let now = Instant::now();
        Self {
            curr_strategy_index: if strategies.len() > 0 {
                Some(0)
            } else {
                None {}
            },
            strategies,
            key_states: core::array::from_fn(|_| KeyState::new(now)),
            tunings: core::array::from_fn(|_| Stack::new_zero()),
            pedal_hold: [false; 16],
            sostenuto_hold: [false; 16],
            soft_hold: [false; 16],
            queue: VecDeque::new(),
        }
    }
}

impl<T: StackType> ProcessFromStrategy<T> {
    fn handle_midi(&mut self, time: Instant, msg: MidiMsg, forward: &mut ProcessOutput<T>) {
        let mut forward_untouched = || {
            forward.send(FromProcess::OutgoingMidi {
                bytes: msg.to_midi(),
                time,
            });
        };

        match msg {
            MidiMsg::ChannelVoice {
                channel,
                msg: NoteOn { note, velocity },
            } => self.handle_note_on(time, note, channel, velocity, forward),
            MidiMsg::ChannelVoice {
                channel,
                msg: NoteOff { note, velocity },
            } => self.handle_note_off(time, note, channel, velocity, forward),

            MidiMsg::ChannelVoice {
                channel,
                msg: ControlChange {
                    control: Hold(value),
                },
            } => self.handle_pedal_hold(time, channel, value, forward),

            MidiMsg::ChannelVoice {
                channel,
                msg:
                    ControlChange {
                        control: Sostenuto(value),
                    },
            } => {
                if let Some(csi) = self.curr_strategy_index {
                    let (ref mut strategy, ref bindings) = self.strategies[csi];
                    let was_down = self.sostenuto_hold.iter().any(|b| *b);
                    self.sostenuto_hold[channel as usize] = value > 0;
                    let is_down = self.sostenuto_hold.iter().any(|b| *b);
                    let action = match (was_down, is_down) {
                        (false, true) => bindings.get(&MidiBindable::SostenutoPedalDown),
                        (true, false) => bindings.get(&MidiBindable::SostenutoPedalUp),
                        _ => None {},
                    };
                    if let Some(&action) = action {
                        let _ = strategy.handle_msg(
                            &self.key_states,
                            &mut self.tunings,
                            ToStrategy::Action { action, time },
                            &mut self.queue,
                        );
                        self.queue.drain(..).for_each(|msg| {
                            forward.send(FromProcess::FromStrategy(msg));
                        });
                    } else {
                        forward_untouched();
                    }
                }
            }

            MidiMsg::ChannelVoice {
                channel,
                msg:
                    ControlChange {
                        control: SoftPedal(value),
                    },
            } => {
                if let Some(csi) = self.curr_strategy_index {
                    let (ref mut strategy, ref bindings) = self.strategies[csi];
                    let was_down = self.soft_hold.iter().any(|b| *b);
                    self.soft_hold[channel as usize] = value > 0;
                    let is_down = self.soft_hold.iter().any(|b| *b);
                    let action = match (was_down, is_down) {
                        (false, true) => bindings.get(&MidiBindable::SoftPedalDown),
                        (true, false) => bindings.get(&MidiBindable::SoftPedalUp),
                        _ => None {},
                    };
                    if let Some(&action) = action {
                        let _ = strategy.handle_msg(
                            &self.key_states,
                            &mut self.tunings,
                            ToStrategy::Action { action, time },
                            &mut self.queue,
                        );
                        self.queue.drain(..).for_each(|msg| {
                            forward.send(FromProcess::FromStrategy(msg));
                        });
                    } else {
                        forward_untouched();
                    }
                }
            }

            MidiMsg::ChannelVoice {
                channel,
                msg: ProgramChange { program },
            } => {
                forward.send(FromProcess::ProgramChange {
                    channel,
                    program,
                    time,
                });
            }

            _ => forward_untouched(),
        }
    }

    fn handle_note_on(
        &mut self,
        time: Instant,
        note: u8,
        channel: Channel,
        velocity: u8,
        forward: &mut ProcessOutput<T>,
    ) {
        let mut send_simple_note_on = || {
            forward.send(FromProcess::NoteOn {
                channel,
                note,
                velocity,
                time,
            });
        };

        if let Some(csi) = self.curr_strategy_index {
            if self.key_states[note as usize].note_on(channel, time) {
                match self.strategies[csi].0.note_on(
                    &self.key_states,
                    &mut self.tunings,
                    note,
                    time,
                    &mut self.queue,
                ) {
                    Some((tuning, tuning_stack)) => {
                        forward.send(FromProcess::TunedNoteOn {
                            channel,
                            note,
                            velocity,
                            tuning,
                            tuning_stack: tuning_stack.clone(),
                            time,
                        });
                        self.queue.drain(..).for_each(|msg| {
                            forward.send(FromProcess::FromStrategy(msg));
                        });
                    }
                    None {} => {
                        send_simple_note_on();
                        self.queue.clear();
                    }
                }
            } else {
                send_simple_note_on();
            }
        }
    }

    fn handle_note_off(
        &mut self,
        time: Instant,
        note: u8,
        channel: Channel,
        velocity: u8,
        forward: &mut ProcessOutput<T>,
    ) {
        if let Some(csi) = self.curr_strategy_index {
            if self.key_states[note as usize].note_off(
                channel,
                self.pedal_hold[channel as usize],
                time,
            ) {
                self.strategies[csi].0.note_off(
                    &self.key_states,
                    &mut self.tunings,
                    note,
                    time,
                    &mut self.queue,
                );
                self.queue.drain(..).for_each(|msg| {
                    forward.send(FromProcess::FromStrategy(msg));
                });
            }
            forward.send(FromProcess::NoteOff {
                channel,
                note,
                velocity,
                time,
            });
        }
    }

    fn handle_pedal_hold(
        &mut self,
        time: Instant,
        channel: Channel,
        value: u8,
        forward: &mut ProcessOutput<T>,
    ) {
        if let Some(csi) = self.curr_strategy_index {
            if value > 0 {
                self.pedal_hold[channel as usize] = true;
            } else {
                self.pedal_hold[channel as usize] = false;
                for i in 0..128 {
                    let changed = self.key_states[i].pedal_off(channel, time);
                    if changed {
                        let _ = self.strategies[csi].0.note_off(
                            &self.key_states,
                            &mut self.tunings,
                            i as u8,
                            time,
                            &mut self.queue,
                        );
                        self.queue.drain(..).for_each(|msg| {
                            forward.send(FromProcess::FromStrategy(msg));
                        });
                    }
                }
            }
            forward.send(FromProcess::PedalHold {
                channel,
                value,
                time,
            });
        }
    }

    fn start(&mut self, time: Instant, forward: &mut ProcessOutput<T>) {
        if let Some(csi) = self.curr_strategy_index {
            self.strategies[csi].0.start(
                &self.key_states,
                &mut self.tunings,
                time,
                &mut self.queue,
            );
            self.queue.drain(..).for_each(|msg| {
                forward.send(FromProcess::FromStrategy(msg));
            });
        }
        forward.send(FromProcess::CurrentStrategyIndex(self.curr_strategy_index));
    }
}

impl<T: StackType + fmt::Debug + 'static> HandleMsg<ToProcess<T>, FromProcess<T>>
    for ProcessFromStrategy<T>
{
    fn handle_msg(&mut self, msg: ToProcess<T>, forward: &mpsc::Sender<FromProcess<T>>) {
        let mut output = ProcessOutput::Channel(forward);

        match msg {
            ToProcess::Stop => {}
            ToProcess::Reset { time } => self.start(time, &mut output),
            ToProcess::Start { time } => self.start(time, &mut output),
            ToProcess::IncomingMidi { time, bytes } => match MidiMsg::from_midi(&bytes) {
                Ok((msg, _)) => self.handle_midi(time, msg, &mut output),
                Err(e) => {
                    output.send(FromProcess::MidiParseErr(e.to_string()));
                }
            },
            ToProcess::NoteOn {
                channel,
                note,
                velocity,
                time,
            } => self.handle_note_on(time, note, channel, velocity, &mut output),

            ToProcess::NoteOff {
                channel,
                note,
                velocity,
                time,
            } => self.handle_note_off(time, note, channel, velocity, &mut output),

            ToProcess::PedalHold {
                channel,
                value,
                time,
            } => self.handle_pedal_hold(time, channel, value, &mut output),
            ToProcess::ToStrategy(msg) => {
                if let Some(csi) = self.curr_strategy_index {
                    let _success = self.strategies[csi].0.handle_msg(
                        &self.key_states,
                        &mut self.tunings,
                        msg,
                        &mut self.queue,
                    );

                    self.queue.drain(..).for_each(|msg| {
                        output.send(FromProcess::FromStrategy(msg));
                    });
                }
            }
            ToProcess::StrategyListAction { action, time } => {
                action.apply_to(
                    |(strat, bind)| (strat.extract_config().realize(), bind.clone()),
                    &mut self.strategies,
                    &mut self.curr_strategy_index,
                );

                self.start(time, &mut output);
            }
            ToProcess::BindAction { action, bindable } => {
                if let Some(csi) = self.curr_strategy_index {
                    let (_, bindings) = &mut self.strategies[csi];
                    if let Some(action) = action {
                        bindings.insert(bindable, action);
                    } else {
                        bindings.remove(&bindable);
                    }
                }
            }
            ToProcess::GetCurrentConfig => {
                output.send(FromProcess::CurrentConfig(self.extract_config()));
            }
            ToProcess::RestartWithConfig { time, config } => {
                *self = <Self as FromConfigAndState<_, _>>::initialise(config, ());
                self.start(time, &mut output);
            }

            ToProcess::RestartWithCurrentConfig { time } => {
                *self = <Self as FromConfigAndState<_, _>>::initialise(self.extract_config(), ());
                self.start(time, &mut output);
            }
        }
    }
}

impl<T: StackType> ExtractConfig<ProcessConfig<T>> for ProcessFromStrategy<T> {
    fn extract_config(&self) -> ProcessConfig<T> {
        ProcessConfig {
            strategies: self
                .strategies
                .iter()
                .map(|(s, b)| (s.extract_config(), b.clone()))
                .collect(),
        }
    }
}

impl<T: StackType, S> FromConfigAndState<ProcessConfig<T>, S> for ProcessFromStrategy<T> {
    fn initialise(config: ProcessConfig<T>, _state: S) -> Self {
        let ProcessConfig { mut strategies } = config;
        Self::new(
            strategies
                .drain(..)
                .map(|(s, b)| (s.realize(), b))
                .collect(),
        )
    }
}

impl<T: StackType> ProcessFromStrategy<T> {
    pub fn note_on(
        &mut self,
        channel: Channel,
        note: u8,
        velocity: u8,
        time: Instant,
    ) -> Vec<FromProcess<T>> {
        let mut output = Vec::new();
        let mut sink = ProcessOutput::Vec(&mut output);

        self.handle_note_on(time, note, channel, velocity, &mut sink);

        output
    }

    pub fn note_off(
        &mut self,
        channel: Channel,
        note: u8,
        velocity: u8,
        time: Instant,
    ) -> Vec<FromProcess<T>> {
        let mut output = Vec::new();
        let mut sink = ProcessOutput::Vec(&mut output);

        self.handle_note_off(time, note, channel, velocity, &mut sink);

        output
    }

    pub fn pedal_hold(
        &mut self,
        channel: Channel,
        value: u8,
        time: Instant,
    ) -> Vec<FromProcess<T>> {
        let mut output = Vec::new();
        let mut sink = ProcessOutput::Vec(&mut output);

        self.handle_pedal_hold(time, channel, value, &mut sink);

        output
    }

    pub fn program_change(
        &mut self,
        channel: Channel,
        program: u8,
        time: Instant,
    ) -> Vec<FromProcess<T>> {
        vec![FromProcess::ProgramChange {
            channel,
            program,
            time,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use midi_msg::Channel;
    use std::time::Instant;

    use crate::{config::ProcessConfig, interval::stacktype::fivelimit::TheFiveLimitStackType};

    fn empty_process() -> ProcessFromStrategy<TheFiveLimitStackType> {
        ProcessFromStrategy::initialise(
            ProcessConfig {
                strategies: Vec::new(),
            },
            (),
        )
    }

    #[test]
    fn note_on_without_strategy_produces_no_event() {
        let mut process = empty_process();

        let events = process.note_on(Channel::Ch1, 60, 100, Instant::now());

        assert!(events.is_empty());
    }

    #[test]
    fn note_off_without_strategy_produces_no_event() {
        let mut process = empty_process();

        let events = process.note_off(Channel::Ch1, 60, 0, Instant::now());

        assert!(events.is_empty());
    }

    #[test]
    fn pedal_without_strategy_produces_no_event() {
        let mut process = empty_process();

        let events = process.pedal_hold(Channel::Ch1, 127, Instant::now());

        assert!(events.is_empty());
    }

    #[test]
    fn program_change_is_forwarded_without_strategy() {
        let mut process = empty_process();

        let events = process.program_change(Channel::Ch1, 42, Instant::now());

        assert_eq!(events.len(), 1);

        assert!(matches!(
            events[0],
            FromProcess::ProgramChange {
                channel: Channel::Ch1,
                program: 42,
                ..
            }
        ));
    }
}
