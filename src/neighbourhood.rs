//! A neighbourhood is a description of the tunings of some notes, relative to
//! a reference note.

use std::collections::BTreeMap;

use serde_derive::{Deserialize, Serialize};

use crate::interval::{
    stack::{ScaledAdd, Stack},
    stacktype::r#trait::{IntervalBasis, PeriodicIntervalBasis, StackCoeff},
};

use serde::ser::SerializeMap;
use serde::Deserializer;
use serde_with::{serde_as, DeserializeAs, MapPreventDuplicates};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum SomeNeighbourhood<T: IntervalBasis> {
    PeriodicComplete(PeriodicComplete<T>),
    PeriodicPartial(PeriodicPartial<T>),
    Partial(Partial<T>),
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum SomeCompleteNeighbourhood<T: IntervalBasis> {
    PeriodicComplete(PeriodicComplete<T>),
}

impl<T: IntervalBasis> From<PeriodicComplete<T>> for SomeCompleteNeighbourhood<T> {
    fn from(pc: PeriodicComplete<T>) -> Self {
        Self::PeriodicComplete(pc)
    }
}

/// Tunings for all notes, described by giving the tunings of all notes in the first "octave-like"
/// period above the reference. The period can be an interval in the sense of
/// [IntervalBasis::try_period_index], but may also be a composite interval described by a [Stack]
#[derive(Debug, PartialEq, Clone)]
pub struct PeriodicComplete<T: IntervalBasis> {
    pub stacks: Vec<Stack<T>>,
    pub period: Stack<T>,
    pub period_index: Option<usize>,
}

impl<T: PeriodicIntervalBasis> PeriodicComplete<T> {
    /// invariants:
    /// - the [Stack::key_distance] of the stack on index `ì` of `stacks` is `i`.
    /// - the length of `stacks` is the [IntervalBasis::period_index] of `T`, if it exists
    pub fn new_periodic(stacks: Vec<Stack<T>>) -> Self {
        Self {
            stacks,
            period: Stack::from_pure_interval(T::period_index(), 1),
            period_index: Some(T::period_index()),
        }
    }
}

/// Like [PeriodicComplete], but some positions in the period may be undefined, i.e. have no tuning
/// associated.
#[derive(Debug, PartialEq, Clone)]
pub struct PeriodicPartial<T: IntervalBasis> {
    pub stacks: Vec<(Stack<T>, bool)>,
    pub period: Stack<T>,
    pub period_index: Option<usize>,
}

impl<T: IntervalBasis> PeriodicPartial<T> {
    pub fn new_from_period_index(period_index: usize) -> Self {
        Self {
            stacks: vec![
                (Stack::new_zero(), false);
                T::intervals()[period_index].key_distance as usize
            ],
            period: Stack::from_pure_interval(period_index, 1),
            period_index: Some(period_index),
        }
    }
}

impl<T: IntervalBasis> PeriodicNeighbourhood<T> for PeriodicPartial<T> {}

impl<T: IntervalBasis> Neighbourhood<T> for PeriodicPartial<T> {
    fn insert(&mut self, stack: &Stack<T>) -> &Stack<T> {
        let n = self.period_keys();
        let quot = stack.key_distance().div_euclid(n);
        let rem = stack.key_distance().rem_euclid(n) as usize;
        self.stacks[rem].0.clone_from(stack);
        self.stacks[rem].0.scaled_add(-quot, &self.period);
        self.stacks[rem].1 = true;
        &self.stacks[rem].0
    }

    fn for_each_stack<F: FnMut(StackCoeff, &Stack<T>) -> ()>(&self, mut f: F) {
        for (i, (stack, valid)) in self.stacks.iter().enumerate() {
            if *valid {
                f(i as StackCoeff, stack)
            }
        }
    }

    fn for_each_stack_failing<E, F: FnMut(StackCoeff, &Stack<T>) -> Result<(), E>>(
        &self,
        mut f: F,
    ) -> Result<(), E> {
        for (i, (stack, valid)) in self.stacks.iter().enumerate() {
            if *valid {
                f(i as StackCoeff, stack)?;
            }
        }
        Ok(())
    }

    fn for_each_stack_mut<F: FnMut(StackCoeff, &mut Stack<T>) -> ()>(&mut self, mut f: F) {
        for (i, (stack, valid)) in self.stacks.iter_mut().enumerate() {
            if *valid {
                f(i as StackCoeff, stack)
            }
        }
    }

    fn has_tuning_for(&self, offset: StackCoeff) -> bool {
        let n = self.period_keys();
        let rem = offset.rem_euclid(n) as usize;
        self.stacks[rem].1
    }

    fn try_write_relative_stack(&self, target: &mut Stack<T>, offset: StackCoeff) -> bool {
        let n = self.period_keys();
        let quot = offset.div_euclid(n);
        let rem = offset.rem_euclid(n) as usize;
        if self.stacks[rem].1 {
            target.clone_from(&self.stacks[rem].0);
            target.scaled_add(quot, &self.period);
            true
        } else {
            false
        }
    }

    fn try_period(&self) -> Option<&Stack<T>> {
        Some(&self.period)
    }

    fn try_period_index(&self) -> Option<usize> {
        self.period_index
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Partial<T: IntervalBasis> {
    pub stacks: BTreeMap<StackCoeff, Stack<T>>,
}

impl<T: IntervalBasis> Partial<T> {
    pub fn new() -> Self {
        Self {
            stacks: BTreeMap::new(),
        }
    }

    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, StackCoeff, Stack<T>> {
        self.stacks.iter()
    }
}

pub trait Neighbourhood<T: IntervalBasis> {
    /// Insert a tuning. If there's already a tuning for the (relative) note described by Stack,
    /// update. Returns a reference to the actually inserted Stack (which may be different in the
    /// case of [PeriodicNeighbourhood]s, where we store the representative in the "octave" above
    /// the reference)
    fn insert(&mut self, stack: &Stack<T>) -> &Stack<T>;

    /// Go through all stacks _that are actually stored_ (for example, in a
    /// [PeriodicNeighbourhood], only at most the entries for one period are stored) in the
    /// neighbourhood, with their offset to the reference.
    fn for_each_stack<F: FnMut(StackCoeff, &Stack<T>) -> ()>(&self, f: F);

    fn for_each_stack_failing<E, F: FnMut(StackCoeff, &Stack<T>) -> Result<(), E>>(
        &self,
        f: F,
    ) -> Result<(), E>;

    /// like [Neighbourhood::for_each_stack], but allows mutation.
    fn for_each_stack_mut<F: FnMut(StackCoeff, &mut Stack<T>) -> ()>(&mut self, f: F);

    /// Does this neighbourhood provide a tuning for a note with the given offset from the
    /// reference?
    ///
    /// Must return `true` for every `offset` in the case of [CompleteNeigbourhood]s.
    fn has_tuning_for(&self, offset: StackCoeff) -> bool;

    /// Like [Neighbourhood::try_get_relative_stack], but with an output argument `target`, which
    /// must remain unchanged if it returns `false`.
    fn try_write_relative_stack(&self, target: &mut Stack<T>, offset: StackCoeff) -> bool;

    /// Return the Stack describing the interval with the given offset. Must return `Some` iff
    /// [Neighbourhood::has_tuning_for] returns true for the same offset.
    fn try_get_relative_stack(&self, offset: StackCoeff) -> Option<Stack<T>> {
        if self.has_tuning_for(offset) {
            let mut res = Stack::new_zero();
            let _ = self.try_write_relative_stack(&mut res, offset);
            Some(res)
        } else {
            None
        }
    }

    /// Must return `Some` for an implementation of [PeriodicNeighbourhood] to be valid.
    fn try_period(&self) -> Option<&Stack<T>>;

    /// If the period returned by [Neighbourhood::try_period] corresponds to a base interval,
    /// return that interval's index in the [IntervalBasis::intervals].
    fn try_period_index(&self) -> Option<usize>;

    /// Returns the lowest and highest entry in the given dimension. The `axis` must be in the
    /// range `0..N`, where `N` is the [IntervalBasis::num_intervals].
    fn bounds(&self, axis: usize) -> (StackCoeff, StackCoeff) {
        let (mut min, mut max) = (0, 0);
        self.for_each_stack(|_, stack| {
            let x = stack.target[axis];
            if x > max {
                max = x
            }
            if x < min {
                min = x
            }
        });
        (min, max)
    }
}

impl<T: IntervalBasis> Neighbourhood<T> for SomeNeighbourhood<T> {
    fn insert(&mut self, stack: &Stack<T>) -> &Stack<T> {
        match self {
            SomeNeighbourhood::PeriodicComplete(x) => x.insert(stack),
            SomeNeighbourhood::PeriodicPartial(x) => x.insert(stack),
            SomeNeighbourhood::Partial(x) => x.insert(stack),
        }
    }

    fn for_each_stack<F: FnMut(StackCoeff, &Stack<T>) -> ()>(&self, f: F) {
        match self {
            SomeNeighbourhood::PeriodicComplete(x) => x.for_each_stack(f),
            SomeNeighbourhood::PeriodicPartial(x) => x.for_each_stack(f),
            SomeNeighbourhood::Partial(x) => x.for_each_stack(f),
        }
    }

    fn for_each_stack_failing<E, F: FnMut(StackCoeff, &Stack<T>) -> Result<(), E>>(
        &self,
        f: F,
    ) -> Result<(), E> {
        match self {
            SomeNeighbourhood::PeriodicComplete(x) => x.for_each_stack_failing(f),
            SomeNeighbourhood::PeriodicPartial(x) => x.for_each_stack_failing(f),
            SomeNeighbourhood::Partial(x) => x.for_each_stack_failing(f),
        }
    }

    fn for_each_stack_mut<F: FnMut(StackCoeff, &mut Stack<T>) -> ()>(&mut self, f: F) {
        match self {
            SomeNeighbourhood::PeriodicComplete(x) => x.for_each_stack_mut(f),
            SomeNeighbourhood::PeriodicPartial(x) => x.for_each_stack_mut(f),
            SomeNeighbourhood::Partial(x) => x.for_each_stack_mut(f),
        }
    }

    fn has_tuning_for(&self, offset: StackCoeff) -> bool {
        match self {
            SomeNeighbourhood::PeriodicComplete(x) => x.has_tuning_for(offset),
            SomeNeighbourhood::PeriodicPartial(x) => x.has_tuning_for(offset),
            SomeNeighbourhood::Partial(x) => x.has_tuning_for(offset),
        }
    }

    fn try_write_relative_stack(&self, target: &mut Stack<T>, offset: StackCoeff) -> bool {
        match self {
            SomeNeighbourhood::PeriodicComplete(x) => x.try_write_relative_stack(target, offset),
            SomeNeighbourhood::PeriodicPartial(x) => x.try_write_relative_stack(target, offset),
            SomeNeighbourhood::Partial(x) => x.try_write_relative_stack(target, offset),
        }
    }

    fn try_period(&self) -> Option<&Stack<T>> {
        match self {
            SomeNeighbourhood::PeriodicComplete(x) => x.try_period(),
            SomeNeighbourhood::PeriodicPartial(x) => x.try_period(),
            SomeNeighbourhood::Partial(x) => x.try_period(),
        }
    }

    fn try_period_index(&self) -> Option<usize> {
        match self {
            SomeNeighbourhood::PeriodicComplete(x) => x.try_period_index(),
            SomeNeighbourhood::PeriodicPartial(x) => x.try_period_index(),
            SomeNeighbourhood::Partial(x) => x.try_period_index(),
        }
    }
}

impl<T: IntervalBasis> Neighbourhood<T> for SomeCompleteNeighbourhood<T> {
    fn insert(&mut self, stack: &Stack<T>) -> &Stack<T> {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => n.insert(stack),
        }
    }

    fn for_each_stack<F: FnMut(StackCoeff, &Stack<T>) -> ()>(&self, f: F) {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => n.for_each_stack(f),
        }
    }

    fn for_each_stack_failing<E, F: FnMut(StackCoeff, &Stack<T>) -> Result<(), E>>(
        &self,
        f: F,
    ) -> Result<(), E> {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => n.for_each_stack_failing(f),
        }
    }

    fn for_each_stack_mut<F: FnMut(StackCoeff, &mut Stack<T>) -> ()>(&mut self, f: F) {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => n.for_each_stack_mut(f),
        }
    }

    fn has_tuning_for(&self, offset: StackCoeff) -> bool {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => n.has_tuning_for(offset),
        }
    }

    fn try_write_relative_stack(&self, target: &mut Stack<T>, offset: StackCoeff) -> bool {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => {
                n.try_write_relative_stack(target, offset)
            }
        }
    }

    fn try_period(&self) -> Option<&Stack<T>> {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => n.try_period(),
        }
    }

    fn try_period_index(&self) -> Option<usize> {
        match self {
            SomeCompleteNeighbourhood::PeriodicComplete(n) => n.try_period_index(),
        }
    }
}

