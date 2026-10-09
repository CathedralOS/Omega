//! What one lowered machine hands back to the entrance.
//!
//! `LoweredSelectedMachine` carries the unsealed module beside the choices the
//! plan family already made about it: which source machines it closed over
//! (the exact machine-owner catalog) and whether a debug companion is published
//! (`DebugPublication`). The entrance reads those modes to decide what
//! remains to run.

use crate::lowering_error::{LoweringError, unsupported};
use lowered_psi::LoweredPsi;

#[derive(Default, Debug, PartialEq, Eq)]
pub(crate) enum DebugPublication {
    #[default]
    FromCheckedPlan,
    Omit,
}

/// Work remaining after a producer has assembled its machines.
#[derive(Default)]
pub(crate) struct LoweringCompletion {
    pub(crate) debug: DebugPublication,
}

pub(crate) struct LoweredSelectedMachine {
    pub(crate) terminal: LoweredPsi,
    /// Checked source closure selected by the producer; not inferred from emitted ordinals.
    pub(crate) source_machine_ids: Vec<(symbols::SymbolHandle, semantic_vocabulary::MachineId)>,
    pub(crate) completion: LoweringCompletion,
}

impl LoweredSelectedMachine {
    /// A lowering that published an exact catalog of its machine owners.
    pub(crate) fn source_mapped(
        lowered: SourceMappedLowered,
        completion: LoweringCompletion,
    ) -> Self {
        Self {
            terminal: lowered.terminal,
            source_machine_ids: lowered.source_machine_ids,
            completion,
        }
    }
}

pub(crate) struct SourceMappedLowered {
    pub(crate) terminal: LoweredPsi,
    /// Exact catalog owners, ordered by the emitted machine table.
    pub(crate) source_machine_ids: Vec<(symbols::SymbolHandle, semantic_vocabulary::MachineId)>,
}

impl SourceMappedLowered {
    pub(crate) fn new(
        terminal: LoweredPsi,
        sources: Vec<(symbols::SymbolHandle, semantic_vocabulary::MachineId)>,
    ) -> Result<Self, LoweringError> {
        if sources.len() != terminal.semantic_module.machines.len()
            || sources.iter().enumerate().any(|(index, (source, id))| {
                sources[..index]
                    .iter()
                    .any(|(prior_source, prior_id)| prior_source == source || prior_id == id)
            })
        {
            return unsupported("source owners do not match the exact machine catalog");
        }
        let source_machine_ids = terminal
            .semantic_module
            .machines
            .iter()
            .map(|machine| {
                sources
                    .iter()
                    .find(|(_, id)| *id == machine.id)
                    .copied()
                    .ok_or(LoweringError::Unsupported(
                        "emitted machine has no exact source owner",
                    ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            terminal,
            source_machine_ids,
        })
    }
}
