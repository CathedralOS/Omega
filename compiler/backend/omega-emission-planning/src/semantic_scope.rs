use crate::EmissionPlanningInput;
use omega_checked_trees::name::ProgramName;
use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};

pub(super) fn state_name(input: &EmissionPlanningInput<'_>, key: StateKey) -> String {
    input
        .control_flow
        .state_names_by_key(key)
        .map(|(machine, state)| format!("{machine}.{state}"))
        .unwrap_or_else(|| "<unknown>.<unknown>".to_owned())
}

pub(super) fn proof_scope_suffix(input: &EmissionPlanningInput<'_>, key: StateKey) -> String {
    let obligation_count = input
        .control_flow
        .proof_obligations
        .iter()
        .filter(|(_, obligation)| {
            obligation.machine_symbol == key.machine && obligation.state_symbol == key.state
        })
        .count();
    let guarded_count = input
        .control_flow
        .proof_obligations
        .iter()
        .filter(|(_, obligation)| {
            obligation.machine_symbol == key.machine
                && obligation.state_symbol == key.state
                && obligation.kind == omega_control_flow::ProofFactKind::GuardedTransition
        })
        .count();

    if obligation_count == 0 {
        String::new()
    } else if guarded_count == 0 {
        format!(" ({obligation_count} checked proof obligation(s))")
    } else {
        format!(
            " ({obligation_count} checked proof obligation(s), {guarded_count} guarded-transition)"
        )
    }
}

pub(super) fn invariant_suffix(
    invariant_names: &Arena<ProgramName>,
    names: HandleSpan<ProgramName>,
) -> String {
    match invariant_names.span_or_empty(names) {
        [] => String::new(),
        [name] => format!(" (checked invariant `{}`)", name),
        _ => format!(
            " (checked invariants: {})",
            invariant_names
                .span_or_empty(names)
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}
