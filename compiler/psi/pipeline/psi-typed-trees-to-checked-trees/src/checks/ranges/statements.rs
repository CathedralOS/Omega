mod aliases;
mod transitions;

use self::aliases::{seed_local_alias_facts, seed_subslice_window_facts};
use self::transitions::check_transition_target;
use super::arrays::fixed_array_type_length;
use super::expressions::{expression_indexable_length, expression_integer_value, expression_name};
use super::facts::RangeFacts;
use super::guards::{seed_guard_facts, seed_negated_guard_facts};
use super::indexes::check_expression;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{StatementNode, TransitionGuardNode};

pub(super) fn check_statement(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &mut RangeFacts<'_>,
    statement: &StatementNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => {
            check_expression(
                program,
                machine,
                state,
                facts,
                assignment.target,
                diagnostics,
            );
            check_expression(
                program,
                machine,
                state,
                facts,
                assignment.value,
                diagnostics,
            );
            // The target's prior index upper bound is STALE the moment it is
            // reassigned (it now holds a different value). Drop it so a loosening
            // reassignment (`j = j + 1`) cannot retain an old tighter bound and
            // wrongly prove an access AFTER the mutation. Any new bound is
            // re-derived below from the value (`seed_offset_index_bound`).
            facts.forget_index_upper_bound(
                &program.expression_table.display_name(assignment.target),
            );
            // Collection-relative index/range facts (`i < items.len`,
            // `i <= items.len`) name the scalar value in their second slot.
            // Reassigning that scalar invalidates them too; otherwise a guard
            // or loop-head invariant about the old value could prove an access
            // using the new value.
            facts.forget_index_position_facts(
                &program.expression_table.display_name(assignment.target),
            );
            // The `>= 0` fact is likewise STALE on reassignment -- the new value
            // may be negative. A `>= 0` guard re-establishes it where it holds.
            facts.forget_non_negative(&program.expression_table.display_name(assignment.target));
            // Orderings (`i <= j`) naming the target are STALE on reassignment --
            // a guard re-establishes them where they still hold. Without this a
            // chained bound (`i <= j < len => i < len`) could survive a loosening
            // write to `i` or `j`.
            facts.forget_orderings(&program.expression_table.display_name(assignment.target));
            if let Some((symbol, name)) = expression_name(program, assignment.target) {
                let next_length = expression_indexable_length(program, facts, assignment.value);
                let next_integer = expression_integer_value(program, facts, assignment.value);
                facts.assign_local(symbol, name, next_length, next_integer);
                seed_boolean_guard_local(program, facts, symbol, name, assignment.value);
                seed_local_alias_facts(program, facts, assignment.value, name);
                seed_subslice_window_facts(program, facts, assignment.value, name);
            } else if let Some((symbol, name)) = expression_member_name(program, assignment.target)
            {
                let next_integer = expression_integer_value(program, facts, assignment.value);
                facts.assign_field_integer(symbol, name, next_integer);
                seed_offset_index_bound(program, facts, assignment.target, assignment.value);
            }
        }
        StatementNode::Call(call) => {
            for argument in program.statement_table.expression_handles(call.arguments) {
                check_expression(program, machine, state, facts, *argument, diagnostics);
            }
            facts.forget_field_integers();
            // R4 witness mint, checker tier: a BOUNDARY callee's `ensures
            // <param> <= K` bounds the `&mut` out-argument's place the
            // moment the call returns (the boundary model's citable fact).
            // Any prior upper-bound fact for a written place is dropped
            // first; the ensures then re-proves what it states.
            seed_boundary_call_ensures_facts(program, machine, call, facts);
        }
        StatementNode::Expression(expression) => {
            check_expression(program, machine, state, facts, *expression, diagnostics);
        }
        StatementNode::LocalData(local) => {
            check_expression(
                program,
                machine,
                state,
                facts,
                local.initial_value,
                diagnostics,
            );
            let length = expression_indexable_length(program, facts, local.initial_value)
                .or_else(|| fixed_array_type_length(program, local.type_reference));
            let integer = expression_integer_value(program, facts, local.initial_value);
            facts.define_local(local.symbol, local.name.to_string(), length, integer);
            seed_boolean_guard_local(
                program,
                facts,
                local.symbol,
                Some(local.name.as_str()),
                local.initial_value,
            );
            seed_local_alias_facts(
                program,
                facts,
                local.initial_value,
                Some(local.name.as_str()),
            );
            seed_subslice_window_facts(
                program,
                facts,
                local.initial_value,
                Some(local.name.as_str()),
            );
        }
        StatementNode::Transition(transition) => {
            let (target_facts, continuation_facts) = match transition.guard {
                TransitionGuardNode::When(guard) => {
                    check_expression(program, machine, state, facts, guard, diagnostics);
                    let mut guarded_facts = facts.clone();
                    seed_guard_facts(program, &mut guarded_facts, guard);
                    super::guards::seed_value_vs_value_endpoints(
                        program,
                        machine,
                        state,
                        &mut guarded_facts,
                        guard,
                    );
                    let mut negated_facts = facts.clone();
                    seed_negated_guard_facts(program, &mut negated_facts, guard);
                    (guarded_facts, negated_facts)
                }
                TransitionGuardNode::Always => (facts.clone(), facts.clone()),
            };
            check_transition_target(
                program,
                machine,
                state,
                &target_facts,
                transition.target,
                diagnostics,
            );
            check_transition_target(
                program,
                machine,
                state,
                &continuation_facts,
                transition.continuation,
                diagnostics,
            );
        }
    }
}

