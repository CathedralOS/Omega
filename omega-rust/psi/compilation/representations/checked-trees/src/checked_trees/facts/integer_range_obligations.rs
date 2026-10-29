//! Target-dependent bounds of evaluated integer values, not source replay.
//!
//! Checking captures a value before its storage changes and composes its
//! mathematical bounds with the selected builtin operation. The temporary
//! postfix builder travels with ordinary flow facts. Publication moves those
//! steps into an arena; closure reads only admitted observations and these
//! captured bounds. It never looks up a mutable variable's initializer.
//! Every potentially overflowing child has its own obligation, so a later
//! cancellation cannot erase an earlier formation failure.

use arena::{Arena, HandleSpan};
use diagnostics::Diagnostic;
use numerics::bignum::BigInt;
use numerics::integer_bounds::{self, IntegerBoundStep};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};

#[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
struct SourceObservation {
    expression: ExpressionHandle,
    owner: SymbolHandle,
}

type RangeStep = IntegerBoundStep<SourceObservation>;

/// Invocation-owned construction storage. Equal snapshots compare by value,
/// not allocation identity, so flow fixed points do not manufacture changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegerRangeRecipe {
    steps: Vec<RangeStep>,
    target_dependent: bool,
}

impl IntegerRangeRecipe {
    pub fn bounds(low: BigInt, high: BigInt) -> Self {
        Self {
            steps: vec![RangeStep::Bounds { low, high }],
            target_dependent: false,
        }
    }

    /// The producer has admitted this observation's semantic identity. This
    /// is not permission to defer arbitrary calls or runtime place reads.
    pub fn observation(expression: ExpressionHandle, owner: SymbolHandle) -> Self {
        Self {
            steps: vec![RangeStep::Observation(SourceObservation {
                expression,
                owner,
            })],
            target_dependent: true,
        }
    }

    pub fn is_target_dependent(&self) -> bool {
        self.target_dependent
    }

    pub fn binary(&self, operator: BinaryOperator, right: &Self) -> Option<Self> {
        let step = match operator {
            BinaryOperator::Add => RangeStep::Add,
            BinaryOperator::Subtract => RangeStep::Subtract,
            BinaryOperator::Multiply => RangeStep::Multiply,
            _ => return None,
        };
        Some(self.compose(right, step))
    }

    /// Bounds established by a checked source guarantee, not a runtime read.
    pub fn interval(lower: &Self, upper: &Self) -> Self {
        lower.compose(upper, RangeStep::Interval)
    }

    pub fn intersect(&self, right: &Self) -> Self {
        self.compose(right, RangeStep::Intersect)
    }

    pub fn union(&self, right: &Self) -> Self {
        if self == right {
            return self.clone();
        }
        self.compose(right, RangeStep::Union)
    }

    fn compose(&self, right: &Self, step: RangeStep) -> Self {
        let mut steps = Vec::with_capacity(self.steps.len() + right.steps.len() + 1);
        steps.extend_from_slice(&self.steps);
        steps.extend_from_slice(&right.steps);
        steps.push(step);
        Self {
            steps,
            target_dependent: self.target_dependent || right.target_dependent,
        }
    }
}

/// The endpoint meaning owned by the formation being retained. A selected
/// address model is not a fixed integer ceiling known during shared checking.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
enum RangeRequirement {
    Fixed { minimum: BigInt, maximum: BigInt },
    AddressModel,
}

/// Exact model dependency consumed while closing one address formation.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct IntegerRangeModel {
    pub carrier: semantic_vocabulary::IntegerType,
    pub semantics_version: u32,
    pub commitment: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct IntegerRangeObligation {
    expression: ExpressionHandle,
    owner: SymbolHandle,
    steps: Arena<RangeStep>,
    recipe: HandleSpan<RangeStep>,
    requirement: RangeRequirement,
    description: String,
    discharged_address_model: Option<IntegerRangeModel>,
    discharged_target: Option<typed_trees::name::Identifier>,
}

impl IntegerRangeObligation {
    pub fn retain(
        expression: ExpressionHandle,
        owner: SymbolHandle,
        recipe: IntegerRangeRecipe,
        minimum: BigInt,
        maximum: BigInt,
        description: String,
    ) -> Self {
        Self::with_requirement(
            expression,
            owner,
            recipe,
            RangeRequirement::Fixed { minimum, maximum },
            description,
        )
    }

