use serde_derive::{Deserialize, Serialize};

use crate::{
    interval::stack::Stack, interval::stacktype::r#trait::IntervalBasis,
    neighbourhood::SomeCompleteNeighbourhood, reference::Reference,
};

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct StaticTuningConfig<T: IntervalBasis> {
    pub neighbourhoods: Vec<SomeCompleteNeighbourhood<T>>,
    pub tuning_reference: Reference<T>,
    pub reference: Stack<T>,
}
