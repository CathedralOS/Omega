//! Specialization custody commitments of a selected source closure.
//!
//! Specialization custody applies to ordinary calls as well as calls that
//! produce proof evidence, so the closure's commitment roster is drawn from
//! the exact source owners and the emitted call occurrences — display names
//! and matching callback signatures cannot select a body. The roster is
//! replayed once per selected batch before source companions are discarded.

use checked_trees::CheckedTrees;
use lowered_psi::LoweredPsi;
use symbols::SymbolHandle;

/// The checked specialization instances whose custody this selected closure
/// must commit: every specialization whose instance is an included source, or
/// whose instance's states appear as the closure's call endpoints.
pub(crate) fn selected_closure_specializations(
    checked: &CheckedTrees,
    source_machines: &[SymbolHandle],
    lowered: &LoweredPsi,
) -> Vec<SymbolHandle> {
    checked
        .machine_specializations
        .iter()
        .filter(|specialization| {
            source_machines.contains(&specialization.instance)
                || checked.machines().iter().any(|machine| {
                    machine.symbol == specialization.instance
                        && checked.machine_states(machine).iter().any(|state| {
                            lowered.source_call_occurrences.iter().any(|call| {
                                call.source_state == state.symbol
                                    || call.source_target == state.symbol
                            })
                        })
                })
        })
        .map(|specialization| specialization.instance)
        .collect()
}
