//! Positive proof-context selection for domain-owned operator meanings
//! (chapter 8, "Domain-Sensitive Operators"): a domain-owned spelled candidate
//! is ADMISSIBLE at a use site only when the left operand's membership in the
//! owning domain is a PROVEN fact in the flow context entering the statement.
//! The same context the call-`requires` discharge reads supplies the facts, so
//! caller `requires`, call `ensures`, and interleaved mutation invalidation
//! all participate for free.
//!
//! Selection ruling (recorded in TASKS): exactly one admissible domain meaning
//! wins the expression — the proven fact deliberately narrowed the context and
//! exposes a unique meaning, so it takes precedence over the builtin surface.
//! No admissible domain meaning leaves the ordinary operation in place when
//! one exists (unique root candidate or a primitive operand's builtin), and is
//! rejected otherwise. Two or more admissible domain meanings are ambiguous.

use omega_checked_trees::{
    CheckedOperatorCandidateFact, CheckedOperatorFacts, CheckedOperatorResolutionStatus,
    CheckedOperatorUseFact, CheckedValueOrigin, FlowFacts, FlowStateFact,
};
use omega_core::symbols::SymbolHandle;
use omega_facts::FactPlan;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

use super::receiver::expression_type_reference_for_origin;
use crate::semantic_places::canonical_place_to_fact_place_in_state;

/// Rewrites every `DomainPending` operator use into its final resolution
/// status using the proof contexts that now exist. Runs after flow facts are
/// built and before the checks; no `DomainPending` use survives this pass.
pub(crate) fn select_pending_domain_operator_meanings(
    program: &TypedTrees,
    operators: &mut CheckedOperatorFacts,
    semantic: &mut FactPlan,
    flow: &FlowFacts,
) {
    let pending: Vec<_> = operators
        .uses
        .iter()
        .filter(|(_, operator_use)| {
            operator_use.status == CheckedOperatorResolutionStatus::DomainPending
        })
        .map(|(handle, operator_use)| (handle, *operator_use))
        .collect();

    for (handle, operator_use) in pending {
        let candidates: Vec<CheckedOperatorCandidateFact> =
            operators.candidates(&operator_use).to_vec();
        let (status, selected_operator_symbol) =
            finalize_pending_use(program, semantic, flow, &operator_use, &candidates);
        let operator_use = operators.uses.get_mut(handle);
        operator_use.status = status;
        operator_use.selected_operator_symbol = selected_operator_symbol;
    }
}

fn finalize_pending_use(
    program: &TypedTrees,
    semantic: &mut FactPlan,
    flow: &FlowFacts,
    operator_use: &CheckedOperatorUseFact,
    candidates: &[CheckedOperatorCandidateFact],
) -> (CheckedOperatorResolutionStatus, SymbolHandle) {
    let admissible: Vec<&CheckedOperatorCandidateFact> = candidates
        .iter()
        .filter(|candidate| candidate.is_domain_owned())
        .filter(|candidate| {
            domain_membership_proven_at_use(
                program,
                semantic,
                flow,
                operator_use,
                candidate.domain_symbol,
            )
        })
        .collect();

    match admissible.as_slice() {
        [winner] => (
            CheckedOperatorResolutionStatus::Resolved,
            winner.operator_symbol,
        ),
        [] => inadmissible_domain_fallback(program, operator_use, candidates),
        // Competing PROVEN domain meanings for one expression: chapter 8 makes
        // this a compile error rather than picking silently.
        _ => (
            CheckedOperatorResolutionStatus::Ambiguous,
            SymbolHandle::invalid(),
        ),
    }
}

/// No domain candidate was admissible: the ordinary meaning stays selected
/// when one exists (chapter 8: declaring a domain operator does not replace
/// the builtin). A unique root spelled candidate is that surface; otherwise a
/// primitive left operand keeps its builtin operation. A non-primitive operand
/// with only inadmissible domain meanings has no meaning at all.
fn inadmissible_domain_fallback(
    program: &TypedTrees,
    operator_use: &CheckedOperatorUseFact,
    candidates: &[CheckedOperatorCandidateFact],
) -> (CheckedOperatorResolutionStatus, SymbolHandle) {
    let roots: Vec<&CheckedOperatorCandidateFact> = candidates
        .iter()
        .filter(|candidate| !candidate.is_domain_owned())
        .collect();

    match roots.as_slice() {
        [root] => (
            CheckedOperatorResolutionStatus::Resolved,
            root.operator_symbol,
        ),
        [] if builtin_meaning_exists(program, operator_use) => (
            CheckedOperatorResolutionStatus::BuiltinFallback,
            SymbolHandle::invalid(),
        ),
        [] => (
            CheckedOperatorResolutionStatus::Inadmissible,
            SymbolHandle::invalid(),
        ),
        _ => (
            CheckedOperatorResolutionStatus::Ambiguous,
            SymbolHandle::invalid(),
        ),
    }
}

/// Whether the left operand's membership in `domain_symbol` is proven by any
/// semantic context entering the use's statement. Statement entry constraints
/// are already invalidation-adjusted, so a fact killed by an interleaved
/// mutation de-admits the candidate here.
fn domain_membership_proven_at_use(
    program: &TypedTrees,
    semantic: &mut FactPlan,
    flow: &FlowFacts,
    operator_use: &CheckedOperatorUseFact,
    domain_symbol: SymbolHandle,
) -> bool {
    let CheckedValueOrigin::StateStatement {
        machine_symbol,
        state_symbol,
        statement_index,
        ..
    } = operator_use.origin
    else {
        // Non-statement origins (contract expressions, decreases, data
        // initializers) carry no flow context, so no domain fact is proven.
        return false;
    };
    let Some(left_operand) = binary_left_operand(program, operator_use.expression) else {
        return false;
    };
    let Some(state_flow) = find_state_flow(flow, machine_symbol, state_symbol) else {
        return false;
    };
    let Some(place) = canonical_place_to_fact_place_in_state(
        program,
        semantic,
        state_symbol,
        statement_index,
        left_operand,
    ) else {
        return false;
    };

    let entry_constraints = flow
        .state_statement(state_flow, statement_index)
        .map(|statement| statement.entry_constraints)
        .unwrap_or(state_flow.entry_constraints);
    flow.semantic_constraint_contexts(entry_constraints)
        .any(|context| {
            let context = semantic.contexts.get(context);
            semantic
                .context_view(context)
                .proves_place_domain_membership_in_program(program, place, domain_symbol)
        })
}

fn binary_left_operand(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => Some(binary.left),
        _ => None,
    }
}

fn find_state_flow<'flow>(
    flow: &'flow FlowFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Option<&'flow FlowStateFact> {
    flow.control
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.machine_symbol == machine_symbol && state.state_symbol == state_symbol)
}

/// Whether the left operand type carries the ordinary builtin operation: a
/// primitive scalar does, a user data type does not.
fn builtin_meaning_exists(program: &TypedTrees, operator_use: &CheckedOperatorUseFact) -> bool {
    let Some(left_operand) = binary_left_operand(program, operator_use.expression) else {
        return false;
    };
    expression_type_reference_for_origin(program, left_operand, operator_use.origin)
        .is_some_and(|type_reference| type_is_primitive(program, type_reference))
}

fn type_is_primitive(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => type_is_primitive(program, *referee),
        TypeReferenceNode::Named { name, .. } => PrimitiveType::from_name(name.as_str()).is_some(),
        TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => false,
    }
}
