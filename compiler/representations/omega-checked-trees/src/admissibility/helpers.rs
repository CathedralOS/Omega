use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

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

pub(super) fn effect_evidence_count(effects: omega_effects::EffectSet) -> usize {
    effects.bits().count_ones() as usize
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