fn seed_boolean_guard_local(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    symbol: psi_symbols::SymbolHandle,
    name: Option<&str>,
    expression: ExpressionHandle,
) {
    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Binary(_)
    ) {
        facts.define_boolean_guard_local(symbol, name.unwrap_or_default().to_owned(), expression);
    }
}

fn expression_member_name(
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<(psi_symbols::SymbolHandle, Option<&str>)> {
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return None;
    };
    Some((member.member_symbol, Some(member.member.as_str())))
}

/// When `target` is assigned `source + positiveConst`, carry `source`'s exclusive
/// upper bound across the offset: `target < source_bound + const`. This is sound
/// because `target = source + const` exactly (an overflowing add traps under
/// Trapping and is a proof obligation under Exact), so whenever `source` is
/// in-bounds at runtime `target` stays within `source_bound + const`. It proves
/// the derived-index pattern `arr[i + 1]` -- a `jp = self.i + 1` field then
/// `arr[self.jp]` inside a loop where `self.i` is bounded by the loop guard
/// (sorts, sliding windows, reversals).
fn seed_offset_index_bound(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    target: ExpressionHandle,
    value: ExpressionHandle,
) {
    let Some((source_name, offset)) = field_plus_positive_constant(program, value) else {
        return;
    };
    let Some(source_bound) = facts.proven_index_upper_bound(&source_name) else {
        return;
    };
    let Some(new_bound) = source_bound.checked_add(offset) else {
        return;
    };
    let target_name = program.expression_table.display_name(target);
    facts.prove_index_upper_bound(target_name, new_bound);
}

/// Recognize `field + positiveConst` (either operand order), returning the
/// field's display name and the constant.
fn field_plus_positive_constant(
    program: &psi_typed_trees::TypedTrees,
    value: ExpressionHandle,
) -> Option<(String, i64)> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(value) else {
        return None;
    };
    if !matches!(binary.operator, BinaryOperator::Add) {
        return None;
    }
    for (field_side, constant_side) in [(binary.left, binary.right), (binary.right, binary.left)] {
        if let ExpressionNode::Integer(constant) =
            program.expression_table.expression(constant_side)
            && let Some(constant) = constant.value_i64()
            && constant > 0
        {
            return Some((program.expression_table.display_name(field_side), constant));
        }
    }
    None
}

/// Resolve a call statement's BOUNDARY-TRAIT callee signature from the typed
/// trees (receiver field's declared trait), then seed `ensures <param> <OP>
/// <literal>` conjuncts as index-upper-bound facts on the matching `&mut`
/// argument places. Prior bounds for every `&mut`-written place are
/// forgotten regardless, ensures or not.
pub(super) fn seed_boundary_call_ensures_facts(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    call: &psi_typed_trees::statement::TableCall,
    facts: &mut RangeFacts<'_>,
) {
    use psi_typed_trees::domain::ProofFact;
    use psi_typed_trees::signature::SignatureContractKind;
    let arguments = program.statement_table.expression_handles(call.arguments);
    // Drop stale bounds for every &mut-written place first.
    for argument in arguments {
        if let ExpressionNode::Borrow(inner) = program.expression_table.expression(*argument) {
            let place = program.expression_table.display_name(inner.target);
            facts.forget_index_upper_bound(&place);
        }
    }
    // Receiver field -> declared trait -> called signature (the shared
    // TypedTrees chain).
    let Some(signature) =
        psi_typed_trees::boundary::called_boundary_signature(program, machine, call)
    else {
        return;
    };
    let parameters: Vec<_> = program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect();
    for contract in program
        .signature_contracts
        .span_or_empty(signature.contracts)
    {
        if !matches!(contract.kind, SignatureContractKind::Ensures) {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            seed_ensures_bound_conjunct(program, &parameters, arguments, *expression, facts);
        }
    }
}

fn seed_ensures_bound_conjunct(
    program: &psi_typed_trees::TypedTrees,
    parameters: &[&psi_typed_trees::signature::StateParameter],
    arguments: &[ExpressionHandle],
    conjunct: ExpressionHandle,
    facts: &mut RangeFacts<'_>,
) {
    use psi_typed_trees::expression::BinaryOperator;
    let ExpressionNode::Binary(comparison) = program.expression_table.expression(conjunct) else {
        return;
    };
    if comparison.operator == BinaryOperator::And {
        let (left, right) = (comparison.left, comparison.right);
        seed_ensures_bound_conjunct(program, parameters, arguments, left, facts);
        seed_ensures_bound_conjunct(program, parameters, arguments, right, facts);
        return;
    }
    // `param <= K` / `param < K`, param on the left (the ensures house
    // spelling); the EXCLUSIVE bound feeds the index prover.
    let exclusive = match comparison.operator {
        BinaryOperator::LessOrEqual => 1,
        BinaryOperator::Less => 0,
        _ => return,
    };
    let ExpressionNode::Name(path) = program.expression_table.expression(comparison.left) else {
        return;
    };
    let [param_name] = program.expression_table.name_path_members(path.members) else {
        return;
    };
    let ExpressionNode::Integer(literal) = program.expression_table.expression(comparison.right)
    else {
        return;
    };
    let Some(bound) = literal
        .value_i64()
        .and_then(|value| value.checked_add(exclusive))
    else {
        return;
    };
    let Some(position) = parameters
        .iter()
        .position(|parameter| parameter.name.as_str() == param_name.as_str())
    else {
        return;
    };
    let Some(argument) = arguments.get(position).copied() else {
        return;
    };
    let ExpressionNode::Borrow(place) = program.expression_table.expression(argument) else {
        return;
    };
    let place = program.expression_table.display_name(place.target);
    facts.prove_index_upper_bound(place, bound);
}
