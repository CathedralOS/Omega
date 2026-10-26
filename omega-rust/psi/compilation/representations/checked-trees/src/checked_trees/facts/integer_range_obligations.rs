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
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum RangeStep {
    #[default]
    Missing,
    Bounds {
        low: BigInt,
        high: BigInt,
    },
    Observation {
        expression: ExpressionHandle,
        owner: SymbolHandle,
    },
    Add,
    Subtract,
    Multiply,
    Union,
    Interval,
    Intersect,
}

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
            steps: vec![RangeStep::Observation { expression, owner }],
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegerRangeObligation {
    expression: ExpressionHandle,
    owner: SymbolHandle,
    steps: Arena<RangeStep>,
    recipe: HandleSpan<RangeStep>,
    minimum: BigInt,
    maximum: BigInt,
    description: String,
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
        let mut steps = Arena::new();
        let recipe = steps.insert_many(recipe.steps);
        Self {
            expression,
            owner,
            steps,
            recipe,
            minimum,
            maximum,
            description,
            discharged_target: None,
        }
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

    pub fn discharge(&mut self, program: &TypedTrees, target: &str) -> Result<(), Diagnostic> {
        let result = self
            .steps
            .span(self.recipe)
            .ok_or_else(|| "retained integer range has no recipe storage".to_owned())
            .and_then(|steps| evaluate(steps, program))
            .and_then(|(low, high)| {
                if self.minimum <= low && high <= self.maximum {
                    Ok(())
                } else {
                    Err(format!(
                        "bounds [{low}, {high}] lie outside [{}, {}]",
                        self.minimum, self.maximum
                    ))
                }
            });
        match result {
            Ok(()) => {
                self.discharged_target = Some(typed_trees::name::Identifier::generated(target));
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

fn evaluate(steps: &[RangeStep], program: &TypedTrees) -> Result<(BigInt, BigInt), String> {
    let mut values: Vec<(BigInt, BigInt)> = Vec::new();
    for step in steps {
        match step {
            RangeStep::Missing => return Err("missing integer bound step".to_owned()),
            RangeStep::Bounds { low, high } => {
                if low > high {
                    return Err("inverted captured integer bounds".to_owned());
                }
                values.push((low.clone(), high.clone()));
            }
            RangeStep::Observation { expression, owner } => {
                let value = program
                    .closed_integer_expression_value_in(*expression, *owner)
                    .ok_or_else(|| "target observation has no closed integer value".to_owned())?;
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
                    RangeStep::Add => (left_low.add(&right_low), left_high.add(&right_high)),
                    RangeStep::Subtract => (left_low.sub(&right_high), left_high.sub(&right_low)),
                    RangeStep::Interval => {
                        if left_low > right_high {
                            return Err("inverted contract bounds".to_owned());
                        }
                        (left_low, right_high)
                    }
                    RangeStep::Intersect => {
                        let low = left_low.max(right_low);
                        let high = left_high.min(right_high);
                        if low > high {
                            return Err("inconsistent captured contract bounds".to_owned());
                        }
                        (low, high)
                    }
                    RangeStep::Multiply => {
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
                    RangeStep::Union => (left_low.min(right_low), left_high.max(right_high)),
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
