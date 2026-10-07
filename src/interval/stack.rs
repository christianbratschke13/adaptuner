use std::{marker::PhantomData, ops};

use ndarray::{Array1, ArrayView1};
use num_rational::Ratio;
use num_traits::Zero;

use crate::interval::{
    base::Semitones,
    stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
};

use std::fmt;

use serde::ser::SerializeMap;
use serde::{de::Visitor, ser::SerializeStruct, Serializer};
use serde_derive::Deserialize;

struct NamedCoefficientsView<'a, T: IntervalBasis, C> {
    coeffs: ArrayView1<'a, C>,
    _phantom: PhantomData<T>,
}

impl<'a, T: IntervalBasis, C> NamedCoefficientsView<'a, T, C> {
    fn new(inner: ArrayView1<'a, C>) -> Self {
        Self {
            coeffs: inner,
            _phantom: PhantomData,
        }
    }
}

trait DeSerNumber: Sized {
    type Rep;

    fn to_rep(&self) -> Self::Rep;
    fn from_rep(rep: &Self::Rep) -> Option<Self>;
}

impl DeSerNumber for StackCoeff {
    type Rep = StackCoeff;

    fn to_rep(&self) -> Self::Rep {
        *self
    }

    fn from_rep(rep: &Self::Rep) -> Option<Self> {
        Some(*rep)
    }
}

impl DeSerNumber for Ratio<StackCoeff> {
    type Rep = String;

    fn to_rep(&self) -> Self::Rep {
        self.to_string()
    }

    fn from_rep(rep: &Self::Rep) -> Option<Self> {
        rep.parse().ok()
    }
}

impl<'a, T, C> serde::Serialize for NamedCoefficientsView<'a, T, C>
where
    T: IntervalBasis,
    C: Zero + DeSerNumber,
    C::Rep: serde::Serialize,
{
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        let n = self.coeffs.iter().filter(|x| !x.is_zero()).count();
        let mut t = ser.serialize_map(Some(n))?;

        for (i, c) in self.coeffs.iter().enumerate() {
            if !c.is_zero() {
                t.serialize_entry(&T::intervals()[i].name, &c.to_rep())?;
            }
        }

        t.end()
    }
}

struct NamedCoefficients<T: IntervalBasis, C> {
    coeffs: Array1<C>,
    _phantom: PhantomData<T>,
}

impl<'de, T, C> serde::Deserialize<'de> for NamedCoefficients<T, C>
where
    T: IntervalBasis,
    C: 'static + Clone + DeSerNumber + Zero,
    C::Rep: serde::Deserialize<'de> + fmt::Display,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct NamedCoefficientsVisitor<T: IntervalBasis, C> {
            _phantom: PhantomData<(T, C)>,
        }

        impl<'de, T, C> Visitor<'de> for NamedCoefficientsVisitor<T, C>
        where
            T: IntervalBasis,
            C: 'static + Clone + DeSerNumber + Zero,
            C::Rep: serde::Deserialize<'de> + fmt::Display,
        {
            type Value = NamedCoefficients<T, C>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "map of interval names")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut res = Array1::zeros(T::num_intervals());
                let mut set = Array1::from_elem(T::num_intervals(), false);

                while let Some((key, value)) = map.next_entry::<String, C::Rep>()? {
                    match T::interval_positions().get(&key) {
                        None => {
                            return Err(serde::de::Error::custom(format!(
                                "'{}' is not an interval name",
                                key
                            )));
                        }
                        Some(i) => match C::from_rep(&value) {
                            None => {
                                return Err(serde::de::Error::custom(format!(
                                    "'{}' is not a well-formed number",
                                    value
                                )));
                            }
                            Some(c) => {
                                if set[*i] {
                                    return Err(serde::de::Error::custom(format!(
                                        "duplicate definition for '{}'",
                                        key
                                    )));
                                }

                                res[*i] = c;
                                set[*i] = true;
                            }
                        },
                    }
                }

                Ok(Self::Value {
                    coeffs: res,
                    _phantom: PhantomData,
                })
            }
        }

        deserializer.deserialize_map(NamedCoefficientsVisitor {
            _phantom: PhantomData,
        })
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct Stack<T: IntervalBasis> {
    _phantom: PhantomData<T>,
    pub target: Array1<StackCoeff>,
    pub actual: Array1<Ratio<StackCoeff>>,
}

