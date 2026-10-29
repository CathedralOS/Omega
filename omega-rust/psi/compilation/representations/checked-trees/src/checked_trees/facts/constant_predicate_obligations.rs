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

/// Invocation-local construction input; only captured value origins may fill it.
pub struct ConstantPredicateArgument {
    pub parameter: symbols::SymbolHandle,
    pub sources: Vec<ConstantPredicateSource>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ConstantPredicateSource {
    pub expression: ExpressionHandle,
    pub owner: symbols::SymbolHandle,
    pub state: symbols::SymbolHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
struct ConstantPredicateBinding {
    parameter: symbols::SymbolHandle,
    sources: arena::HandleSpan<ConstantPredicateSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct ConstantPredicateObligation {
    expression: ExpressionHandle,
    description: String,
    owner: symbols::SymbolHandle,
    caller: symbols::SymbolHandle,
    discharged_target: Option<typed_trees::name::Identifier>,
    bindings: arena::Arena<ConstantPredicateBinding>,
    sources: arena::Arena<ConstantPredicateSource>,
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
        Self::retain_application(
            program,
            ConstantPredicateSource {
                expression,
                owner,
                state: symbols::SymbolHandle::invalid(),
            },
            caller,
            description,
            &[],
            &|_, current| builtin(current),
        )
    }

    /// Substitute only the operand-time origins supplied by the value owner.
    /// Every candidate must be ground; a runtime read beside a layout query
    /// cannot become a deferred constant or a successful call precondition.
    pub fn retain_application(
        program: &TypedTrees,
        root: ConstantPredicateSource,
        caller: symbols::SymbolHandle,
        description: String,
        arguments: &[ConstantPredicateArgument],
        builtin: &impl Fn(ConstantPredicateSource, ExpressionHandle) -> bool,
    ) -> Option<Self> {
        let mut pending = vec![(root, true)];
        let mut visited = Vec::new();
        let mut required = Vec::new();
        let mut needs_closure = false;
        while let Some((source, substitute)) = pending.pop() {
            let current = source.expression;
            if visited.contains(&(source, substitute)) {
                continue;
            }
            if !program.expression_table.expression_is_valid(current) {
                return None;
            }
            visited.push((source, substitute));
            let child = |expression| {
                (
                    ConstantPredicateSource {
                        expression,
                        ..source
                    },
                    substitute,
                )
            };
            match program.expression_table.expression(current) {
                ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => {}
                ExpressionNode::Call(call) if call.layout_query.is_some() => needs_closure = true,
                ExpressionNode::Name(path) => {
                    if substitute
                        && let Some(argument) = arguments
                            .iter()
                            .find(|argument| argument.parameter == path.symbol)
                    {
                        if argument.sources.is_empty() {
                            return None;
                        }
                        if !required.contains(&path.symbol) {
                            required.push(path.symbol);
                        }
                        // Actual sources cannot recursively borrow the callee's
                        // formal environment, even in a recursive invocation.
                        pending.extend(
                            argument
                                .sources
                                .iter()
                                .copied()
                                .map(|source| (source, false)),
                        );
                    } else {
                        let declaration = program.const_declaration_by_symbol(path.symbol)?;
                        pending.push((
                            ConstantPredicateSource {
                                expression: declaration.materialized_initializer,
                                ..source
                            },
                            false,
                        ));
                    }
                }
                ExpressionNode::Binary(binary) => {
                    if !builtin(source, current) {
                        return None;
                    }
                    pending.extend([child(binary.left), child(binary.right)]);
                }
                ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                    pending.push(child(unary.operand))
                }
                ExpressionNode::Cast(cast) if !cast.form.is_recast() => {
                    pending.push(child(cast.value))
                }
                _ => return None,
            }
        }
        if !needs_closure {
            return None;
        }
        let mut bindings = arena::Arena::new();
        let mut sources = arena::Arena::new();
        for parameter in required {
            let argument = arguments
                .iter()
                .find(|argument| argument.parameter == parameter)?;
            bindings.insert(ConstantPredicateBinding {
                parameter,
                sources: sources.insert_many(argument.sources.iter().copied()),
            });
        }
        Some(Self {
            expression: root.expression,
            description,
            owner: root.owner,
            caller,
            discharged_target: None,
            bindings,
            sources,
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

    fn closed_value(&self, program: &TypedTrees) -> Option<ScalarValue> {
        let mut bindings = Vec::new();
        for (_, binding) in self.bindings.iter() {
            let mut values = Vec::new();
            for source in self.sources.span(binding.sources)? {
                let value = closed_value(
                    program,
                    source.expression,
                    source.owner,
                    &[],
                    &mut Vec::new(),
                )?;
                if !values.contains(&value) {
                    values.push(value);
                }
            }
            if values.is_empty() {
                return None;
            }
            bindings.push((binding.parameter, values));
        }
        fn every(
            obligation: &ConstantPredicateObligation,
            program: &TypedTrees,
            bindings: &[(symbols::SymbolHandle, Vec<ScalarValue>)],
            selected: &mut Vec<(symbols::SymbolHandle, ScalarValue)>,
        ) -> Option<ScalarValue> {
            let Some((binding, rest)) = bindings.split_first() else {
                return closed_value(
                    program,
                    obligation.expression,
                    obligation.owner,
                    selected,
                    &mut Vec::new(),
                );
            };
            for value in &binding.1 {
                selected.push((binding.0, value.clone()));
                let result = every(obligation, program, rest, selected);
                selected.pop();
                if !matches!(result, Some(ScalarValue::Boolean(true))) {
                    return result;
                }
            }
            Some(ScalarValue::Boolean(true))
        }
        every(self, program, &bindings, &mut Vec::new())
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
                let result = obligation.closed_value(program);
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
    bindings: &[(symbols::SymbolHandle, ScalarValue)],
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
                if let Some((_, value)) = bindings
                    .iter()
                    .find(|(parameter, _)| *parameter == path.symbol)
                {
                    return Some(value.clone());
                }
                let declaration = program.const_declaration_by_symbol(path.symbol)?;
                return closed_value(
                    program,
                    declaration.materialized_initializer,
                    owner,
                    bindings,
                    active,
                );
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
