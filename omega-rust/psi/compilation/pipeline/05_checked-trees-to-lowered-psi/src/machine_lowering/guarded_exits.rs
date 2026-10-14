//! Guarded-exit admission for outcome-specific guarantees.
//!
//! A checked `outcome`-parameterized guarantee is provable only through
//! guarded exits: every return of the guaranteeing machine is an exact
//! payloadless structural case, so each exit names the case whose guarantees
//! it establishes. A machine that decides between cases (from a scalar input)
//! returns from several blocks; a checked guarantee for one case rides only
//! the exits of that case. The lowering route refuses guarantees whose
//! machine has any other kind of exit once the source closure is lowered.
//! The guaranteeing machine may be the entry or any callee in the closure; a
//! caller arm that selects the guarded case carries the guarantee on its
//! structural call site.
//!
//! The exact case exits are read from the lowered machine, not from which
//! producer lowered it: a payloadless case constructor is an ordinary Unit
//! body, and the verifier replays the same exact-case exits
//! (`exact_payloadless_case_return_exits`) before it accepts any guarded row.

use checked_trees::CheckedTrees;
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;
use terminal_psi::{
    OperationKind, OutcomeSpecificGuard, TerminalMachine, TerminalModule, Terminator,
};

use crate::lowering_error::{LoweringError, unsupported};

/// The guards the machine's exact payloadless case returns establish, one
/// per returning block, or `None` when any return is not a field-free case
/// construction of the machine's own structural result type.
pub(crate) fn exact_payloadless_return_guards(
    machine: &TerminalMachine,
) -> Option<Vec<OutcomeSpecificGuard>> {
    let result = machine.result.structural()?;
    let mut guards = Vec::new();
    for block in &machine.blocks {
        let Terminator::ReturnStructural { source, .. } = block.terminator else {
            continue;
        };
        let operation = block.operations.iter().find(|operation| {
            operation
                .result
                .structural()
                .is_some_and(|result| result.place == source)
        })?;
        let OperationKind::EstablishScalarCase {
            result_case,
            ref fields,
        } = operation.kind
        else {
            return None;
        };
        if !fields.is_empty() {
            return None;
        }
        let operation_result = operation.result.structural()?;
        if operation_result.structural_type != result.structural_type {
            return None;
        }
        guards.push(OutcomeSpecificGuard {
            result_type: result.structural_type,
            result_case,
        });
    }
    (!guards.is_empty()).then_some(guards)
}

/// Refuse outcome-specific guarantees no guarded exit of the lowered closure
/// can publish.
///
/// A guarantee is admitted when every return of its machine's lowered body
/// is an exact payloadless case.
pub(crate) fn reject_unguarded_outcome_guarantees(
    checked: &CheckedTrees,
    source_machine_ids: &[(SymbolHandle, MachineId)],
    module: &TerminalModule,
) -> Result<(), LoweringError> {
    let guarded = |machine: SymbolHandle| {
        source_machine_ids
            .iter()
            .find(|(source, _)| *source == machine)
            .and_then(|(_, terminal)| module.machine(*terminal))
            .and_then(exact_payloadless_return_guards)
            .is_some()
    };
    if checked
        .facts
        .proof
        .outcome_specific_guarantees
        .iter()
        .any(|(_, guarantee)| {
            source_machine_ids
                .iter()
                .any(|(source, _)| *source == guarantee.machine_symbol)
                && !guarded(guarantee.machine_symbol)
        })
    {
        return unsupported(
            "outcome-specific guarantees require guarded exit and caller-arm lowering",
        );
    }
    Ok(())
}