impl<T: IntervalBasis> serde::Serialize for Stack<T> {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        if self.is_target() {
            NamedCoefficientsView::<T, _>::new(self.target.view()).serialize(ser)
        } else {
            let mut t = ser.serialize_struct("stack", 2)?;
            t.serialize_field(
                "target",
                &NamedCoefficientsView::<T, _>::new(self.target.view()),
            )?;
            t.serialize_field(
                "actual",
                &NamedCoefficientsView::<T, _>::new(self.actual.view()),
            )?;
            t.end()
        }
    }
}

impl<'de, T: IntervalBasis> serde::Deserialize<'de> for Stack<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct StackVisitor<T: IntervalBasis> {
            _phantom: PhantomData<T>,
        }

        impl<T: IntervalBasis> StackVisitor<T> {
            fn new() -> Self {
                Self {
                    _phantom: PhantomData,
                }
            }
        }

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum StackField {
            Target,
            Actual,
            // #[serde(, flatten)]
            IntervalName(String),
        }

        impl<'de, T> Visitor<'de> for StackVisitor<T>
        where
            T: IntervalBasis,
        {
            type Value = Stack<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "stack")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut target = None {};
                let mut actual = None {};
                let mut raw_target: Array1<StackCoeff> = Array1::zeros(T::num_intervals());
                let mut raw_set = Array1::from_elem(T::num_intervals(), false);
                let mut is_raw = false;
                while let Some(key) = map.next_key()? {
                    match key {
                        StackField::Target => {
                            if target.is_some() {
                                return Err(serde::de::Error::duplicate_field("target"));
                            }
                            if is_raw {
                                return Err(serde::de::Error::custom(
                                    "don't specify 'target' on the same level as interval names",
                                ));
                            }
                            let NamedCoefficients { coeffs, .. } =
                                map.next_value::<NamedCoefficients<T, StackCoeff>>()?;
                            target = Some(coeffs);
                        }
                        StackField::Actual => {
                            if actual.is_some() {
                                return Err(serde::de::Error::duplicate_field("actual"));
                            }
                            if is_raw {
                                return Err(serde::de::Error::custom(
                                    "don't specify 'actual' on the same level as interval names",
                                ));
                            }
                            let NamedCoefficients { coeffs: inner, .. } =
                                map.next_value::<NamedCoefficients<T, Ratio<StackCoeff>>>()?;
                            actual = Some(inner);
                        }
                        StackField::IntervalName(name) => {
                            if actual.is_some() {
                                return Err(serde::de::Error::custom(format!("'actual' alreay specified, don't specify interval name '{name}' on the same level")));
                            }
                            if target.is_some() {
                                return Err(serde::de::Error::custom(format!("'target' alreay specified, don't specify interval name '{name}' on the same level")));
                            }

                            match T::interval_positions().get(&name) {
                                None {} => {
                                    return Err(serde::de::Error::custom(format!(
                                        "'{}' is not an interval name",
                                        name
                                    )))
                                }
                                Some(i) => {
                                    if raw_set[*i] {
                                        return Err(serde::de::Error::custom(format!(
                                            "duplicate definition for '{}'",
                                            name
                                        )));
                                    }
                                    raw_set[*i] = true;
                                    let c = map.next_value::<StackCoeff>()?;
                                    is_raw = true;
                                    raw_target[*i] = c;
                                }
                            }
                        }
                    }
                }

                // if is_raw is already true, then there's no change. Otherwise, the map was
                // empty, and then the zeros in raw_target are accurate.
                if target.is_none() & actual.is_none() {
                    is_raw = true;
                }

                if is_raw {
                    Ok(Stack::from_target(raw_target))
                } else {
                    if target.is_none() {
                        return Err(serde::de::Error::missing_field("target"));
                    }
                    Ok(match actual {
                        None {} => Stack::from_target(target.unwrap()),
                        Some(actual) => Stack::from_target_and_actual(target.unwrap(), actual),
                    })
                }
            }
        }

        deserializer.deserialize_struct("stack", &["target", "actual"], StackVisitor::new())
    }
}

pub trait ScaledAdd<S> {
    fn scaled_add<P: ops::Deref<Target = Self>>(&mut self, scalar: S, other: P);
}