impl<T: IntervalBasis> CompleteNeigbourhood<T> for SomeCompleteNeighbourhood<T> {}

impl<T: IntervalBasis> Neighbourhood<T> for Partial<T> {
    fn insert(&mut self, stack: &Stack<T>) -> &Stack<T> {
        let offset = stack.key_distance();
        if let Some(old_entry) = self.stacks.get_mut(&offset) {
            old_entry.clone_from(stack);
        } else {
            self.stacks.insert(offset, stack.clone());
        }
        self.stacks.get(&offset).unwrap()
    }

    fn for_each_stack<F: FnMut(StackCoeff, &Stack<T>) -> ()>(&self, mut f: F) {
        for (i, stack) in self.stacks.iter() {
            f(*i, stack);
        }
    }

    fn for_each_stack_failing<E, F: FnMut(StackCoeff, &Stack<T>) -> Result<(), E>>(
        &self,
        mut f: F,
    ) -> Result<(), E> {
        for (i, stack) in self.stacks.iter() {
            f(*i, stack)?;
        }
        Ok(())
    }

    fn for_each_stack_mut<F: FnMut(StackCoeff, &mut Stack<T>) -> ()>(&mut self, mut f: F) {
        for (i, stack) in self.stacks.iter_mut() {
            f(*i, stack);
        }
    }

