use crate::lowering_error::{LoweringError, unsupported};
use lowered_psi::LoweredPsi;

#[derive(Default, Debug, PartialEq, Eq)]
pub(crate) enum OperandProofCompletion {
    #[default]
    Validate,
    Finalize,
}

#[derive(Default, Debug, PartialEq, Eq)]
pub(crate) enum DebugPublication {
    #[default]
    FromCheckedPlan,
    Omit,
}

pub(crate) enum SourceMapping {
    EntryOnly,
    ScalarClosureOrder,
    ExactCatalog(Vec<(symbols::SymbolHandle, semantic_vocabulary::MachineId)>),
}

impl SourceMapping {
    pub(crate) fn exact_owners(
        &self,
    ) -> Option<&[(symbols::SymbolHandle, semantic_vocabulary::MachineId)]> {
        match self {
            Self::ExactCatalog(owners) => Some(owners),
            Self::EntryOnly | Self::ScalarClosureOrder => None,
        }
    }

    pub(crate) fn projection_sources(
        self,
        lowered: &LoweredPsi,
        entry: symbols::SymbolHandle,
        source_machines: &[symbols::SymbolHandle],
    ) -> Result<Vec<(symbols::SymbolHandle, semantic_vocabulary::MachineId)>, LoweringError> {
        match self {
            Self::ExactCatalog(owners) => Ok(owners),
            Self::ScalarClosureOrder => {
                if source_machines.len() != lowered.semantic_module.machines.len() {
                    return unsupported(
                        "scalar call closure source and Terminal machine tables must correspond exactly",
                    );
                }
                Ok(source_machines
                    .iter()
                    .copied()
                    .zip(
                        lowered
                            .semantic_module
                            .machines
                            .iter()
                            .map(|machine| machine.id),
                    )
                    .collect())
            }
            Self::EntryOnly => Ok(vec![(entry, lowered.semantic_module.entry)]),
        }
    }
}

/// Work remaining after a producer has assembled its machines.
#[derive(Default)]
pub(crate) struct LoweringCompletion {
    pub(crate) conformances: ConformancePublication,
    pub(crate) operands: OperandProofCompletion,
    pub(crate) debug: DebugPublication,
}

#[derive(Default)]
pub(crate) enum ConformancePublication {
    #[default]
    Reconstruct,
    ExactRoot,
    BoundedRoot,
    BoundedModule,
}

pub(crate) struct LoweredSelectedMachine {
    pub(crate) terminal: LoweredPsi,
    /// Checked source closure selected by the producer; not inferred from emitted ordinals.
    pub(crate) source_machines: Vec<symbols::SymbolHandle>,
    pub(crate) completion: LoweringCompletion,
    pub(crate) source_mapping: SourceMapping,
}

impl LoweredSelectedMachine {
    /// A lowering whose source mapping is its entry machine alone.
    pub(crate) fn entry_only(
        terminal: LoweredPsi,
        completion: LoweringCompletion,
        source_machines: Vec<symbols::SymbolHandle>,
    ) -> Self {
        Self {
            terminal,
            source_machines,
            completion,
            source_mapping: SourceMapping::EntryOnly,
        }
    }

    /// A lowering that published an exact catalog of its machine owners.
    pub(crate) fn source_mapped(
        lowered: SourceMappedLowered,
        completion: LoweringCompletion,
    ) -> Self {
        Self {
            terminal: lowered.terminal,
            source_machines: lowered
                .source_machine_ids
                .iter()
                .map(|(source, _)| *source)
                .collect(),
            completion,
            source_mapping: SourceMapping::ExactCatalog(lowered.source_machine_ids),
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
