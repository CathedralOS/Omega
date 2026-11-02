//! The exact executable realization one selected plan row names.
//!
//! Boundary adapter rows, top-level requirement rows, template family rows,
//! specialization roster instances and checked-operator rows all settle the
//! same question: does the machine this row names realize it, and which entry
//! state starts that machine. One owner answers it so the settling lanes
//! cannot drift apart; a lane that accepted a machine the others reject would
//! install a realization the plan row never named.

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

/// The entry state of the exact nominal checked body one selected plan row
/// realizes. `machine` is the machine the lane already chose — the plan's
/// checked adapter, a template family member, or a resolved specialization
/// instance; this verifies it is that row's realization, that it is a checked
/// body, and that it starts at exactly one state with an exact symbol.
/// `label` names the machine in the caller's diagnostics.
pub(crate) fn exact_checked_body_entry<'typed>(
    typed: &'typed TypedTrees,
    plan: &effects::provider_plan::ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
    machine: &'typed typed_trees::machine::Machine,
    label: &str,
) -> Result<&'typed typed_trees::state::State, Diagnostic> {
    if !provider_planning::provider_row_matches(
        provider_planning::ProviderPlanningContext::new(typed, source::DependencyScope::Product),
        plan,
        row,
        machine,
    ) {
        return Err(Diagnostic::error(format!(
            "{label} does not belong to nominal provider `{}`",
            plan.provider_type,
        )));
    }
    if !machine.supply_mode.is_checked_body() {
        return Err(Diagnostic::error(format!("{label} is not a checked body")));
    }
    let Some(entry) = typed.machine_states(machine).first() else {
        return Err(Diagnostic::error(format!(
            "{label} has no executable entry state"
        )));
    };
    if !entry.symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "{label} has no exact entry-state symbol"
        )));
    }
    Ok(entry)
}