    fn has_tuning_for(&self, offset: StackCoeff) -> bool {
        self.stacks.contains_key(&offset)
    }

    fn try_write_relative_stack(&self, target: &mut Stack<T>, offset: StackCoeff) -> bool {
        if let Some(stack) = self.stacks.get(&offset) {
            target.clone_from(stack);
            true
        } else {
            false
        }
    }

    fn try_period(&self) -> Option<&Stack<T>> {
        None {}
    }

    fn try_period_index(&self) -> Option<usize> {
        None {}
    }
}

impl<T: IntervalBasis> Neighbourhood<T> for PeriodicComplete<T> {
    fn insert(&mut self, stack: &Stack<T>) -> &Stack<T> {
        let n = self.period_keys();
        let quot = stack.key_distance().div_euclid(n);
        let rem = stack.key_distance().rem_euclid(n) as usize;
        self.stacks[rem].clone_from(stack);
        self.stacks[rem].scaled_add(-quot, &self.period);
        &self.stacks[rem]
    }

    fn for_each_stack<F: FnMut(StackCoeff, &Stack<T>) -> ()>(&self, mut f: F) {
        for (i, stack) in self.stacks.iter().enumerate() {
            f(i as StackCoeff, stack)
        }
    }

    fn for_each_stack_failing<E, F: FnMut(StackCoeff, &Stack<T>) -> Result<(), E>>(
        &self,
        mut f: F,
    ) -> Result<(), E> {
        for (i, stack) in self.stacks.iter().enumerate() {
            f(i as StackCoeff, stack)?;
        }
        Ok(())
    }

