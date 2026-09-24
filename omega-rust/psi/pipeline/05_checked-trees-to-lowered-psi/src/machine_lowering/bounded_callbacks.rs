//! The checked-to-Terminal coordinate join for an isolated callback body.
//!
//! A callback machine lowers through the ordinary [`super::lower_machine`]
//! route rooted at its entry — the same plan-family dispatch and retained
//! post-obligations as every other selected machine, not a list of admitted
//! body shapes — and is produced by the same Terminal production body as a
//! program entry. The callback placement separately owns its satisfaction and
//! ABI evidence and validates the produced thunk against the requirement's
//! signature and inbound entry plan; this module owns only the join that
//! reports which lowered machine and entry block the checked callback
//! coordinate became.

use checked_trees::CheckedTrees;
use lowered_psi::{CallbackTerminalLoweringReceipt, LoweredPsi};

use crate::lowering_error::{LoweringError, unsupported};

/// Join the checked callback coordinate onto `lowered`'s entry machine.
///
/// `source_machine` must be the machine `lowered` was selected from and
/// `source_entry` must be that machine's checked entry state; the callback
/// placement binds its own admission evidence to the reported Terminal
/// machine and entry block.
pub fn callback_lowering_receipt(
    checked: &CheckedTrees,
    lowered: &LoweredPsi,
    source_machine: symbols::SymbolHandle,
    source_entry: symbols::SymbolHandle,
) -> Result<CallbackTerminalLoweringReceipt, LoweringError> {
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
    let terminal_machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .ok_or(LoweringError::Unsupported(
            "lowered callback module does not contain its entry machine",
        ))?;
    Ok(CallbackTerminalLoweringReceipt {
        source_machine,
        source_entry,
        terminal_machine: terminal_machine.id,
        terminal_entry: terminal_machine.entry,
    })
}
