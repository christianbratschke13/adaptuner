use crate::{
    interval::{
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, StackType},
    },
    neighbourhood::SomeCompleteNeighbourhood,
    reference::Reference,
    strategy::r#static::{StaticTuning, StaticTuningConfig},
    strategy::r#trait::Strategy,
};

#[derive(Clone)]
pub enum StrategyConfig<T: IntervalBasis> {
    StaticTuning(StaticTuningConfig<T>),
}

impl<T: StackType> StrategyConfig<T> {
    pub fn realize(self) -> Box<dyn Strategy<T>> {
        match self {
            StrategyConfig::StaticTuning(config) => Box::new(StaticTuning::new(config)),
        }
    }
}

#[derive(Clone)]
pub struct ProcessConfig<T: IntervalBasis> {
    pub strategies: Vec<StrategyConfig<T>>,
}

#[derive(Clone)]
pub struct CoreConfig<T: IntervalBasis> {
    pub neighbourhoods: Vec<SomeCompleteNeighbourhood<T>>,
    pub tuning_reference: Reference<T>,
    pub reference: Stack<T>,
}