    fn for_each_stack_mut<F: FnMut(StackCoeff, &mut Stack<T>) -> ()>(&mut self, mut f: F) {
        for (i, stack) in self.stacks.iter_mut().enumerate() {
            f(i as StackCoeff, stack)
        }
    }

    fn has_tuning_for(&self, _: StackCoeff) -> bool {
        true
    }

    fn try_write_relative_stack(&self, target: &mut Stack<T>, offset: StackCoeff) -> bool {
        let n = self.period_keys();
        let quot = offset.div_euclid(n);
        let rem = offset.rem_euclid(n) as usize;
        target.clone_from(&self.stacks[rem]);
        target.scaled_add(quot, &self.period);
        true
    }

    fn try_period(&self) -> Option<&Stack<T>> {
        Some(&self.period)
    }

    fn try_period_index(&self) -> Option<usize> {
        self.period_index
    }
}

/// Marker trait of neighbourhoods that can return a note for every offset.
pub trait CompleteNeigbourhood<T: IntervalBasis>: Neighbourhood<T> {
    fn write_relative_stack(&self, target: &mut Stack<T>, offset: StackCoeff) {
        self.try_write_relative_stack(target, offset);
    }

    fn get_relative_stack(&self, offset: StackCoeff) -> Stack<T> {
        self.try_get_relative_stack(offset).expect(
            "This should never happen: CompleteNeigbourhood doesn't have a tuning for an offset!",
        )
    }
}

