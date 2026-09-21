//! Guarded-exit admission for outcome-specific guarantees.
//!
//! A checked `outcome`-parameterized guarantee is provable only through a
//! guarded exit: either the machine itself is the exact payloadless
//! structural case, or its guarantee rides the caller arm of a guarded
//! payloadless call into the selected callee. The lowering route therefore
//! refuses guarantees that cannot reach such an exit, once for the selected
//! entry before any lowering work starts (the callee exemption does not
//! apply to an entry still awaiting its closure) and once for the complete
//! source closure afterward.

use checked_trees::CheckedTrees;
use symbols::SymbolHandle;

use crate::lowering_error::{LoweringError, unsupported};

/// The guarded-exit routes available to one selected entry, read from the
/// checked flow facts before lowering.
pub(crate) struct GuardedExitAdmission {
    /// The entry produces its own exact payloadless structural case.
    pub(crate) exact_payloadless: bool,
    /// The entry's guarded payloadless call targets this machine, so its
    /// outcome guarantees discharge through that caller arm.
    pub(crate) payloadless_callee: Option<SymbolHandle>,
}

impl GuardedExitAdmission {
    pub(crate) fn for_entry(checked: &CheckedTrees, entry: SymbolHandle) -> Self {
        Self {
            exact_payloadless: checked
                .facts
                .flow
                .terminal_structural_returns
                .payloadless_case_for_machine(entry)
                .is_some(),
            payloadless_callee: checked
                .facts
                .flow
                .terminal_structural_call_returns
                .payloadless_guarded_for_machine(entry)
                .map(|plan| plan.target_machine),
        }
    }
}

/// Refuse outcome-specific guarantees no guarded exit can publish.
///
/// A guarantee is admitted when its machine is the exact payloadless entry
/// of a single-machine closure, or when `admit_guarded_callee` accepts the
/// caller-arm exemption and the machine is the guarded callee. The early
/// entry check passes `admit_guarded_callee: false`: the callee exemption is
/// earned by the emitted closure, not the selection alone.
pub(crate) fn reject_unguarded_outcome_guarantees(
    checked: &CheckedTrees,
    source_machines: &[SymbolHandle],
    entry: SymbolHandle,
    admission: &GuardedExitAdmission,
    admit_guarded_callee: bool,
) -> Result<(), LoweringError> {
    if checked
        .facts
        .proof
        .outcome_specific_guarantees
        .iter()
        .any(|(_, guarantee)| {
            source_machines.contains(&guarantee.machine_symbol)
                && !((admission.exact_payloadless
                    && source_machines == [entry]
                    && guarantee.machine_symbol == entry)
                    || (admit_guarded_callee
                        && admission.payloadless_callee == Some(guarantee.machine_symbol)))
        })
    {
        return unsupported(
            "outcome-specific guarantees require guarded exit and caller-arm lowering",
        );
    }
    Ok(())
}
