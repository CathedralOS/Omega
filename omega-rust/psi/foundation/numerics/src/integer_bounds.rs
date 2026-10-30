//! Captured integer bounds with caller-owned observation identities.
//!
//! This is interval arithmetic over already-selected operations, not expression
//! execution. Source checking and source-free admission use the same operations;
//! each caller must supply the exact observations justified in its own context.

use crate::bignum::BigInt;

/// Postfix steps. Consumers choose observation identities and own their storage.
#[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
pub enum IntegerBoundStep<Observation> {
    #[default]
    Missing,
    Bounds {
        low: BigInt,
        high: BigInt,
    },
    Observation(Observation),
    Add,
    Subtract,
    Multiply,
    Union,
    Interval,
    Intersect,
}

impl<Observation> IntegerBoundStep<Observation> {
    /// Change only the observation namespace. Captured bounds and selected
    /// operations retain their meaning at the representation boundary.
    pub fn map_observation<Mapped, Error>(
        &self,
        observe: &mut impl FnMut(&Observation) -> Result<Mapped, Error>,
    ) -> Result<IntegerBoundStep<Mapped>, Error> {
        Ok(match self {
            Self::Missing => IntegerBoundStep::Missing,
            Self::Bounds { low, high } => IntegerBoundStep::Bounds {
                low: low.clone(),
                high: high.clone(),
            },
            Self::Observation(observation) => IntegerBoundStep::Observation(observe(observation)?),
            Self::Add => IntegerBoundStep::Add,
            Self::Subtract => IntegerBoundStep::Subtract,
            Self::Multiply => IntegerBoundStep::Multiply,
            Self::Union => IntegerBoundStep::Union,
            Self::Interval => IntegerBoundStep::Interval,
            Self::Intersect => IntegerBoundStep::Intersect,
        })
    }
}

/// Evaluate captured bounds without guessing missing observations. Each step
/// consumes its own operands; malformed stacks and inconsistent bounds refuse.
pub fn evaluate<Observation>(
    steps: &[IntegerBoundStep<Observation>],
    observe: &mut impl FnMut(&Observation) -> Result<BigInt, String>,
) -> Result<(BigInt, BigInt), String> {
    let mut values: Vec<(BigInt, BigInt)> = Vec::new();
    for step in steps {
        match step {
            IntegerBoundStep::Missing => return Err("missing integer bound step".to_owned()),
            IntegerBoundStep::Bounds { low, high } => {
                if low > high {
                    return Err("inverted captured integer bounds".to_owned());
                }
                values.push((low.clone(), high.clone()));
            }
            IntegerBoundStep::Observation(observation) => {
                let value = observe(observation)?;
                values.push((value.clone(), value));
            }
            operation => {
                let (right_low, right_high) = values
                    .pop()
                    .ok_or_else(|| "missing right integer bounds".to_owned())?;
                let (left_low, left_high) = values
                    .pop()
                    .ok_or_else(|| "missing left integer bounds".to_owned())?;
                values.push(match operation {
                    IntegerBoundStep::Add => (left_low.add(&right_low), left_high.add(&right_high)),
                    IntegerBoundStep::Subtract => {
                        (left_low.sub(&right_high), left_high.sub(&right_low))
                    }
                    IntegerBoundStep::Interval => {
                        if left_low > right_high {
                            return Err("inverted contract bounds".to_owned());
                        }
                        (left_low, right_high)
                    }
                    IntegerBoundStep::Intersect => {
                        let low = left_low.max(right_low);
                        let high = left_high.min(right_high);
                        if low > high {
                            return Err("inconsistent captured contract bounds".to_owned());
                        }
                        (low, high)
                    }
                    IntegerBoundStep::Multiply => {
                        let corners = [
                            left_low.mul(&right_low),
                            left_low.mul(&right_high),
                            left_high.mul(&right_low),
                            left_high.mul(&right_high),
                        ];
                        let low = corners
                            .iter()
                            .min()
                            .cloned()
                            .ok_or_else(|| "missing product corners".to_owned())?;
                        let high = corners
                            .iter()
                            .max()
                            .cloned()
                            .ok_or_else(|| "missing product corners".to_owned())?;
                        (low, high)
                    }
                    IntegerBoundStep::Union => (left_low.min(right_low), left_high.max(right_high)),
                    _ => return Err("invalid integer range operation".to_owned()),
                });
            }
        }
    }
    if values.len() != 1 {
        return Err("integer range recipe does not produce one value".to_owned());
    }
    values
        .pop()
        .ok_or_else(|| "integer range recipe has no result".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{IntegerBoundStep, evaluate};
    use crate::bignum::BigInt;

    fn bounds(low: i64, high: i64) -> IntegerBoundStep<u32> {
        IntegerBoundStep::Bounds {
            low: BigInt::from_i64(low),
            high: BigInt::from_i64(high),
        }
    }

    #[test]
    fn preserves_signed_products_and_intersections() {
        let steps = [
            bounds(-3, 2),
            bounds(-4, 5),
            IntegerBoundStep::Multiply,
            bounds(-10, 11),
            IntegerBoundStep::Intersect,
        ];
        let result = evaluate(&steps, &mut |_| Err("unexpected observation".into()));
        assert_eq!(result, Ok((BigInt::from_i64(-10), BigInt::from_i64(11))));
    }

    #[test]
    fn joins_preserve_every_branch_bound() {
        let steps = [
            bounds(2, 4),
            bounds(10, 12),
            IntegerBoundStep::Union,
            bounds(3, 5),
            IntegerBoundStep::Subtract,
        ];
        let result = evaluate(&steps, &mut |_| Err("unexpected observation".into()));
        assert_eq!(result, Ok((BigInt::from_i64(-3), BigInt::from_i64(9))));
    }

    #[test]
    fn missing_observations_do_not_disappear_at_union() {
        let steps = [
            bounds(7, 7),
            IntegerBoundStep::Observation(42),
            IntegerBoundStep::Union,
        ];
        let result = evaluate(&steps, &mut |observation| {
            Err(format!("missing {observation}"))
        });
        assert_eq!(result, Err("missing 42".into()));
    }

    #[test]
    fn mapping_changes_only_observation_identity() {
        let source = [
            IntegerBoundStep::Observation(7u32),
            bounds(1, 1),
            IntegerBoundStep::Add,
        ];
        let mapped = source
            .iter()
            .map(|step| {
                step.map_observation(&mut |observation| {
                    Ok::<_, String>(u64::from(*observation) + 100)
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .expect("mapped observation");
        let result = evaluate(&mapped, &mut |observation| {
            assert_eq!(*observation, 107);
            Ok(BigInt::from_i64(16))
        });
        assert_eq!(result, Ok((BigInt::from_i64(17), BigInt::from_i64(17))));
    }

    #[test]
    fn malformed_stacks_and_inconsistent_bounds_refuse() {
        for steps in [
            vec![],
            vec![IntegerBoundStep::Add],
            vec![bounds(0, 0), bounds(1, 1)],
            vec![bounds(1, 0)],
            vec![bounds(0, 1), bounds(2, 3), IntegerBoundStep::Intersect],
            vec![bounds(5, 6), bounds(1, 2), IntegerBoundStep::Interval],
        ] {
            assert!(evaluate(&steps, &mut |_| Err("unexpected observation".into())).is_err());
        }
    }
}
