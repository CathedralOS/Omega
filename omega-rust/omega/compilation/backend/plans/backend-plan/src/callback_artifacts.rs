//! Produced callback bodies and their portable declaration joins.
//!
//! The compiler supplies produced Terminal bodies and checked placements. This
//! owner checks their artifact, registrar-operation and declaration joins;
//! native admission then borrows only executable and ABI evidence. These checks
//! do not replace callback interface admission or final object/image replay.

/// Callback selection in the shared canonical program. Its entry retains the
/// declaration chosen by Psi; native preparation joins that identity to the
/// caller's portable demand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCallbackThunkArtifact {
    artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    entry: semantic_vocabulary::MachineId,
    /// Dense semantic formal ordinals in the body's scalar-then-structural
    /// parameter order. These are produced from the checked entry before
    /// source erasure, never inferred from equal native shapes.
    parameter_formals: Vec<u32>,
}

impl TerminalCallbackThunkArtifact {
    pub fn new(
        artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
        entry: semantic_vocabulary::MachineId,
        scalar_source_positions: &[u32],
        structural_source_positions: &[u32],
    ) -> Result<Self, &'static str> {
        let module = artifact.semantic_module();
        let Some(machine) = module.machine(entry) else {
            return Err("Terminal callback thunk artifact does not contain its entry machine");
        };
        if machine.declaration.is_none() {
            return Err("Terminal callback thunk has no checked declaration identity");
        }
        if machine.parameters.len() != scalar_source_positions.len()
            || machine.structural_parameters.len() != structural_source_positions.len()
            || scalar_source_positions
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || structural_source_positions
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err("callback checked and Terminal entry parameter lanes disagree");
        }
        let mut runtime_positions = scalar_source_positions
            .iter()
            .chain(structural_source_positions)
            .copied()
            .collect::<Vec<_>>();
        runtime_positions.sort_unstable();
        if runtime_positions.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err("callback checked entry has duplicate runtime formal positions");
        }
        let parameter_formals = scalar_source_positions
            .iter()
            .chain(structural_source_positions)
            .map(|position| {
                runtime_positions
                    .binary_search(position)
                    .ok()
                    .and_then(|ordinal| u32::try_from(ordinal).ok())
                    .ok_or("callback entry formal cannot be represented")
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            artifact,
            entry,
            parameter_formals,
        })
    }

    pub fn artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.artifact
    }

    pub fn parameter_formals(&self) -> &[u32] {
        &self.parameter_formals
    }

    pub const fn entry(&self) -> semantic_vocabulary::MachineId {
        self.entry
    }
}

/// Exact join from one retained callback placement to the canonical Terminal
/// boundary-call operation produced from its authored registrar occurrence.
/// The thunk retains its portable declaration identity. Native
/// admission projects the executable, ABI inputs and private function identity,
/// without a source-side receipt.
///
/// `placement_index` reaches the retained placement that owns the registrar
/// plan and the private destination application; this join does not re-carry
/// that application as an independent copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCallbackOccurrenceProposal {
    placement_index: usize,
    application_index: u32,
    terminal_operation: semantic_vocabulary::OperationId,
    callback_thunk_identity: function_identity::MachineFunctionIdentity,
    callback_thunk_artifact: TerminalCallbackThunkArtifact,
    boundary_entry_plan: calling_conventions::BoundaryEntryPlan,
    entry_parameters: Vec<crate::NativeCallbackEntryParameter>,
}

impl TerminalCallbackOccurrenceProposal {
    pub fn new(
        placement_index: usize,
        application_index: u32,
        terminal_operation: semantic_vocabulary::OperationId,
        callback_thunk_identity: function_identity::MachineFunctionIdentity,
        callback_thunk_artifact: TerminalCallbackThunkArtifact,
        boundary_entry_plan: calling_conventions::BoundaryEntryPlan,
        entry_parameters: Vec<crate::NativeCallbackEntryParameter>,
    ) -> Self {
        Self {
            placement_index,
            application_index,
            terminal_operation,
            callback_thunk_identity,
            callback_thunk_artifact,
            boundary_entry_plan,
            entry_parameters,
        }
    }

