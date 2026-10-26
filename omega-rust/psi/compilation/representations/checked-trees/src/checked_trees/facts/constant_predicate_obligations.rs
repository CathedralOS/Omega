//! Constant proof predicates awaiting their selected-target observations.
//!
//! A portable check cannot prove a target's layout by observing the host. It
//! can retain a ground predicate whose only unknown leaves are explicit layout
//! requests. This is conditional evidence, not a successful proof: target
//! closure must discharge every predicate before producing Terminal Psi.
//! Runtime places and calls cannot enter this route. The canonical expression
//! keeps the checked operator meaning; closure supplies values, not new syntax
//! or a second source-checking pass. The ordinary scalar predicate evaluator
//! is shared with the source prover so Boolean semantics cannot drift.

use super::integer_range_obligations::RetainedIntegerRange;
use super::scalar_predicates::{self, ScalarValue};
use arena::Arena;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};

#[derive(Debug, Clone, PartialEq, Eq)]
enum PredicateValue {
    Authored,
    Captured(Arena<CapturedComparison>),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum CapturedComparison {
    #[default]
    Missing,
    Integer {
        operator: BinaryOperator,
        left: RetainedIntegerRange,
        right: RetainedIntegerRange,
    },
}

impl CapturedComparison {
    fn closed_value(&self, program: &TypedTrees) -> Option<bool> {
        let Self::Integer {
            operator,
            left,
            right,
        } = self
        else {
            return None;
        };
        let (left_low, left_high) = left.closed_bounds(program).ok()?;
        let (right_low, right_high) = right.closed_bounds(program).ok()?;
        match operator {
            BinaryOperator::Equal if left_low == left_high && right_low == right_high => {
                Some(left_low == right_low)
            }
            BinaryOperator::NotEqual if left_high < right_low || right_high < left_low => {
                Some(true)
            }
            BinaryOperator::Less if left_high < right_low => Some(true),
            BinaryOperator::LessOrEqual if left_high <= right_low => Some(true),
            BinaryOperator::Greater if left_low > right_high => Some(true),
            BinaryOperator::GreaterOrEqual if left_low >= right_high => Some(true),
            _ if left_low == left_high && right_low == right_high => Some(false),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstantPredicateObligation {
    expression: ExpressionHandle,
    description: String,
    owner: symbols::SymbolHandle,
    caller: symbols::SymbolHandle,
    observation_dependencies: Vec<(ExpressionHandle, symbols::SymbolHandle)>,
    constant_dependencies: Vec<typed_trees::constant_recipes::ConstantRecipeHandle>,
    value: PredicateValue,
    discharged_target: Option<typed_trees::name::Identifier>,
}

impl ConstantPredicateObligation {
    /// Retain a wholly constant predicate that needs target closure. A runtime
    /// unknown is not a deferred constant, even beside a layout observation.
    pub fn retain(
        program: &TypedTrees,
        expression: ExpressionHandle,
        owner: symbols::SymbolHandle,
        caller: symbols::SymbolHandle,
        description: String,
        builtin: &impl Fn(ExpressionHandle) -> bool,
    ) -> Option<Self> {
        let mut pending = vec![expression];
        let mut visited = Vec::new();
        let mut observation_dependencies = Vec::new();
        while let Some(current) = pending.pop() {
            if visited.contains(&current) {
                continue;
            }
            if !program.expression_table.expression_is_valid(current) {
                return None;
            }
            visited.push(current);
            match program.expression_table.expression(current) {
                ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => {}
                ExpressionNode::Call(call) if call.layout_query.is_some() => {
                    observation_dependencies.push((current, owner));
                }
                ExpressionNode::Name(path) => {
                    let declaration = program.const_declaration_by_symbol(path.symbol)?;
                    pending.push(declaration.materialized_initializer);
                }
                ExpressionNode::Binary(binary) => {
                    // Contract expressions may have no executable operator
                    // row. Only the producer's exact-owner resolution grants
                    // builtin meaning; absence of a row is not authority.
                    if !builtin(current) {
                        return None;
                    }
                    pending.extend([binary.left, binary.right]);
                }
                ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                    pending.push(unary.operand);
                }
                ExpressionNode::Cast(cast) if !cast.form.is_recast() => {
                    pending.push(cast.value);
                }
                _ => return None,
            }
        }
        (!observation_dependencies.is_empty()).then_some(Self {
            expression,
            description,
            owner,
            caller,
            observation_dependencies,
            constant_dependencies: Vec::new(),
            value: PredicateValue::Authored,
            discharged_target: None,
        })
    }

    /// Selected arithmetic meaning and exact captured actuals supply this
    /// conjunction. These are conditional obligations, never caller hypotheses.
    pub fn retain_captured(
        expression: ExpressionHandle,
        owner: symbols::SymbolHandle,
        caller: symbols::SymbolHandle,
        description: String,
        comparisons: Vec<(
            BinaryOperator,
            super::IntegerRangeRecipe,
            super::IntegerRangeRecipe,
        )>,
    ) -> Option<Self> {
        if comparisons.is_empty()
            || !comparisons
                .iter()
                .any(|(_, left, right)| left.is_target_dependent() || right.is_target_dependent())
        {
            return None;
        }
        let mut captured = Arena::new();
        let mut observations = Vec::new();
        let mut constants = Vec::new();
        for (operator, left, right) in comparisons {
            if !matches!(
                operator,
                BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
            ) {
                return None;
            }
            // Each observation retains its own meaning owner below; authored
            // predicate leaves use the predicate's owner through the old lane.
            observations.extend(left.observation_dependencies());
            observations.extend(right.observation_dependencies());
            constants.extend(left.constant_dependencies());
            constants.extend(right.constant_dependencies());
            captured.insert(CapturedComparison::Integer {
                operator,
                left: left.retain(),
                right: right.retain(),
            });
        }
        Some(Self {
            expression,
            owner,
            caller,
            description,
            observation_dependencies: observations,
            constant_dependencies: constants,
            value: PredicateValue::Captured(captured),
            discharged_target: None,
        })
    }

    pub fn is_pending(&self) -> bool {
        self.discharged_target.is_none()
    }

    /// The application scope belongs to the caller, not the callee whose
    /// declaration supplies operator meaning. One portable helper can be
    /// called from several differently scoped bodies.
    pub fn caller(&self) -> symbols::SymbolHandle {
        self.caller
    }

    /// Original query leaves, including those reached through constant names.
    /// Their meaning owner is independent of the caller's application scope.
    pub fn observation_dependencies(
        &self,
    ) -> impl Iterator<Item = (ExpressionHandle, symbols::SymbolHandle)> + '_ {
        self.observation_dependencies.iter().copied()
    }

    pub fn constant_dependencies(
        &self,
    ) -> impl Iterator<Item = typed_trees::constant_recipes::ConstantRecipeHandle> + '_ {
        self.constant_dependencies.iter().copied()
    }

