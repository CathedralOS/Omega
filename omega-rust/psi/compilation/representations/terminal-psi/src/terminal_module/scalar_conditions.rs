//! Conditional integer judgments owned by the portable program.
//!
//! A retained condition is a debt, never a proof. Its target-independent bound
//! expression names exact applications or an exact result of a complete scalar
//! entry. Target admission supplies those observations and the address model;
//! absence, an unexecuted occurrence, or a final return value cannot substitute.

use arena::{Arena, HandleSpan};
use numerics::bignum::BigInt;
use numerics::integer_bounds::IntegerBoundStep;
use semantic_vocabulary::{MachineId, ScalarApplicationId, ScalarType, StructuralTypeId, ValueId};

use super::ProofDeclarationId;

/// The declaration use which demands this condition. Module-wide requirements
/// are explicit; failed owner lookup must never become module-wide ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarConditionSubject {
    Module,
    Declaration(ProofDeclarationId),
    StructuralType(StructuralTypeId),
    Application(ScalarApplicationId),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ScalarRangeObservation {
    #[default]
    Missing,
    Literal(BigInt),
    Application(ScalarApplicationId),
    /// The value belongs to the application's root activation, not an arbitrary
    /// recursive invocation of the same body. Repeated occurrences cannot use
    /// last-writer-wins; admission must retain the exact occurrence or refuse.
    EntryResult {
        application: ScalarApplicationId,
        machine: MachineId,
        result: ValueId,
        scalar_type: ScalarType,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalarRangeRequirement {
    Fixed { minimum: BigInt, maximum: BigInt },
    AddressModel,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ScalarRangeCondition {
    #[default]
    Missing,
    Range {
        subject: ScalarConditionSubject,
        requirement: ScalarRangeRequirement,
        steps: HandleSpan<IntegerBoundStep<ScalarRangeObservation>>,
    },
}

/// One module-owned arena for condition rows and their captured bounds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScalarConditions {
    ranges: Arena<ScalarRangeCondition>,
    steps: Arena<IntegerBoundStep<ScalarRangeObservation>>,
}

impl ScalarConditions {
    pub fn ranges(&self) -> &[ScalarRangeCondition] {
        self.ranges.storage_slice()
    }

    /// References retained by all conditions, including values with no
    /// surviving executable use. Optimizers must preserve these identities.
    pub fn observations(&self) -> impl Iterator<Item = &ScalarRangeObservation> {
        self.steps
            .storage_slice()
            .iter()
            .filter_map(|step| match step {
                IntegerBoundStep::Observation(observation) => Some(observation),
                _ => None,
            })
    }

    pub fn steps(
        &self,
        condition: &ScalarRangeCondition,
    ) -> Result<&[IntegerBoundStep<ScalarRangeObservation>], &'static str> {
        let ScalarRangeCondition::Range { steps, .. } = condition else {
            return Err("scalar condition is absent");
        };
        self.steps
            .span(*steps)
            .ok_or("scalar condition lost its captured bounds")
    }

    /// Retain a formed bound expression without establishing its truth.
    pub fn retain_range(
        &mut self,
        subject: ScalarConditionSubject,
        requirement: ScalarRangeRequirement,
        steps: Vec<IntegerBoundStep<ScalarRangeObservation>>,
    ) -> Result<(), &'static str> {
        if let ScalarRangeRequirement::Fixed { minimum, maximum } = &requirement
            && minimum > maximum
        {
            return Err("scalar condition has inverted required bounds");
        }
        let mut depth = 0usize;
        for step in &steps {
            match step {
                IntegerBoundStep::Missing => {
                    return Err("scalar condition contains an absent step");
                }
                IntegerBoundStep::Bounds { low, high } => {
                    if low > high {
                        return Err("scalar condition has inverted captured bounds");
                    }
                    depth += 1;
                }
                IntegerBoundStep::Observation(ScalarRangeObservation::Missing) => {
                    return Err("scalar condition contains an absent observation");
                }
                IntegerBoundStep::Observation(_) => depth += 1,
                IntegerBoundStep::Add
                | IntegerBoundStep::Subtract
                | IntegerBoundStep::Multiply
                | IntegerBoundStep::Union
                | IntegerBoundStep::Interval
                | IntegerBoundStep::Intersect => {
                    if depth < 2 {
                        return Err("scalar condition has missing bound operands");
                    }
                    depth -= 1;
                }
            }
        }
        if depth != 1 {
            return Err("scalar condition does not produce one bound interval");
        }
        let steps = self.steps.insert_many(steps);
        self.ranges.insert(ScalarRangeCondition::Range {
            subject,
            requirement,
            steps,
        });
        Ok(())
    }
}
