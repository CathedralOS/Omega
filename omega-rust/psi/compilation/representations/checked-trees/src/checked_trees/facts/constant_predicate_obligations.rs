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

use super::scalar_predicates::{self, ScalarValue};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, UnaryOperator};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstantPredicateObligation {
    expression: ExpressionHandle,
    description: String,
    owner: symbols::SymbolHandle,
    caller: symbols::SymbolHandle,
    observation_dependencies: Vec<ExpressionHandle>,
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
                    observation_dependencies.push(current);
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
        self.observation_dependencies
            .iter()
            .copied()
            .map(move |expression| (expression, self.owner))
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
                let result = closed_value(
                    program,
                    obligation.expression,
                    obligation.owner,
                    &mut Vec::new(),
                );
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
