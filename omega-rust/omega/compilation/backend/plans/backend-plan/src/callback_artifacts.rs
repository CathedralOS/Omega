//! Produced callback bodies and their portable declaration joins.
//!
//! The compiler supplies produced Terminal bodies and checked placements. This
//! owner checks their artifact, registrar-operation and declaration joins;
//! native admission then borrows only executable and ABI evidence. These checks
//! do not replace callback interface admission or final object/image replay.

use crate::BoundNominalCallbackPlacement;

/// Canonical callback body. Its entry retains the declaration chosen by Psi;
/// native preparation joins that identity to the caller's portable demand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCallbackThunkArtifact {
    artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
}

impl TerminalCallbackThunkArtifact {
    pub fn new(artifact: terminal_codec::CanonicalTerminalArtifact) -> Result<Self, &'static str> {
        let module = artifact.semantic_module();
        // Callees may be present too; the artifact's entry is the callback.
        let Some(machine) = module.machine(module.entry) else {
            return Err("Terminal callback thunk artifact does not contain its entry machine");
        };
        if machine.declaration.is_none() {
            return Err("Terminal callback thunk has no checked declaration identity");
        }
        Ok(Self {
            artifact: std::sync::Arc::new(artifact),
        })
    }

    pub fn artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.artifact
    }
}

/// Exact join from one retained callback placement to the canonical Terminal
/// boundary-call operation produced from its authored registrar occurrence.
/// The thunk retains its portable declaration identity. Native
/// admission projects the executable, ABI inputs and private function identity,
/// without a source-side receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCallbackOccurrenceProposal {
    placement_index: usize,
    terminal_operation: semantic_vocabulary::OperationId,
    destination_parameter_application: Option<calling_conventions::NativeParameterApplication>,
    callback_thunk_identity: function_identity::MachineFunctionIdentity,
    callback_thunk_artifact: TerminalCallbackThunkArtifact,
}

impl TerminalCallbackOccurrenceProposal {
    pub fn new(
        placement_index: usize,
        terminal_operation: semantic_vocabulary::OperationId,
        destination_parameter_application: Option<calling_conventions::NativeParameterApplication>,
        callback_thunk_identity: function_identity::MachineFunctionIdentity,
        callback_thunk_artifact: TerminalCallbackThunkArtifact,
    ) -> Self {
        Self {
            placement_index,
            terminal_operation,
            destination_parameter_application,
            callback_thunk_identity,
            callback_thunk_artifact,
        }
    }

    pub const fn placement_index(&self) -> usize {
        self.placement_index
    }

    pub const fn terminal_operation(&self) -> semantic_vocabulary::OperationId {
        self.terminal_operation
    }

    /// The parameter application the callback's destination binds: the
    /// callback's own telescope row for a direct parameter destination, or
    /// the root record parameter for a field destination.
    pub const fn destination_parameter_application(
        &self,
    ) -> Option<&calling_conventions::NativeParameterApplication> {
        self.destination_parameter_application.as_ref()
    }

    pub const fn callback_thunk_identity(&self) -> function_identity::MachineFunctionIdentity {
        self.callback_thunk_identity
    }

    pub const fn callback_thunk_artifact(&self) -> &TerminalCallbackThunkArtifact {
        &self.callback_thunk_artifact
    }
}

/// Borrowed canonical body and placement join for one compiler-private
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
    pub boundary_entry_plan: &'artifact calling_conventions::BoundaryEntryPlan,
}

/// Join the caller's portable demand to the callback entry, then borrow native
/// inputs in occurrence order. No source arena or lowering stage is consulted.
///
/// This is the body/placement projection, not complete callback admission.
/// Callers separately validate occurrence coverage, registrar destinations,
/// the thunk's native interface, and the canonical program and proof.
pub fn admitted_native_callback_thunks<'artifact>(
    caller: terminal_codec::TerminalArtifactView<'_>,
    placements: &'artifact [BoundNominalCallbackPlacement],
    occurrences: &'artifact [TerminalCallbackOccurrenceProposal],
) -> Result<Vec<NativeCallbackThunkSettlement<'artifact>>, &'static str> {
    let module = caller.module();
    let mut admitted = Vec::with_capacity(occurrences.len());
    for occurrence in occurrences {
        let placement = placements
            .get(occurrence.placement_index())
            .ok_or("callback thunk occurrence names an absent retained placement")?;
        let thunk = occurrence.callback_thunk_artifact();
        let application = module
            .callback_applications
            .binary_search_by_key(&placement.application_index, |row| row.application_index)
            .ok()
            .map(|ordinal| &module.callback_applications[ordinal])
            .ok_or("callback thunk has no portable application")?;
        let body = thunk.artifact().semantic_module();
        let declaration = body
            .machine(body.entry)
            .and_then(|machine| machine.declaration.as_ref())
            .ok_or("callback thunk has no entry declaration")?;
        let expected_function = function_identity::MachineFunctionIdentity::callback_thunk(
            caller.identity(),
            occurrence.placement_index(),
        );
        if declaration != &application.body
            || application.registrar != occurrence.terminal_operation()
            || application.static_machine_ordinal != placement.static_machine_ordinal
            || application.requirement != placement.canonical_requirement_overload
            || application.body.contract
                != placement.resource_receipt.contract_commitment().as_bytes()
            || expected_function != Some(occurrence.callback_thunk_identity())
        {
            return Err(
                "callback thunk body or function identity drifted from its retained placement",
            );
        }
        admitted.push(NativeCallbackThunkSettlement {
            terminal_operation: occurrence.terminal_operation(),
            placement_index: occurrence.placement_index(),
            callback_function: occurrence.callback_thunk_identity(),
            artifact: thunk.artifact(),
            boundary_entry_plan: &placement.boundary_entry_plan,
        });
    }
    Ok(admitted)
}
