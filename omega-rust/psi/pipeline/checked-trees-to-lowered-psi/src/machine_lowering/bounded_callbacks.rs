//! Bounded callback-body lowering for isolated entrance publication.
//!
//! The selected callback machine lowers through ordinary machine lowering
//! rooted at the callback entry — the same plan-family dispatch and retained
//! post-obligations as [`super::lower_machine`], not a list of admitted body
//! shapes. The callback placement separately owns its satisfaction and ABI
//! evidence and validates the lowered thunk against the requirement's
//! signature and inbound entry plan; this producer owns only the lowering
//! route and the checked-to-Terminal coordinate join.

use checked_trees::CheckedTrees;
use lowered_psi::{CallbackTerminalLoweringReceipt, LoweredCallbackPsi};

use crate::lowering_error::{LoweringError, unsupported};
use crate::machine_lowering::lower_terminal_selection;

/// Lower the selected callback machine through ordinary machine lowering
/// rooted at the callback entry.
///
/// `source_machine` must select exactly one checked Terminal machine, and
/// `source_entry` must be that machine's checked entry state. The resulting
/// module retains every obligation the ordinary route already discharges;
/// the coordinate join reports the lowered module's entry machine and entry
/// block so the callback placement can bind its own admission evidence.
pub fn lower_bounded_callback_identity_machine(
    checked: &CheckedTrees,
    source_machine: symbols::SymbolHandle,
    source_entry: symbols::SymbolHandle,
) -> Result<LoweredCallbackPsi, LoweringError> {
    let matching = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .filter(|selection| selection.machine == source_machine)
        .collect::<Vec<_>>();
    let [selection] = matching.as_slice() else {
        return unsupported(
            "bounded callback body must name one exact checked Terminal machine selection",
        );
    };
    let machine_row = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == source_machine)
        .ok_or(LoweringError::Unsupported(
            "bounded callback body must name one checked source machine",
        ))?;
    let entry_state =
        checked
            .machine_states(machine_row)
            .first()
            .ok_or(LoweringError::Unsupported(
                "bounded callback machine names no checked entry state",
            ))?;
    if entry_state.symbol != source_entry {
        return unsupported(
            "bounded callback entry must be the selected machine's checked entry state",
        );
    }
    let lowered = lower_terminal_selection(checked, selection)?;
    let terminal_machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .ok_or(LoweringError::Unsupported(
            "lowered callback module does not contain its entry machine",
        ))?;
    Ok(LoweredCallbackPsi {
        receipt: CallbackTerminalLoweringReceipt {
            source_machine,
            source_entry,
            terminal_machine: terminal_machine.id,
            terminal_entry: terminal_machine.entry,
        },
        terminal: lowered,
    })
}