impl<T: IntervalBasis> CompleteNeigbourhood<T> for PeriodicComplete<T> {}

pub trait PeriodicNeighbourhood<T: IntervalBasis>: Neighbourhood<T> {
    /// The "octave": keys will be tuned relative to the highest note that can be obtained by
    /// shifting the reference a number (negative, zero, or positive) of these periods.
    fn period(&self) -> &Stack<T> {
        self.try_period()
            .expect("This should never happen: PeriodicNeighbourhood doesn't have a period")
    }

    /// Convenience: the [key_distance][Stack::key_distance] of the period.
    fn period_keys(&self) -> StackCoeff {
        self.period().key_distance()
    }
}

impl<T: IntervalBasis> PeriodicNeighbourhood<T> for PeriodicComplete<T> {}

fn serialize_as_map<T, N, S>(neigh: &N, ser: S) -> Result<S::Ok, S::Error>
where
    T: IntervalBasis + serde::Serialize,
    N: Neighbourhood<T>,
    S: serde::Serializer,
{
    let mut t = ser.serialize_map(T::try_period_keys().map(|x| x as usize))?;
    neigh.for_each_stack_failing(|offset, stack| t.serialize_entry(&offset, stack))?;
    t.end()
}

impl<T: IntervalBasis + serde::Serialize> serde::Serialize for PeriodicComplete<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_as_map(self, serializer)
    }
}

impl<T: IntervalBasis + serde::Serialize> serde::Serialize for PeriodicPartial<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_as_map(self, serializer)
    }
}

impl<T: IntervalBasis + serde::Serialize> serde::Serialize for Partial<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_as_map(self, serializer)
    }
}

/// needed for the DeserializeAs magic. Otherwise, serde_as will try to deserialize map keys as
/// Strings...
struct AStackCoeff<'de>(&'de str);