    fn with_requirement(
        expression: ExpressionHandle,
        owner: SymbolHandle,
        recipe: IntegerRangeRecipe,
        requirement: RangeRequirement,
        description: String,
    ) -> Self {
        let mut steps = Arena::new();
        let recipe = steps.insert_many(recipe.steps);
        Self {
            expression,
            owner,
            steps,
            recipe,
            requirement,
            description,
            discharged_address_model: None,
            discharged_target: None,
        }
    }

    /// Capture a closed anonymous value at its original formation occurrence.
    /// The producer has checked integrality, selected meaning and every normal
    /// path. This row is conditional; it grants no carrier membership before
    /// the selected target closes it.
    pub fn retain_address_fit(
        expression: ExpressionHandle,
        owner: SymbolHandle,
        low: BigInt,
        high: BigInt,
        description: String,
    ) -> Self {
        Self::with_requirement(
            expression,
            owner,
            IntegerRangeRecipe::bounds(low, high),
            RangeRequirement::AddressModel,
            description,
        )
    }

    pub fn needs_address_model(&self) -> bool {
        matches!(self.requirement, RangeRequirement::AddressModel)
    }

    pub fn discharged_address_model(&self) -> Option<&IntegerRangeModel> {
        self.discharged_address_model.as_ref()
    }

    pub fn owner(&self) -> SymbolHandle {
        self.owner
    }
    pub fn expression(&self) -> ExpressionHandle {
        self.expression
    }
    pub fn is_pending(&self) -> bool {
        self.discharged_target.is_none()
    }

    /// Translate the captured bound expression to a consumer-owned namespace.
    /// A consumer must join every exact observation; it cannot rebuild the
    /// expression from the current source or silently omit an unavailable leaf.
    pub fn map_steps<Observation>(
        &self,
        observe: &mut impl FnMut(ExpressionHandle, SymbolHandle) -> Result<Observation, String>,
    ) -> Result<Vec<IntegerBoundStep<Observation>>, String> {
        self.steps
            .span(self.recipe)
            .ok_or_else(|| "retained integer range has no recipe storage".to_owned())?
            .iter()
            .map(|step| {
                step.map_observation(&mut |observation| {
                    observe(observation.expression, observation.owner)
                })
            })
            .collect()
    }

    /// Fixed representability bounds; an address-model requirement instead
    /// obtains its ceiling from the admitted target model.
    pub fn fixed_limits(&self) -> Option<(&BigInt, &BigInt)> {
        match &self.requirement {
            RangeRequirement::Fixed { minimum, maximum } => Some((minimum, maximum)),
            RangeRequirement::AddressModel => None,
        }
    }

    pub fn discharge(
        &mut self,
        program: &TypedTrees,
        target: &str,
        address_model: Option<&IntegerRangeModel>,
    ) -> Result<(), Diagnostic> {
        let limits = match &self.requirement {
            RangeRequirement::Fixed { minimum, maximum } => {
                Some((minimum.clone(), maximum.clone()))
            }
            RangeRequirement::AddressModel => address_model
                .filter(|model| model.carrier.is_address())
                .map(|model| {
                    (
                        BigInt::zero(),
                        BigInt::from_u64(1)
                            .shl_bits(usize::from(model.carrier.bits()))
                            .sub(&BigInt::from_u64(1)),
                    )
                }),
        };
        let result = self
            .steps
            .span(self.recipe)
            .ok_or_else(|| "retained integer range has no recipe storage".to_owned())
            .and_then(|steps| {
                integer_bounds::evaluate(steps, &mut |observation| {
                    program
                        .closed_integer_expression_value_in(
                            observation.expression,
                            observation.owner,
                        )
                        .ok_or_else(|| "target observation has no closed integer value".to_owned())
                })
            })
            .and_then(|(low, high)| {
                let (minimum, maximum) = limits
                    .as_ref()
                    .ok_or_else(|| "selected address model is not closed".to_owned())?;
                if minimum <= &low && &high <= maximum {
                    Ok(())
                } else {
                    Err(format!(
                        "bounds [{low}, {high}] lie outside [{minimum}, {maximum}]"
                    ))
                }
            });
        match result {
            Ok(()) => {
                self.discharged_target = Some(typed_trees::name::Identifier::generated(target));
                if self.needs_address_model() {
                    self.discharged_address_model = address_model.cloned();
                }
                Ok(())
            }
            Err(reason) => Err(Diagnostic::error(format!(
                "{}: exact arithmetic may overflow under target `{target}`: {reason}",
                self.description,
            ))
            .with_source_span(program.expression_table.source_span(self.expression))),
        }
    }
}
