pub(in crate::checks::ranges) mod aliases;
#[cfg(test)]
mod tests;
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
        StatementNode::RootBinding(_) | StatementNode::AssemblyFact(_) => {}
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
            let extent_survives = super::assignment_lengths::value_preserves_indexed_extent(
                program,
                machine,
                state,
                call_frames,
                facts,
                assignment.target,
                assignment.value,
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
            if !extent_survives {
                diagnostics.push(Diagnostic::error(
                    "cannot prove index remains within the captured byte collection's live length across RHS mutation",
                ));
            }
            // RHS effects and values are evaluated before replacing the target.
            let mut next_length = super::assignment_lengths::replacement_length(
                program,
                machine,
                state,
                facts,
                assignment.target,
                assignment.value,
            );
            // A rebound reference takes its NEW referent's extent the way the
            // `let` binding does: `view = borrow_rows(other)` re-lends
            // `other.rooms`, so the write-origins prefix proves the
            // replacement slice's length evidence the same way.
            let mut referent_floor = None;
            if next_length.is_none()
                && let Some((symbol, _)) = expression_name(program, assignment.target)
                && let Some(declared) =
                    assigned_local_declared_type(program, state, facts.statement_index, symbol)
                && let Some(referent) = bound_reference_referent_extent(
                    program,
                    machine,
                    state,
                    call_frames,
                    facts,
                    symbol,
                    declared,
                )
            {
                next_length = referent.exact;
                referent_floor = referent.minimum;
            }
            let next_integer = expression_integer_value(program, facts, assignment.value);
            let extent = super::assignment_lengths::assigned_extent(
                program,
                machine,
                state,
                facts,
                assignment.target,
                assignment.value,
            );
            facts.invalidate_assignment_bounds(program, machine, state, statement);
            if let Some((symbol, name)) = expression_name(program, assignment.target) {
                facts.assign_local(symbol, name, next_length, next_integer);
                // The referent's floor describes the NEW binding, so it seeds
                // only after the rebound label sheds the old value's facts.
                if let Some(floor) = referent_floor {
                    facts.prove_minimum_length(
                        program.expression_table.display_name(assignment.target),
                        floor,
                    );
                }
                seed_boolean_guard_local(
                    program,
                    machine,
                    call_frames,
                    facts,
                    symbol,
                    name,
                    assignment.value,
                );
                seed_local_alias_facts(
                    program,
                    machine,
                    state,
                    facts,
                    assignment.value,
                    symbol,
                    name,
                );
                seed_subslice_window_facts(program, facts, assignment.value, name);
            } else if let Some((symbol, name)) = expression_member_name(program, assignment.target)
            {
                facts.assign_field_integer(symbol, name, next_integer);
                seed_offset_index_bound(program, facts, assignment.target, assignment.value);
                aliases::seed_ensured_call_result_bounds(
                    program,
                    facts,
                    &program.expression_table.display_name(assignment.target),
                    assignment.value,
                );
            }
            super::assignment_lengths::seed_assigned_extent(program, machine, state, facts, extent);
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
            facts.invalidate_call_writes(
                program,
                machine,
                state,
                paths.as_deref(),
                Some(&crate::semantic_calls::CallSite::Statement(call)),
            );
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
            let mut length = fixed_array_type_length(program, local.type_reference).or_else(|| {
                expression_indexable_length(program, machine, state, facts, local.initial_value)
            });
            // A returned `&mut [T]` binding has no indexable initializer the
            // expression lane can measure: the callee chose the referent. The
            // write-origins prefix recovers that referent, whose declared
            // extent (or recorded live length) is the slice's length.
            if length.is_none()
                && let Some(referent) = bound_reference_referent_extent(
                    program,
                    machine,
                    state,
                    call_frames,
                    facts,
                    local.symbol,
                    local.type_reference,
                )
            {
                length = referent.exact;
                if let Some(minimum) = referent.minimum {
                    facts.prove_minimum_length(local.name.to_string(), minimum);
                }
            }
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
                machine,
                state,
                facts,
                local.initial_value,
                local.symbol,
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
            let readonly_guard = match transition.guard {
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
                    if call_frames.is_some_and(|frames| {
                        frames
                            .expression_write_frame(machine, guard)
                            .into_complete_paths()
                            .is_some_and(|paths| paths.is_empty())
                    }) {
                        Some(guard)
                    } else {
                        None
                    }
                }
                TransitionGuardNode::Always => None,
            };
            if transition.target.is_valid() {
                let mut target_facts = facts.clone();
                if let Some(guard) = readonly_guard {
                    seed_guard_facts(program, machine, state, &mut target_facts, guard);
                    super::guards::seed_value_vs_value_endpoints(
                        program,
                        machine,
                        state,
                        &mut target_facts,
                        guard,
                    );
                }
                check_transition_target(
                    program,
                    machine,
                    state,
                    call_frames,
                    &mut target_facts,
                    transition.target,
                    diagnostics,
                );
            }
            if transition.continuation.is_valid() {
                let mut continuation_facts = facts.clone();
                if let Some(guard) = readonly_guard {
                    seed_negated_guard_facts(
                        program,
                        machine,
                        state,
                        &mut continuation_facts,
                        guard,
                    );
                }
                check_transition_target(
                    program,
                    machine,
                    state,
                    call_frames,
                    &mut continuation_facts,
                    transition.continuation,
                    diagnostics,
                );
            }
            // Reaching the next statement refutes a prior exit arm. Guard
            // evaluation has already retired its write-affected facts, and
            // only the existing read-only frame gate seeds a new complement.
            // Target effects belong to the selected exit, not fall-through.
            // An absent continuation already owns `facts`; cloning it only to
            // replace the original would copy every name and dependency twice.
            if transition.target.is_valid()
                && !transition.continuation.is_valid()
                && let Some(guard) = readonly_guard
            {
                seed_negated_guard_facts(program, machine, state, facts, guard);
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

/// The extent evidence a freshly bound local REFERENCE inherits from its
/// referent, resolved by `bound_reference_referent_extent`.
pub(super) struct ReferentExtent {
    /// The referent's pinned extent: its declared fixed-array length, or a
    /// recorded exact live length on the referent's own label.
    pub(super) exact: Option<usize>,
    /// The referent's proven live-length floor, when one is recorded.
    /// Variable-fill carriers (`[u8; N] in Domain`) hold their live length in
    /// this lane — their capacity says nothing about the current extent.
    pub(super) minimum: Option<i64>,
}

/// The extent a freshly bound local reference inherits from its single exact
/// referent, or `None` when the binding has no uniquely resolved referent.
///
/// A returned `&mut [T]` owns no storage, and its slice type erases the
/// receiver's extent: `let view = borrow_rows(level)` lends the element
/// storage the callee selected (`level.rooms`), so `view` indexes against
/// THAT extent. The write-origins prefix already proves the referent — the
/// same recovery `flow::transfers`' `bound_reference_referent_place` performs
/// to re-anchor element evidence onto the binding — and
/// `canonical_place_type_reference` projects the referent's declared type for
/// its fixed length. When the referent is a variable-fill carrier whose live
/// length is not its type, the recorded `exact_length`/`minimum_length`
/// facts keyed on the referent's own path supply the extent instead.
///
/// An ambiguous or inexact (runtime-index-truncated) origin stays `None`: a
/// truncated place names a container OF the referent, whose extent is not the
/// bound slice's length, and multiple referents cannot pin one extent.
pub(super) fn bound_reference_referent_extent(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    facts: &RangeFacts<'_>,
    local_symbol: symbols::SymbolHandle,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<ReferentExtent> {
    if !local_symbol.is_valid() {
        return None;
    }
    // Only a reference binding HAS a referent. Peel declarative constraints
    // the same way `bound_reference_referent_place` does; any other declared
    // form means the local's extent must come from its own value evidence.
    let mut reference = type_reference;
    for _ in 0..program.type_reference_table.type_reference_count() {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return None;
        }
        match program.type_reference_table.type_reference(reference) {
            typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                reference = *base_type;
            }
            typed_trees::types::TypeReferenceNode::Reference { .. } => break,
            _ => return None,
        }
    }
    // The binding's own alias is recorded once the NEXT statement begins, so
    // the prefix observed there is the one carrying the new referent.
    let before = program
        .statement_table
        .statements(state.statement_nodes)
        .get(facts.statement_index + 1)?;
    let mut referents = call_frames?
        .local_write_origins_before_statement(machine, before)?
        .into_iter()
        .filter(|origin| {
            origin.local_symbol == local_symbol
                && origin.local_segments.is_empty()
                && origin.source_root != local_symbol
        });
    let origin = referents.next()?;
    if referents.next().is_some() {
        return None;
    }
    let (place, exact) = crate::flow::origin_place(program, state, facts.statement_index, &origin)?;
    if !exact {
        return None;
    }
    let exact = crate::flow::canonical_place_type_reference(
        program,
        state.symbol,
        facts.statement_index,
        &place,
    )
    .and_then(|reference| fixed_array_type_length(program, reference))
    .or_else(|| {
        facts
            .exact_length(&origin.source_path)
            .and_then(|length| usize::try_from(length).ok())
    });
    let minimum = facts.minimum_length(&origin.source_path);
    Some(ReferentExtent { exact, minimum })
}

/// The declared type of the local an assignment rebinds, from its unique
/// earlier `let` in this state — the same declared-type recovery
/// `bound_reference_referent_place` performs for `StatementNode::Assignment`.
pub(super) fn assigned_local_declared_type(
    program: &typed_trees::TypedTrees,
    state: &State,
    statement_index: usize,
    local_symbol: symbols::SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let mut declarations = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .filter_map(|statement| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            (local.symbol == local_symbol).then_some(local.type_reference)
        });
    let declared = declarations.next()?;
    declarations.next().is_none().then_some(declared)
}

pub(super) fn expression_member_name(
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
pub(super) fn seed_offset_index_bound(
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
