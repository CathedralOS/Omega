//! Route discharge for named `Namespace::requirement(...)` operator calls.
//!
//! A named use selects the same crash contract as a spelled use but produces
//! no `FlowOperatorInvocationFact` operand capture, so the operand-time
//! `InvocationContexts` discharge in checks/operators cannot see it. The
//! containing statement still records entry semantic contexts: facts the
//! flow pass proved on arrival at that statement. Those facts describe
//! storage as it stands at statement entry, so they falsify a route guard
//! only while every place a guard leaf reads keeps its statement-entry value
//! through the invocation. `entry_operand` supplies exactly that boundary:
//! its pristine-storage window includes the containing statement, so an
//! earlier sibling effect — a receiver-mutating call, an exclusive borrow, an
//! atomic — already voids a mutable operand's provenance, and immutable
//! bindings cannot be overwritten at all. A leaf occurrence that does not
//! resolve to an entry-proven operand — a `self` field, a constant, an
//! unproven binding — keeps the route, as does a leaf that would evaluate a
//! nested call or atomic afresh rather than re-reading captured storage.

use checked_trees::{CrashPredicateExpression, FlowFacts};
use facts::{FactContextHandle, FactPayload, FactPlace, FactPlan};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::signature::StateParameter;

use crate::labels::{
    canonical_place_label, instantiate_operator_contract_expression_label,
    semantic_boolean_fact_label,
};

/// Whether the selected operator's published route guard is provably false at
/// one named call. Mirrors `checks::operator_route_is_false`'s Boolean
/// polarity structure, substituting the containing statement's entry contexts
/// for the operand-captured invocation contexts a spelled use carries.
#[allow(clippy::too_many_arguments)]
pub(super) fn named_route_is_false(
    program: &TypedTrees,
    flow: &FlowFacts,
    semantic: &FactPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    substitution: &[Option<CrashPredicateExpression>],
    expression: ExpressionHandle,
) -> bool {
    let context_rows =
        statement_entry_context_rows(flow, machine_symbol, state_symbol, statement_index);
    if context_rows.is_empty() {
        return false;
    }
    expression_has_polarity(
        program,
        semantic,
        &context_rows,
        parameters,
        operands,
        substitution,
        expression,
        false,
    )
}

/// The entry semantic contexts of every flow statement row at this source
/// coordinate. A statement reached by several paths may carry several rows;
/// every row must prove a leaf before it is discharged, so a fact live on
/// only one incoming path can never narrow the surviving routes.
fn statement_entry_context_rows(
    flow: &FlowFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
) -> Vec<Vec<FactContextHandle>> {
    flow.control
        .states
        .iter()
        .filter(|(_, state)| {
            state.machine_symbol == machine_symbol && state.state_symbol == state_symbol
        })
        .flat_map(|(_, state)| {
            flow.control
                .statements
                .span_or_empty(state.statements)
                .iter()
        })
        .filter(|statement| statement.statement_index == statement_index)
        .map(|statement| {
            flow.contexts
                .semantic_context_refs
                .span_or_empty(statement.entry_semantic_contexts)
                .iter()
                .map(|reference| reference.context)
                .chain(flow.semantic_constraint_contexts(statement.entry_constraints))
                .collect()
        })
        .collect()
}

fn expression_has_polarity(
    program: &TypedTrees,
    semantic: &FactPlan,
    context_rows: &[Vec<FactContextHandle>],
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    substitution: &[Option<CrashPredicateExpression>],
    expression: ExpressionHandle,
    polarity: bool,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => return *value == polarity,
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            return expression_has_polarity(
                program,
                semantic,
                context_rows,
                parameters,
                operands,
                substitution,
                unary.operand,
                !polarity,
            );
        }
        ExpressionNode::Binary(binary)
            if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
        {
            let left = expression_has_polarity(
                program,
                semantic,
                context_rows,
                parameters,
                operands,
                substitution,
                binary.left,
                polarity,
            );
            let right = expression_has_polarity(
                program,
                semantic,
                context_rows,
                parameters,
                operands,
                substitution,
                binary.right,
                polarity,
            );
            return if (binary.operator == BinaryOperator::And) == polarity {
                left && right
            } else {
                left || right
            };
        }
        _ => {}
    }
    if !leaf_reads_only_entry_operands(program, parameters, substitution, expression)
        || expression_evaluates_effects(program, expression)
    {
        return false;
    }
    let required =
        instantiate_operator_contract_expression_label(program, parameters, operands, expression);
    if polarity {
        return context_rows.iter().all(|row| {
            row.iter()
                .any(|context| context_proves_boolean_label(program, semantic, *context, &required))
        });
    }
    context_rows.iter().all(|row| {
        row.iter().any(|context| {
            semantic
                .context_view(semantic.contexts.get(*context))
                .facts()
                .any(|fact| {
                    matches!(fact.payload, FactPayload::BooleanValue { expression, value: false }
                        if program.expression_table.display_name(expression) == required)
                })
        })
    })
}