    /// Consume the target's already closed observations. Unknown and false
    /// both refuse; the diagnostic distinguishes them from a proven predicate.
    pub fn discharge_all<'obligation>(
        obligations: impl IntoIterator<Item = &'obligation mut Self>,
        program: &TypedTrees,
        target: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        let obligations: Vec<_> = obligations.into_iter().collect();
        let diagnostics: Vec<_> = obligations
            .iter()
            .filter_map(|obligation| {
                let result = match &obligation.value {
                    PredicateValue::Authored => closed_value(
                    program,
                    obligation.expression,
                    obligation.owner,
                    &mut Vec::new(),
                    ),
                    PredicateValue::Captured(comparisons) => {
                        let mut result = Some(true);
                        for (_, comparison) in comparisons.iter() {
                            match comparison.closed_value(program) {
                                Some(false) => { result = Some(false); break; }
                                None => result = None,
                                Some(true) => {},
                            }
                        }
                        result.map(ScalarValue::Boolean)
                    }
                };
                if matches!(result, Some(ScalarValue::Boolean(true))) {
                    return None;
                }
                Some(
                    Diagnostic::error(format!(
                        "{}: constant predicate {} under the selected target realization `{target}`",
                        obligation.description,
                        if matches!(result, Some(ScalarValue::Boolean(false))) {
                            "is false"
                        } else {
                            "has no closed Boolean value"
                        },
                    ))
                    .with_source_span(program.expression_table.source_span(obligation.expression)),
                )
            })
            .collect();
        if diagnostics.is_empty() {
            for obligation in obligations {
                obligation.discharged_target =
                    Some(typed_trees::name::Identifier::generated(target));
            }
            Ok(())
        } else {
            Err(diagnostics)
        }
    }
}

fn closed_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    owner: symbols::SymbolHandle,
    active: &mut Vec<ExpressionHandle>,
) -> Option<ScalarValue> {
    if active.contains(&expression) {
        return None;
    }
    active.push(expression);
    let result = scalar_predicates::evaluate(
        program,
        expression,
        &mut |leaf| {
            if let ExpressionNode::Name(path) = program.expression_table.expression(leaf) {
                let declaration = program.const_declaration_by_symbol(path.symbol)?;
                return closed_value(program, declaration.materialized_initializer, owner, active);
            }
            program
                .closed_integer_expression_value_in(leaf, owner)
                .map(ScalarValue::Integer)
        },
        &|_, _| true,
    );
    active.pop();
    result
}