impl<T: IntervalBasis> ScaledAdd<StackCoeff> for Stack<T> {
    fn scaled_add<P: ops::Deref<Target = Stack<T>>>(&mut self, scalar: StackCoeff, other: P) {
        self.target.scaled_add(scalar, &other.target);
        self.actual
            .scaled_add(Ratio::from_integer(scalar), &other.actual);
    }
}

/// Like [Stack::key_distance], but for cases when you only have the target coefficients and
/// not a whole [Stack].
pub fn key_distance_from_coefficients<T: IntervalBasis>(
    target: ArrayView1<StackCoeff>,
) -> StackCoeff {
    let mut res = 0;
    for (i, &c) in target.iter().enumerate() {
        res += T::intervals()[i].key_distance as StackCoeff * c;
    }
    res
}

/// Like [Stack::target_semitones], but for cases when you only have the target coefficients and
/// not a whole [Stack].
pub fn semitones_from_target<T: IntervalBasis>(target: ArrayView1<StackCoeff>) -> Semitones {
    let mut res = 0.0;
    for (i, &c) in target.iter().enumerate() {
        res += T::intervals()[i].semitones * c as Semitones;
    }
    res
}

/// Like [Stack::semitones], but for cases when you only have what would be the [Stack::actual]
/// coefficients and not a whole [Stack].
pub fn semitones_from_actual<T: IntervalBasis>(actual: ArrayView1<Ratio<StackCoeff>>) -> Semitones {
    let mut res = 0.0;
    for (i, &c) in actual.iter().enumerate() {
        let (n, d) = c.into_raw();
        res += T::intervals()[i].semitones * n as Semitones / d as Semitones;
    }
    res
}

impl<T: IntervalBasis> Stack<T> {
    pub fn from_target_and_actual(
        target: Array1<StackCoeff>,
        actual: Array1<Ratio<StackCoeff>>,
    ) -> Self {
        Stack {
            _phantom: PhantomData,
            target,
            actual,
        }
    }

    /// actual will be initialised to the same as target
    pub fn from_target<V: Into<Array1<StackCoeff>>>(target: V) -> Self {
        let target = target.into();
        let actual = Array1::from_shape_fn(target.len(), |i| Ratio::from_integer(target[i]));
        Stack {
            _phantom: PhantomData,
            target: target.into(),
            actual,
        }
    }

    pub fn raw(self) -> (Array1<StackCoeff>, Array1<Ratio<StackCoeff>>) {
        (self.target, self.actual)
    }

    pub fn new_zero() -> Self {
        Stack {
            _phantom: PhantomData,
            target: Array1::zeros(T::num_intervals()),
            actual: Array1::zeros(T::num_intervals()),
        }
    }

    pub fn from_pure_interval(interval_index: usize, multiplier: StackCoeff) -> Self {
        let mut target = Array1::zeros(T::num_intervals());
        target[interval_index] = multiplier;
        let mut actual = Array1::zeros(T::num_intervals());
        actual[interval_index] = Ratio::from_integer(multiplier);
        Stack {
            _phantom: PhantomData,
            target,
            actual,
        }
    }

    pub fn target_coefficients(&self) -> ArrayView1<StackCoeff> {
        self.target.view()
    }

    pub fn actual_coefficients(&self) -> ArrayView1<Ratio<StackCoeff>> {
        self.actual.view()
    }

    pub fn increment_at_index_pure(&mut self, interval_index: usize, increment: StackCoeff) {
        self.target[interval_index] += increment;
        self.actual[interval_index] += increment;
    }

    pub fn is_target(&self) -> bool {
        for (i, r) in self.actual.iter().enumerate() {
            if !r.is_integer() {
                return false;
            }
            if r.to_integer() != self.target[i] {
                return false;
            }
        }

        true
    }

    /// - `s.is_target()´ implies ´s.is_pure()´, but not vice versa.
    pub fn is_pure(&self) -> bool {
        for r in self.actual.iter() {
            if !r.is_integer() {
                return false;
            }
        }

        true
    }

    /// Size of the interval described, in fractional semitones.
    pub fn semitones(&self) -> Semitones {
        semitones_from_actual::<T>(self.actual.view())
    }

    /// Like [Self::semitones], but for the target note.
    pub fn target_semitones(&self) -> Semitones {
        semitones_from_target::<T>(self.target.view())
    }

