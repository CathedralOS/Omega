use arena::HandleSpan;
use symbols::SymbolHandle;

use crate::{
    CheckFacts, CheckedValueOrigin, FlowConstraintKind, FlowConstraintRef, FlowFacts,
    FlowSemanticContextRef,
};

pub(super) fn semantic_contexts(
    flow: &FlowFacts,
    contexts: HandleSpan<FlowSemanticContextRef>,
) -> &[FlowSemanticContextRef] {
    flow.contexts.semantic_context_refs.span_or_empty(contexts)
}

pub(super) fn constraints(
    flow: &FlowFacts,
    constraints: HandleSpan<FlowConstraintRef>,
) -> &[FlowConstraintRef] {
    flow.contexts.constraint_refs.span_or_empty(constraints)
}

pub(super) fn borrow_constraint_count(
    flow: &FlowFacts,
    constraints: HandleSpan<FlowConstraintRef>,
) -> usize {
    self::constraints(flow, constraints)
        .iter()
        .filter(|constraint| {
            matches!(
                constraint.kind,
                FlowConstraintKind::BorrowState { .. }
                    | FlowConstraintKind::BorrowCall { .. }
                    | FlowConstraintKind::BorrowWritableRoot { .. }
                    | FlowConstraintKind::BorrowAccess { .. }
                    | FlowConstraintKind::BorrowLoan { .. }
            )
        })
        .count()
}

pub(super) fn service_reach_evidence_count(
    facts: &CheckFacts,
    service_reach: language_semantics::ServiceReachSummary,
) -> usize {
    facts
        .service_reaches
        .rows
        .services(service_reach.transitive)
        .len()
}

pub(super) const fn suspension_evidence_count(
    suspension: language_semantics::SuspensionSummary,
) -> usize {
    suspension.transitive_may_suspend as usize
}

pub(super) const fn blocking_evidence_count(
    blocking: language_semantics::BlockingSummary,
) -> usize {
    blocking.transitive_may_block as usize
}

pub(super) fn machine_decrease_count(facts: &CheckFacts, machine_symbol: SymbolHandle) -> usize {
    facts
        .values
        .values
        .iter()
        .filter(|(_, value)| {
            matches!(
                value.origin,
                CheckedValueOrigin::MachineDecrease {
                    machine_symbol: origin_machine,
                    ..
                } if origin_machine == machine_symbol
            )
        })
        .count()
}