impl<'de> serde::Deserialize<'de> for AStackCoeff<'de> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let i = <_ as serde::Deserialize<'de>>::deserialize(deserializer)?;
        Ok(AStackCoeff(i))
    }
}

impl<'de> DeserializeAs<'de, StackCoeff> for AStackCoeff<'de> {
    fn deserialize_as<D>(deserializer: D) -> Result<StackCoeff, D::Error>
    where
        D: Deserializer<'de>,
    {
        let AStackCoeff(str) = <AStackCoeff as serde::Deserialize<'de>>::deserialize(deserializer)?;
        match str.parse() {
            Ok(i) => Ok(i),
            Err(e) => Err(serde::de::Error::custom(e)),
        }
    }
}

#[serde_as]
#[derive(Deserialize)]
struct NoDuplicates<T: IntervalBasis> {
    #[serde(flatten)]
    #[serde_as(as = "MapPreventDuplicates<AStackCoeff, _>")]
    stacks: BTreeMap<StackCoeff, Stack<T>>,
}

impl<'de, T: IntervalBasis + serde::Deserialize<'de>> serde::Deserialize<'de>
    for PeriodicComplete<T>
{
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut map = BTreeMap::<StackCoeff, Stack<T>>::deserialize(deserializer)?;

        let n = T::try_period_keys().ok_or_else(|| {
            serde::de::Error::custom(
                "cannot deserialize PeriodicComplete neighbourhood for non-periodic IntervalBasis",
            )
        })?;

        if map.len() != n as usize {
            return Err(serde::de::Error::custom(format!(
                "expected {n} entries, but got {}",
                map.len()
            )));
        }

        if let Some((&lo, _)) = map.first_key_value() {
            if lo != 0 {
                return Err(serde::de::Error::custom(format!(
                    "the lowest entry must be 0, but it is {lo}",
                )));
            }
        }

        if let Some((&hi, _)) = map.last_key_value() {
            if hi != n as StackCoeff - 1 {
                return Err(serde::de::Error::custom(format!(
                    "the highest entry must be {}, but it is {hi}",
                    n as StackCoeff - 1
                )));
            }
        }

        if let Some((offset, stack)) = map.iter().find(|(i, stack)| stack.key_distance() != **i) {
            return Err(serde::de::Error::custom(format!(
                "the stack for entry {offset} describes an interval spanning {} keys",
                stack.key_distance()
            )));
        }

        let mut stacks = Vec::with_capacity(n as usize);

        while let Some((_, stack)) = map.pop_first() {
            stacks.push(stack);
        }

        Ok(PeriodicComplete {
            stacks,
            period: Stack::from_pure_interval(T::try_period_index().unwrap(), 1),
            period_index: T::try_period_index(),
        })
    }
}

impl<'de, T: IntervalBasis + serde::Deserialize<'de>> serde::Deserialize<'de>
    for PeriodicPartial<T>
{
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if let Some(n) = T::try_period_keys() {
            let NoDuplicates { mut stacks } = <_>::deserialize(deserializer)?;

            if stacks.len() > n as usize {
                return Err(serde::de::Error::custom(format!(
                    "expected at most {n} entries, but got {}",
                    stacks.len()
                )));
            }

            if let Some((&lo, _)) = stacks.first_key_value() {
                if lo < 0 {
                    return Err(serde::de::Error::custom(format!(
                        "the lowest entry must be at least 0, but it is {lo}",
                    )));
                }
            }
            if let Some((&hi, _)) = stacks.last_key_value() {
                if hi >= n as StackCoeff {
                    return Err(serde::de::Error::custom(format!(
                        "the highest entry must be at most {}, but it is {hi}",
                        n as StackCoeff - 1,
                    )));
                }
            }

            if let Some((offset, stack)) =
                stacks.iter().find(|(i, stack)| stack.key_distance() != **i)
            {
                return Err(serde::de::Error::custom(format!(
                    "the stack for entry {offset} describes an interval spanning {} keys",
                    stack.key_distance()
                )));
            }

            Ok(PeriodicPartial {
                stacks: {
                    let mut v = vec![(Stack::new_zero(), false); n as usize];
                    while let Some((i, stack)) = stacks.pop_first() {
                        v[i as usize] = (stack, true);
                    }
                    v
                },
                // unwrap is ok, because `T::try_period_keys()` was `Some`
                period: Stack::from_pure_interval(T::try_period_index().unwrap(), 1),
                period_index: T::try_period_index(),
            })
        } else {
            panic!(
                "cannot deserialize PeriodicPartial neighbourhood for non-periodic IntervalBasis"
            );
        }
    }
}