    /// If the zero stack corresponds to middle C, return the "fractional MIDI note number"
    /// described by this stack.
    pub fn absolute_semitones(&self, c4_semitones: Semitones) -> Semitones {
        self.semitones() + c4_semitones
    }

    /// Like [Self::absolute_semitones], but for the target note.
    pub fn target_absolute_semitones(&self, c4_semitones: Semitones) -> Semitones {
        self.target_semitones() + c4_semitones
    }

    /// How many fractional semitones higher than the target note is the actual note described by
    /// this stack?
    pub fn semitones_above_target(&self) -> Semitones {
        let mut res = 0.0;
        for (i, &c) in self.target.iter().enumerate() {
            res += T::intervals()[i].semitones * c as Semitones;
        }
        self.semitones() - res
    }

    /// The number of keys on the piano keyboard spanned by the described interval
    pub fn key_distance(&self) -> StackCoeff {
        key_distance_from_coefficients::<T>(self.target.view())
    }

    /// If the zero stack corresponds to middle C, return the MIDI note number of the key that this
    /// stack describes. This uses the [Self::key_distance], so it returns the "enharmonically
    /// correct" key, not the one whose (equally tempered) MIDI note is closest to the actually
    /// sounding note.
    pub fn key_number(&self) -> StackCoeff {
        self.key_distance() + 60
    }

    pub fn reset_to_zero(&mut self) {
        self.target.fill(0);
        self.actual.fill(Ratio::zero());
    }

    pub fn make_pure(&mut self) {
        self.actual.zip_mut_with(&self.target, |l, r| {
            *l = Ratio::from_integer(*r);
        });
    }
}

impl<T: StackType> Stack<T> {
    pub fn from_temperaments_and_target(
        active_temperaments: &[bool],
        coefficients: Vec<StackCoeff>,
    ) -> Self {
        let mut actual =
            Array1::from_shape_fn(coefficients.len(), |i| Ratio::from_integer(coefficients[i]));
        for (t, &active) in active_temperaments.iter().enumerate() {
            if active {
                let temperament = &T::temperaments()[t];
                temperament.add_adjustment(ArrayView1::from(&coefficients), actual.view_mut());
            }
        }
        Stack {
            _phantom: PhantomData,
            target: coefficients.into(),
            actual,
        }
    }

    pub fn increment_at_index(
        &mut self,
        active_temperaments: &[bool],
        interval_index: usize,
        increment: StackCoeff,
    ) {
        self.target[interval_index] += increment;
        self.actual[interval_index] += increment;
        for (t, &active) in active_temperaments.iter().enumerate() {
            if active {
                let temperament = &T::temperaments()[t];
                self.actual.scaled_add(
                    Ratio::from_integer(increment),
                    &temperament.comma(interval_index),
                );
            }
        }
    }

    pub fn retemper(&mut self, active_temperaments: &[bool]) {
        self.actual.zip_mut_with(&self.target, |l, r| {
            *l = Ratio::from_integer(*r);
        });
        for (t, &active) in active_temperaments.iter().enumerate() {
            if active {
                let temperament = &T::temperaments()[t];
                temperament.add_adjustment(self.target.view(), self.actual.view_mut());
            }
        }
    }

