//! Guarded-exit admission for outcome-specific guarantees.
//!
//! A checked `outcome`-parameterized guarantee is provable only through a
//! guarded exit: either the selected entry itself returns one exact
//! payloadless structural case, or its guarantee rides the caller arm of a
//! guarded payloadless call into the selected callee. The lowering route
//! refuses guarantees that cannot reach such an exit once the source closure
//! is lowered.
//!
//! The exact case exit is read from the lowered entry machine, not from which
//! producer lowered it: a payloadless case constructor is an ordinary Unit
//! body, and the verifier replays the same exact-case exit
//! (`exact_payloadless_case_return_exits`) before it accepts any guarded row.

use checked_trees::CheckedTrees;
use symbols::SymbolHandle;
use terminal_psi::{
    OperationKind, OutcomeSpecificGuard, TerminalMachine, TerminalModule, Terminator,
};

use crate::lowering_error::{LoweringError, unsupported};

/// The one guard an exact payloadless case return establishes: the machine
/// returns exactly once, from a field-free case construction of its own
/// structural result type.
pub(crate) fn exact_payloadless_return_guard(
    machine: &TerminalMachine,
) -> Option<OutcomeSpecificGuard> {
    let result = machine.result.structural()?;
    let mut returns = machine.blocks.iter().filter_map(|block| {
        let Terminator::ReturnStructural { source, .. } = block.terminator else {
            return None;
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
        (operation_result.structural_type == result.structural_type).then_some(
            OutcomeSpecificGuard {
                result_type: result.structural_type,
                result_case,
            },
        )
    });
    let guard = returns.next()?;
    returns.next().is_none().then_some(guard)
}

/// Refuse outcome-specific guarantees no guarded exit of the lowered closure
/// can publish.
///
/// A guarantee is admitted when its machine is the entry of a single-machine
/// closure whose lowered entry returns one exact payloadless case, or when
/// the entry is a guarded payloadless caller and the machine is its callee.
pub(crate) fn reject_unguarded_outcome_guarantees(
    checked: &CheckedTrees,
    source_machines: &[SymbolHandle],
    entry: SymbolHandle,
    module: &TerminalModule,
) -> Result<(), LoweringError> {
    let exact_payloadless_entry = source_machines == [entry]
        && module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .and_then(exact_payloadless_return_guard)
            .is_some();
    let payloadless_callee = checked
        .facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_for_machine(entry)
        .map(|plan| plan.target_machine);
    if checked
        .facts
        .proof
        .outcome_specific_guarantees
        .iter()
        .any(|(_, guarantee)| {
            source_machines.contains(&guarantee.machine_symbol)
                && !((exact_payloadless_entry && guarantee.machine_symbol == entry)
                    || payloadless_callee == Some(guarantee.machine_symbol))
        })
    {
        return unsupported(
            "outcome-specific guarantees require guarded exit and caller-arm lowering",
        );
    }
    Ok(())
}