impl<'de, T: IntervalBasis + serde::Deserialize<'de>> serde::Deserialize<'de> for Partial<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let NoDuplicates { stacks } = <_>::deserialize(deserializer)?;

        if let Some((offset, stack)) = stacks.iter().find(|(i, stack)| stack.key_distance() != **i)
        {
            return Err(serde::de::Error::custom(format!(
                "the stack for entry {offset} describes an interval spanning {} keys",
                stack.key_distance()
            )));
        }

        Ok(Partial { stacks })
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        interval::stacktype::fivelimit::mock::MockFiveLimitStackType,
        neighbourhood::SomeNeighbourhood,
    };

    #[test]
    fn test_serialize_neighbourhood() {
        let neigh = SomeNeighbourhood::PeriodicComplete(PeriodicComplete::new_periodic(vec![
            Stack::<MockFiveLimitStackType>::new_zero(), // C
            Stack::from_target(vec![0, -1, 2]),          // C#
            Stack::from_target(vec![-1, 2, 0]),          // D
            Stack::from_target(vec![0, 1, -1]),          // Eb
            Stack::from_target(vec![0, 0, 1]),           // E
            Stack::from_target(vec![1, -1, 0]),          // F
            Stack::from_target(vec![-1, 2, 1]),          // F#
            Stack::from_target(vec![0, 1, 0]),           // G
            Stack::from_target(vec![0, 0, 2]),           // G#
            Stack::from_target(vec![1, -1, 1]),          // A
            Stack::from_target(vec![0, 2, -1]),          // Bb
            Stack::from_target(vec![0, 1, 1]),           // B
        ]));

        assert_eq!(
            serde_yml::to_string(&neigh).unwrap(),
            r#"!periodic-complete
0: {}
1:
  fifth: -1
  third: 2
2:
  octave: -1
  fifth: 2
3:
  fifth: 1
  third: -1
4:
  third: 1
5:
  octave: 1
  fifth: -1
6:
  octave: -1
  fifth: 2
  third: 1
7:
  fifth: 1
8:
  third: 2
9:
  octave: 1
  fifth: -1
  third: 1
10:
  fifth: 2
  third: -1
11:
  fifth: 1
  third: 1
"#
        );
    }

    #[test]
    fn test_deserialize_neighbourhood_errors() {
        let test_error_contains = |input, contained| {
            let res = format!(
                "{:?}",
                serde_yml::from_str::<SomeNeighbourhood<MockFiveLimitStackType>>(input)
            );
            if !res.contains(contained) {
                panic!("the result\n\n{res}\n\ndoesn't contain the expected error message");
            }
        };

        test_error_contains(
            r#"!periodic-complete
0: {}
1:
  fifth: -1
  third: 2
"#,
            "expected 12 entries, but got 2",
        );

        test_error_contains(
            r#"!periodic-partial
        0: {}
        0: {}
        "#,
            "duplicate",
        );

        test_error_contains(
            r#"!periodic-partial
        -7:
          fifth: -1
        "#,
            "the lowest entry must be at least 0, but it is -7",
        );

        test_error_contains(
            r#"!periodic-partial
        3:
          fifth: 1
        "#,
            "the stack for entry 3 describes an interval spanning 7 keys",
        );
    }
}
