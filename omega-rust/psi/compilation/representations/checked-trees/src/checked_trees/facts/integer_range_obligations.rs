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
use std::sync::{Arc, OnceLock};
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
#[derive(Clone)]
pub struct IntegerRangeRecipe {
    steps: Vec<RangeStep>,
    target_dependent: bool,
    /// Immutable recipes are shared by completed observations. Preparing their
    /// ordinary description once avoids serializing the same proof payload for
    /// every resolver fingerprint. This is presentation, never value identity.
    prepared_debug: OnceLock<Arc<str>>,
}

impl PartialEq for IntegerRangeRecipe {
    fn eq(&self, other: &Self) -> bool {
        self.steps == other.steps && self.target_dependent == other.target_dependent
    }
}

impl Eq for IntegerRangeRecipe {}

struct UnpreparedRecipeDebug<'recipe>(&'recipe IntegerRangeRecipe);

impl std::fmt::Debug for UnpreparedRecipeDebug<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IntegerRangeRecipe")
            .field("steps", &self.0.steps)
            .field("target_dependent", &self.0.target_dependent)
            .finish()
    }
}

impl std::fmt::Debug for IntegerRangeRecipe {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if formatter.alternate() {
            // Pretty descriptions inherit the caller's indentation rather
            // than reusing bytes prepared for ordinary fingerprint printing.
            return std::fmt::Debug::fmt(&UnpreparedRecipeDebug(self), formatter);
        }
        let description = self
            .prepared_debug
            .get_or_init(|| Arc::from(format!("{:?}", UnpreparedRecipeDebug(self))));
        formatter.write_str(description)
    }
}

impl IntegerRangeRecipe {
    pub fn bounds(low: BigInt, high: BigInt) -> Self {
        Self {
            steps: vec![RangeStep::Bounds { low, high }],
            target_dependent: false,
            prepared_debug: OnceLock::new(),
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
            prepared_debug: OnceLock::new(),
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
            prepared_debug: OnceLock::new(),
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

#[cfg(test)]
mod prepared_debug_tests {
    use super::{IntegerRangeRecipe as Recipe, RangeStep};
    use numerics::bignum::BigInt;
    use std::hash::{DefaultHasher, Hasher};
    use typed_trees::expression::BinaryOperator;

    // This is the original derived presentation, without preparation storage.
    #[derive(Debug)]
    #[allow(dead_code)]
    struct IntegerRangeRecipe<'recipe> {
        steps: &'recipe [RangeStep],
        target_dependent: bool,
    }

    fn fingerprint(value: &impl std::fmt::Debug) -> u64 {
        struct Writer(DefaultHasher);
        impl std::fmt::Write for Writer {
            fn write_str(&mut self, value: &str) -> std::fmt::Result {
                self.0.write(value.as_bytes());
                Ok(())
            }
        }
        let mut writer = Writer(DefaultHasher::new());
        std::fmt::write(&mut writer, format_args!("{value:?}")).unwrap();
        writer.0.finish()
    }

    #[test]
    fn prepared_debug_preserves_bytes_fingerprints_and_value_identity() {
        let recipe = Recipe::bounds(BigInt::from_i64(-3), BigInt::from_i64(7))
            .binary(
                BinaryOperator::Add,
                &Recipe::observation(
                    typed_trees::expression::ExpressionHandle::from_arena_index(5),
                    symbols::SymbolHandle::from_arena_index(9),
                ),
            )
            .unwrap();
        let cold_clone = recipe.clone();
        let original = IntegerRangeRecipe {
            steps: &recipe.steps,
            target_dependent: recipe.target_dependent,
        };
        assert_eq!(format!("{recipe:#?}"), format!("{original:#?}"));
        assert!(recipe.prepared_debug.get().is_none());
        let expected = fingerprint(&original);
        assert_eq!(fingerprint(&recipe), expected);
        assert_eq!(format!("{recipe:?}"), format!("{original:?}"));
        assert_eq!(fingerprint(&recipe), expected);
        assert_eq!(recipe, cold_clone);
        assert_eq!(fingerprint(&cold_clone), expected);
        let warm_clone = recipe.clone();
        assert!(std::sync::Arc::ptr_eq(
            recipe.prepared_debug.get().unwrap(),
            warm_clone.prepared_debug.get().unwrap(),
        ));
        assert_eq!(fingerprint(&warm_clone), expected);
        assert_eq!(format!("{warm_clone:#?}"), format!("{original:#?}"));
        assert_eq!(recipe, warm_clone);
    }

    #[test]
    fn prepared_debug_composition_prepares_its_new_payload_independently() {
        let left = Recipe::bounds(BigInt::from_i64(2), BigInt::from_i64(4));
        let before = fingerprint(&left);
        let right = Recipe::bounds(BigInt::from_i64(1), BigInt::from_i64(1));
        let composed = left.binary(BinaryOperator::Add, &right).unwrap();
        assert!(composed.prepared_debug.get().is_none());
        let original = IntegerRangeRecipe {
            steps: &composed.steps,
            target_dependent: composed.target_dependent,
        };
        assert_eq!(fingerprint(&composed), fingerprint(&original));
        assert_ne!(fingerprint(&composed), before);
        assert_ne!(composed, left);
        assert_eq!(fingerprint(&left), before);
    }
}