    /// clears all other adjustments
    pub fn apply_temperament(&mut self, temperament_index: usize) {
        self.actual.zip_mut_with(&self.target, |l, r| {
            *l = Ratio::from_integer(*r);
        });
        let temperament = &T::temperaments()[temperament_index];
        temperament.add_adjustment(self.target.view(), self.actual.view_mut());
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use approx::assert_relative_eq;

    type MockStackType = crate::interval::stacktype::fivelimit::mock::MockFiveLimitStackType;

    use crate::interval::stacktype::fivelimit::mock::MockFiveLimitStackType;
    use ndarray::arr1;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_semitones() {
        let fifth = 12.0 * (3.0 / 2.0 as Semitones).log2();
        let third = 12.0 * (5.0 / 4.0 as Semitones).log2();

        let quarter_comma_down = 12.0 * (80.0 / 81.0 as Semitones).log2() / 4.0;
        let edo12_third_error = 4.0 - third;
        let edo12_fifth_error = 7.0 - fifth;

        let eps = 0.00000000001; // just an arbitrary small number

        // all four combinations of temperaments for a single third:
        let s =
            Stack::<MockStackType>::from_temperaments_and_target(&[false, false], vec![0, 0, 1]);
        assert_relative_eq!(s.semitones(), third, max_relative = eps);
        assert_relative_eq!(s.semitones_above_target(), 0.0, max_relative = eps);

        let s = Stack::<MockStackType>::from_temperaments_and_target(&[false, true], vec![0, 0, 1]);
        assert_relative_eq!(s.semitones(), third, max_relative = eps);
        assert_relative_eq!(s.semitones_above_target(), 0.0, max_relative = eps);

        let s = Stack::<MockStackType>::from_temperaments_and_target(&[true, false], vec![0, 0, 1]);
        assert_relative_eq!(s.semitones(), 4.0, max_relative = eps);
        assert_relative_eq!(
            s.semitones_above_target(),
            edo12_third_error,
            max_relative = eps
        );

        let s = Stack::<MockStackType>::from_temperaments_and_target(&[true, true], vec![0, 0, 1]);
        assert_relative_eq!(s.semitones(), 4.0, max_relative = eps);
        assert_relative_eq!(
            s.semitones_above_target(),
            edo12_third_error,
            max_relative = eps
        );

        // all four combinations of temperaments for a single fifth:
        let s =
            Stack::<MockStackType>::from_temperaments_and_target(&[false, false], vec![0, 1, 0]);
        assert_relative_eq!(s.semitones(), fifth, max_relative = eps);
        assert_relative_eq!(s.semitones_above_target(), 0.0, max_relative = eps);

        let s = Stack::<MockStackType>::from_temperaments_and_target(&[false, true], vec![0, 1, 0]);
        assert_relative_eq!(
            s.semitones(),
            fifth + quarter_comma_down,
            max_relative = eps
        );
        assert_relative_eq!(
            s.semitones_above_target(),
            quarter_comma_down,
            max_relative = eps
        );

        let s = Stack::<MockStackType>::from_temperaments_and_target(&[true, false], vec![0, 1, 0]);
        assert_relative_eq!(s.semitones(), 7.0, max_relative = eps);
        assert_relative_eq!(
            s.semitones_above_target(),
            edo12_fifth_error,
            max_relative = eps
        );

        let s = Stack::<MockStackType>::from_temperaments_and_target(&[true, true], vec![0, 1, 0]);
        assert_relative_eq!(
            s.semitones(),
            fifth + quarter_comma_down + edo12_fifth_error,
            max_relative = eps
        );
        assert_relative_eq!(
            s.semitones_above_target(),
            edo12_fifth_error + quarter_comma_down,
            max_relative = eps
        );
    }

    #[test]
    fn test_rollovers() {
        let octave = 12.0;
        //let fifth = 12.0 * (3.0 / 2.0 as Semitones).log2();
        let third = 12.0 * (5.0 / 4.0 as Semitones).log2();

        let quarter_comma_down = 12.0 * (80.0 / 81.0 as Semitones).log2() / 4.0;
        let edo12_third_error = 4.0 - third;

        let eps = 0.00000000001; // just an arbitrary small number

        let s = Stack::<MockStackType>::from_temperaments_and_target(&[false, true], vec![0, 4, 0]);
        assert_relative_eq!(s.semitones(), 2.0 * octave + third, max_relative = eps);
        assert_relative_eq!(
            s.semitones_above_target(),
            4.0 * quarter_comma_down,
            max_relative = eps
        );
        assert!(s.is_pure());
        assert!(!s.is_target());

        let s = Stack::<MockStackType>::from_temperaments_and_target(&[true, false], vec![0, 0, 4]);
        assert_relative_eq!(
            s.semitones(),
            octave + third + edo12_third_error,
            max_relative = eps
        );
        assert_relative_eq!(
            s.semitones_above_target(),
            4.0 * edo12_third_error,
            max_relative = eps
        );
        assert!(!s.is_pure());
        assert!(!s.is_target());
    }

    #[test]
    fn test_serialize_stack() {
        assert_eq!(
            serde_yml::to_string(&Stack::<MockFiveLimitStackType>::new_zero()).unwrap(),
            r#"{}
"#
        );

        assert_eq!(
            serde_yml::to_string(&Stack::<MockFiveLimitStackType>::from_target(vec![
                1, 2, -3
            ]))
            .unwrap(),
            r#"octave: 1
fifth: 2
third: -3
"#
        );

        assert_eq!(
            serde_yml::to_string(&Stack::<MockFiveLimitStackType>::from_target(vec![1, 0, 0]))
                .unwrap(),
            r#"octave: 1
"#
        );

        assert_eq!(
            serde_yml::to_string(&Stack::<MockFiveLimitStackType>::from_target_and_actual(
                arr1(&[1, 2, 3]),
                arr1(&[Ratio::new(1, 2), Ratio::new(-2, 3), Ratio::new(4, 5)])
            ))
            .unwrap(),
            r#"target:
  octave: 1
  fifth: 2
  third: 3
actual:
  octave: '1/2'
  fifth: '-2/3'
  third: '4/5'
"#
        );
    }

    #[test]
    fn test_deserialize_stack() {
        assert_eq!(
            serde_yml::from_str::<Stack<MockFiveLimitStackType>>("target: {}").unwrap(),
            Stack::new_zero(),
        );

        assert_eq!(
            serde_yml::from_str::<Stack<MockFiveLimitStackType>>("{}").unwrap(),
            Stack::new_zero(),
        );

        assert_eq!(
            serde_yml::from_str::<Stack<MockFiveLimitStackType>>("").unwrap(),
            Stack::new_zero(),
        );

        assert_eq!(
            serde_yml::from_str::<Stack<MockFiveLimitStackType>>(
                r#"octave: 1
fifth: 2
third: -3
"#
            )
            .unwrap(),
            Stack::from_target(vec![1, 2, -3]),
        );

        assert_eq!(
            serde_yml::from_str::<Stack<MockFiveLimitStackType>>(
                r#"octave: 1
"#
            )
            .unwrap(),
            Stack::from_target(vec![1, 0, 0]),
        );

        assert_eq!(
            serde_yml::from_str::<Stack<MockFiveLimitStackType>>(
                r#"target:
  octave: 1
actual:
  third: 333333333/4
"#
            )
            .unwrap(),
            Stack::from_target_and_actual(
                arr1(&[1, 0, 0]),
                arr1(&[0.into(), 0.into(), Ratio::new(333333333, 4)])
            ),
        );
    }

    #[test]
    fn test_deserialize_stack_errors() {
        let test_error_contains = |input, contained| {
            let res = format!(
                "{:?}",
                serde_yml::from_str::<Stack<MockFiveLimitStackType>>(input)
            );
            if !res.contains(contained) {
                panic!("the result\n\n{res}\n\ndoesn't contain the expected error message");
            }
        };

        test_error_contains(
            r#"target: 
  sadfio: 4
"#,
            "'sadfio' is not an interval name",
        );
        test_error_contains("sadfio: 4", "'sadfio' is not an interval name");

        test_error_contains(
            r#"target: {}
octave: 4
"#,
            "'target' alreay specified, don't specify interval name 'octave' on the same level",
        );

        test_error_contains(
            r#"actual: {}
octave: 4
"#,
            "'actual' alreay specified, don't specify interval name 'octave' on the same level",
        );

        test_error_contains(
            r#"octave: 4
target: irrelevant
"#,
            "don't specify 'target' on the same level as interval names",
        );

        test_error_contains(
            r#"octave: 4
actual: irrelevant
"#,
            "don't specify 'actual' on the same level as interval names",
        );

        test_error_contains(
            r#"target: 
  octave: 4ew1
"#,
            "invalid type",
        );

        test_error_contains(
            r#"octave: 4ew1
"#,
            "invalid type",
        );

        test_error_contains(
            r#"target: 
  octave: 1
  octave: 2
"#,
            "duplicate definition for 'octave'",
        );

        test_error_contains(
            r#"octave: 1
octave: 2
"#,
            "duplicate definition for 'octave'",
        );
    }

    #[test]
    fn stack_serde_yml_roundtrip() {
        let stacks = [
            Stack::<MockFiveLimitStackType>::new_zero(),
            Stack::from_temperaments_and_target(&[true, false], vec![-1, 2, 3]),
            Stack::from_temperaments_and_target(&[false, true], vec![4, -5, 6]),
            Stack::from_temperaments_and_target(&[true, true], vec![7, 8, -9]),
        ];

        for stack in stacks {
            assert_eq!(
                stack,
                serde_yml::from_str(&serde_yml::to_string(&stack).unwrap()).unwrap()
            );
        }
    }
}