/// Every place occurrence the leaf reads must resolve to an operator
/// parameter whose operand carries entry provenance: only then does a
/// statement-entry fact about the operand's storage still describe the value
/// the invocation evaluates. A constant, a `self` field, or a binding whose
/// provenance is unknown fails this gate and retains the guard.
fn leaf_reads_only_entry_operands(
    program: &TypedTrees,
    parameters: &[StateParameter],
    substitution: &[Option<CrashPredicateExpression>],
    expression: ExpressionHandle,
) -> bool {
    let mut occurrences = Vec::new();
    super::super::contract_occurrences::append_expression_occurrences(
        program,
        expression,
        &mut occurrences,
    );
    occurrences.iter().all(|occurrence| {
        let root = occurrence_root_symbol(program, *occurrence);
        parameters.iter().enumerate().any(|(index, parameter)| {
            parameter.symbol == root && substitution.get(index).is_some_and(Option::is_some)
        })
    })
}

/// A leaf proven by a statement-entry fact is re-evaluated at the invocation,
/// so it must be a pure read of the proven storages. A nested call or atomic
/// could observe storage beyond the operand occurrences — state a matching
/// fact snapshot does not transport — and is kept conservative instead.
fn expression_evaluates_effects(program: &TypedTrees, root: ExpressionHandle) -> bool {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression) {
            return true;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Call(_) | ExpressionNode::Atomic(_) => return true,
            ExpressionNode::Match(dispatch) => {
                pending.push(dispatch.subject);
                for arm in program.expression_table.match_arms(dispatch.arms) {
                    if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                    pending.push(arm.value);
                }
            }
            ExpressionNode::Binary(binary) => {
                pending.push(binary.left);
                pending.push(binary.right);
            }
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
            ExpressionNode::Indexed(indexed) => {
                pending.push(indexed.collection);
                pending.push(indexed.index);
            }
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::Range(range) => {
                pending.push(range.start);
                pending.push(range.end);
            }
            ExpressionNode::ArrayLiteral(values) => {
                pending.extend(
                    program
                        .expression_table
                        .expression_handles(*values)
                        .iter()
                        .copied(),
                );
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    pending.push(field.value);
                }
            }
            _ => {}
        }
    }
    false
}

/// Port of the requires prover's occurrence rooting: a place occurrence roots
/// at the symbol its member/index/borrow selectors descend to.
fn occurrence_root_symbol(program: &TypedTrees, mut expression: ExpressionHandle) -> SymbolHandle {
    loop {
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => return path.symbol,
            ExpressionNode::Member(member) => expression = member.receiver,
            ExpressionNode::Indexed(indexed) => expression = indexed.collection,
            ExpressionNode::Borrow(borrow) => expression = borrow.target,
            _ => return SymbolHandle::invalid(),
        }
    }
}

/// One entry context proves the instantiated leaf, mirroring the spelled
/// use's leaf prover: an evaluated Boolean fact with a matching rendered
/// form, a declaration-shaped contract clause, or a domain membership whose
/// `requires` states the clause over the member value.
fn context_proves_boolean_label(
    program: &TypedTrees,
    semantic: &FactPlan,
    context: FactContextHandle,
    required_label: &str,
) -> bool {
    let context = semantic.contexts.get(context);
    semantic
        .context_view(context)
        .facts()
        .any(|fact| match fact.payload {
            FactPayload::BooleanValue {
                expression,
                value: true,
            }
            | FactPayload::BooleanExpression(expression) => {
                true_expression_proves_label(program, expression, required_label)
            }
            FactPayload::ContractBooleanExpression {
                expression,
                instantiated,
                ..
            } => {
                (!instantiated.is_valid()
                    && true_expression_proves_label(program, expression, required_label))
                    || semantic_boolean_fact_label(program, semantic, fact)
                        .is_some_and(|candidate| candidate == required_label)
            }
            FactPayload::DomainMembership {
                domain_symbol,
                value,
                ..
            }
            | FactPayload::ContractDomainMembership {
                domain_symbol,
                value,
                ..
            } => {
                let FactPlace::Place(place) = fact.place else {
                    return false;
                };
                let canonical_base =
                    canonical_place_label(program, semantic, semantic.places.get(place));
                let display_base = program.expression_table.display_name(value);
                crate::checks::contracts::labels::domain_proves_expression_label(
                    program,
                    domain_symbol,
                    &canonical_base,
                    required_label,
                ) || crate::checks::contracts::labels::domain_proves_expression_label(
                    program,
                    domain_symbol,
                    &display_base,
                    required_label,
                )
            }
            _ => false,
        })
}

/// A retained true conjunction supplies each conjunct; a matching rendered
/// form proves the leaf directly. Selected comparisons receive no inferred
/// arithmetic or complement laws here.
fn true_expression_proves_label(
    program: &TypedTrees,
    expression: ExpressionHandle,
    required_label: &str,
) -> bool {
    if program.expression_table.display_name(expression) == required_label {
        return true;
    }
    matches!(program.expression_table.expression(expression),
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And
            && (true_expression_proves_label(program, binary.left, required_label)
                || true_expression_proves_label(program, binary.right, required_label)))
}
