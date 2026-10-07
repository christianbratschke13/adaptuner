use crate::{
    interval::{
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    },
    neighbourhood::{CompleteNeigbourhood, SomeCompleteNeighbourhood},
    reference::Reference,
    strategy::static_config::StaticTuningConfig,
};

pub struct StaticTuning<T: IntervalBasis> {
    neighbourhoods: Vec<SomeCompleteNeighbourhood<T>>,
    curr_neighbourhood_index: Option<usize>,
    pub tuning_reference: Reference<T>,
    pub reference: Stack<T>,
    tuning_up_to_date: [bool; 128],
}

impl<T: IntervalBasis> StaticTuning<T> {
    pub fn new(config: StaticTuningConfig<T>) -> Self {
        Self {
            neighbourhoods: config.neighbourhoods,
            curr_neighbourhood_index: Some(0),
            tuning_reference: config.tuning_reference,
            reference: config.reference,
            tuning_up_to_date: [false; 128],
        }
    }
}

impl<T: StackType> StaticTuning<T> {
    /// Compute the tuning for a note (that may lie outside of the MIDI range). Returns `None` only
    /// in the case when there's no neighbourhood currently selected).
    pub fn compute_tuning_for(&self, note: StackCoeff) -> Option<Stack<T>> {
        if let Some(cni) = self.curr_neighbourhood_index {
            let mut res =
                self.neighbourhoods[cni].get_relative_stack(note - self.reference.key_number());
            res.scaled_add(1, &self.reference);
            Some(res)
        } else {
            None {}
        }
    }

    /// Returns `Some` iff the tuning was successfully updated (this will always be the case if
    /// long as there's a selected neighbourhood),
    ///
    /// `Some(true)` means the tuning wasn't previously up to date.
    pub fn update_tuning(&mut self, tunings: &mut [Stack<T>; 128], note: u8) -> Option<bool> {
        if let Some(cni) = self.curr_neighbourhood_index {
            if !self.tuning_up_to_date[note as usize] {
                self.neighbourhoods[cni].write_relative_stack(
                    tunings.get_mut(note as usize).unwrap(),
                    note as StackCoeff - self.reference.key_number(),
                );
                tunings
                    .get_mut(note as usize)
                    .unwrap()
                    .scaled_add(1, &self.reference);
                self.tuning_up_to_date[note as usize] = true;
                Some(true)
            } else {
                Some(false)
            }
        } else {
            None {}
        }
    }

    pub fn mark_tuning_as_outdated(&mut self, note: u8) {
        self.tuning_up_to_date[note as usize] = false;
    }

    pub fn mark_all_tunings_as_outdated(&mut self) {
        self.tuning_up_to_date.iter_mut().for_each(|b| *b = false);
    }

    pub fn tuning_for_note(
        &mut self,
        tunings: &mut [Stack<T>; 128],
        note: u8,
    ) -> (f64, Stack<T>, bool) {
        let updated = self.update_tuning(tunings, note).unwrap_or(false);

        let tuning = tunings[note as usize].clone();
        let semitones = tuning.absolute_semitones(self.tuning_reference.c4_semitones());

        (semitones, tuning, updated)
    }
}