    pub const fn placement_index(&self) -> usize {
        self.placement_index
    }

    pub const fn terminal_operation(&self) -> semantic_vocabulary::OperationId {
        self.terminal_operation
    }

    pub const fn callback_thunk_identity(&self) -> function_identity::MachineFunctionIdentity {
        self.callback_thunk_identity
    }

    pub const fn callback_thunk_artifact(&self) -> &TerminalCallbackThunkArtifact {
        &self.callback_thunk_artifact
    }
}

/// Borrowed canonical program, selected body and placement for one compiler-private
/// callback thunk. The ordinary callback-argument carrier remains separate so
/// target lowering cannot confuse executable body custody with a semantic
/// registrar argument.
///
/// Native identity is scoped to the caller's canonical Terminal program.
#[derive(Debug, Clone, Copy)]
pub struct NativeCallbackThunkSettlement<'artifact> {
    pub terminal_operation: semantic_vocabulary::OperationId,
    pub placement_index: usize,
    pub callback_function: function_identity::MachineFunctionIdentity,
    pub artifact: &'artifact terminal_codec::CanonicalTerminalArtifact,
    pub entry: semantic_vocabulary::MachineId,
    pub boundary_entry_plan: &'artifact calling_conventions::BoundaryEntryPlan,
    pub parameter_formals: &'artifact [u32],
    pub entry_parameters: &'artifact [crate::NativeCallbackEntryParameter],
}

/// Close produced callback bodies against the caller's portable demands.
/// The proposal producer performs this join once before retaining native inputs.
///
/// This is the body/placement projection, not complete callback admission.
/// Callers separately validate occurrence coverage, registrar destinations,
/// the thunk's native interface, and the canonical program and proof.
pub fn validate_callback_thunk_joins(
    caller: terminal_codec::TerminalArtifactView<'_>,
    occurrences: &[TerminalCallbackOccurrenceProposal],
) -> Result<(), &'static str> {
    let module = caller.module();
    for occurrence in occurrences {
        let thunk = occurrence.callback_thunk_artifact();
        if thunk.artifact().view().identity() != caller.identity() {
            return Err("callback thunk must select a body in its caller's Terminal program");
        }
        let application = module
            .callback_applications
            .binary_search_by_key(&occurrence.application_index, |row| row.application_index)
            .ok()
            .map(|ordinal| &module.callback_applications[ordinal])
            .ok_or("callback thunk has no portable application")?;
        let body = thunk.artifact().semantic_module();
        let declaration = body
            .machine(thunk.entry())
            .and_then(|machine| machine.declaration.as_ref())
            .ok_or("callback thunk has no entry declaration")?;
        let expected_function = function_identity::MachineFunctionIdentity::callback_thunk(
            caller.identity(),
            occurrence.placement_index(),
        );
        // The selected body must be the declared callback, not merely a
        // signature-compatible sibling in the shared program.
        if !declaration.same_identity_as(&body.identities, &application.body, &module.identities)
            || application.registrar != occurrence.terminal_operation()
            || expected_function != Some(occurrence.callback_thunk_identity())
        {
            return Err(
                "callback thunk body or function identity drifted from its retained placement",
            );
        }
    }
    Ok(())
}

/// Borrow complete producer-owned bodies and ABI plans in occurrence order.
/// Re-entry does not repeat the producer's declaration and registrar joins.
pub fn native_callback_thunks(
    occurrences: &[TerminalCallbackOccurrenceProposal],
) -> Vec<NativeCallbackThunkSettlement<'_>> {
    occurrences
        .iter()
        .map(|occurrence| NativeCallbackThunkSettlement {
            terminal_operation: occurrence.terminal_operation(),
            placement_index: occurrence.placement_index(),
            callback_function: occurrence.callback_thunk_identity(),
            artifact: occurrence.callback_thunk_artifact().artifact(),
            entry: occurrence.callback_thunk_artifact().entry(),
            boundary_entry_plan: &occurrence.boundary_entry_plan,
            parameter_formals: occurrence.callback_thunk_artifact().parameter_formals(),
            entry_parameters: &occurrence.entry_parameters,
        })
        .collect()
}
