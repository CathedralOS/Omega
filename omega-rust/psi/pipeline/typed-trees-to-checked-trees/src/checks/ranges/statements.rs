mod aliases;
mod transitions;

use self::aliases::{seed_local_alias_facts, seed_subslice_window_facts};
use self::transitions::check_transition_target;
use super::arrays::fixed_array_type_length;
use super::expressions::{expression_indexable_length, expression_integer_value, expression_name};
use super::facts::RangeFacts;
use super::guards::{seed_guard_facts, seed_negated_guard_facts};
use super::indexes::check_expression;
use diagnostics::Diagnostic;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionGuardNode};

pub(super) fn check_statement<'program>(
    program: &'program typed_trees::TypedTrees,
    machine: &'program Machine,
    state: &State,
    call_frames: Option<&validation::CallFrameResolver<'program>>,
    facts: &mut RangeFacts<'_>,
    statement: &'program StatementNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => {
            check_expression(
                program,
                machine,
                state,
                call_frames,
                facts,
                assignment.target,
                diagnostics,
            );
            check_expression(
                program,
                machine,
                state,
                call_frames,
                facts,
                assignment.value,
                diagnostics,
            );
            // RHS effects and values are evaluated before replacing the target.
            let next_length = expression_indexable_length(program, facts, assignment.value);
            let next_integer = expression_integer_value(program, facts, assignment.value);
            facts.invalidate_assignment_bounds(
                &program.expression_table.display_name(assignment.target),
            );
            if let Some((symbol, name)) = expression_name(program, assignment.target) {
                facts.assign_local(symbol, name, next_length, next_integer);
                seed_boolean_guard_local(
                    program,
                    machine,
                    call_frames,
                    facts,
                    symbol,
                    name,
                    assignment.value,
                );
                seed_local_alias_facts(program, facts, assignment.value, name);
                seed_subslice_window_facts(program, facts, assignment.value, name);
            } else if let Some((symbol, name)) = expression_member_name(program, assignment.target)
            {
                facts.assign_field_integer(symbol, name, next_integer);
                seed_offset_index_bound(program, facts, assignment.target, assignment.value);
            }
        }
        StatementNode::Call(call) => {
            for argument in program.statement_table.expression_handles(call.arguments) {
                check_expression(
                    program,
                    machine,
                    state,
                    call_frames,
                    facts,
                    *argument,
                    diagnostics,
                );
            }
            let paths = call_frames
                .and_then(|frames| frames.may_write_frame(machine, call).into_complete_paths());
            facts.invalidate_call_writes(program, state, paths.as_deref());
            // R4 witness mint, checker tier: a BOUNDARY callee's `ensures
            // <param> <= K` bounds the `&mut` out-argument's place the
            // moment the call returns (the boundary model's citable fact).
            // Any prior upper-bound fact for a written place is dropped
            // first; the ensures then re-proves what it states.
            seed_boundary_call_ensures_facts(program, machine, call, facts);
        }
        StatementNode::Expression(expression) => {
            check_expression(
                program,
                machine,
                state,
                call_frames,
                facts,
                *expression,
                diagnostics,
            );
        }
        StatementNode::LocalData(local) => {
            check_expression(
                program,
                machine,
                state,
                call_frames,
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
                machine,
                call_frames,
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
            let (mut target_facts, mut continuation_facts) = match transition.guard {
                TransitionGuardNode::When(guard) => {
                    check_expression(
                        program,
                        machine,
                        state,
                        call_frames,
                        facts,
                        guard,
                        diagnostics,
                    );
                    let mut guarded_facts = facts.clone();
                    let mut negated_facts = facts.clone();
                    if call_frames.is_some_and(|frames| {
                        frames
                            .expression_write_frame(machine, guard)
                            .into_complete_paths()
                            .is_some_and(|paths| paths.is_empty())
                    }) {
                        seed_guard_facts(program, &mut guarded_facts, guard);
                        super::guards::seed_value_vs_value_endpoints(
                            program,
                            machine,
                            state,
                            &mut guarded_facts,
                            guard,
                        );
                        seed_negated_guard_facts(program, &mut negated_facts, guard);
                    }
                    (guarded_facts, negated_facts)
                }
                TransitionGuardNode::Always => (facts.clone(), facts.clone()),
            };
            check_transition_target(
                program,
                machine,
                state,
                call_frames,
                &mut target_facts,
                transition.target,
                diagnostics,
            );
            check_transition_target(
                program,
                machine,
                state,
                call_frames,
                &mut continuation_facts,
                transition.continuation,
                diagnostics,
            );
            // Reaching the next statement refutes a prior exit arm. Guard
            // evaluation has already retired its write-affected facts, and
            // only the existing read-only frame gate seeds a new complement.
            // Target effects belong to the selected exit, not fall-through.
            if transition.target.is_valid() && !transition.continuation.is_valid() {
                *facts = continuation_facts;
            }
        }
    }
}

fn seed_boolean_guard_local<'program>(
    program: &'program typed_trees::TypedTrees,
    machine: &'program Machine,
    call_frames: Option<&validation::CallFrameResolver<'program>>,
    facts: &mut RangeFacts<'_>,
    symbol: symbols::SymbolHandle,
    name: Option<&str>,
    expression: ExpressionHandle,
) {
    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Binary(_)
    ) && call_frames.is_some_and(|frames| {
        frames
            .expression_write_frame(machine, expression)
            .into_complete_paths()
            .is_some_and(|paths| paths.is_empty())
    }) {
        facts.define_boolean_guard_local(symbol, name.unwrap_or_default().to_owned(), expression);
    }
}

fn expression_member_name(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<(symbols::SymbolHandle, Option<&str>)> {
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
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    call: &typed_trees::statement::TableCall,
    facts: &mut RangeFacts<'_>,
) {
    use typed_trees::domain::ProofFact;
    use typed_trees::signature::SignatureContractKind;
    let arguments = program.statement_table.expression_handles(call.arguments);
    // Both callers apply the complete write frame before reaching this
    // postcondition publisher. Borrow syntax alone is not a write footprint.
    // Receiver field -> declared trait -> called signature (the shared
    // TypedTrees chain).
    let Some(signature) = typed_trees::boundary::called_boundary_signature(program, machine, call)
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
    program: &typed_trees::TypedTrees,
    parameters: &[&typed_trees::signature::StateParameter],
    arguments: &[ExpressionHandle],
    conjunct: ExpressionHandle,
    facts: &mut RangeFacts<'_>,
) {
    use typed_trees::expression::BinaryOperator;
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
