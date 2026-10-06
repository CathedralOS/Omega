//! Retained callback bodies, exact placement replay, and native thunk inputs.
//!
//! The compiler supplies produced Terminal bodies and checked placements. This
//! owner checks their artifact, registrar-operation and source-selection joins;
//! native admission then borrows only executable and ABI evidence. These checks
//! do not replace callback interface admission or final object/image replay.

use crate::{
    BoundNominalCallbackPlacement, canonical_callback_private_symbol,
    canonical_callback_thunk_identity,
};

/// Canonical executable body and source-to-Terminal receipt retained beside one
/// checked callback placement. This is producer-side evidence, not a source-free
/// native input: admission checks the selected source machine and entry before
/// projecting a [`NativeCallbackThunkSettlement`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCallbackThunkArtifact {
    private_symbol: std::sync::Arc<str>,
    artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    lowering_receipt: lowered_psi::CallbackTerminalLoweringReceipt,
}

impl TerminalCallbackThunkArtifact {
    pub fn new(
        private_symbol: std::sync::Arc<str>,
        artifact: terminal_codec::CanonicalTerminalArtifact,
        lowering_receipt: lowered_psi::CallbackTerminalLoweringReceipt,
    ) -> Result<Self, &'static str> {
        if private_symbol.is_empty() {
            return Err("Terminal callback thunk has an empty private symbol");
        }
        artifact
            .validate()
            .map_err(|_| "Terminal callback thunk contains an invalid canonical artifact")?;
        let module = artifact
            .semantic_module()
            .map_err(|_| "Terminal callback thunk semantics could not be decoded")?;
        // The artifact is the isolated module of the placement's selected
        // machine; its call closure may lower additional callee machines. The
        // binding custody is the entry join, not a machine-count silhouette.
        let Some(machine) = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
        else {
            return Err("Terminal callback thunk artifact does not contain its entry machine");
        };
        if module.entry != lowering_receipt.terminal_machine
            || machine.entry != lowering_receipt.terminal_entry
        {
            return Err("Terminal callback thunk lowering receipt drifted from its artifact");
        }
        Ok(Self {
            private_symbol,
            artifact: std::sync::Arc::new(artifact),
            lowering_receipt,
        })
    }

    pub fn private_symbol(&self) -> &std::sync::Arc<str> {
        &self.private_symbol
    }

    pub fn artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.artifact
    }

    pub const fn lowering_receipt(&self) -> lowered_psi::CallbackTerminalLoweringReceipt {
        self.lowering_receipt
    }
}

/// Exact join from one retained callback placement to the canonical Terminal
/// boundary-call operation produced from its authored registrar occurrence.
/// The thunk retains source-to-Terminal evidence for placement replay. Native
/// admission projects the executable, ABI inputs and private function identity,
/// without the lowering receipt.
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
/// The producer's source-to-Terminal receipt is not a lowering input. The
/// existing private function identity still retains its source continuation
/// key; removing that cross-stage identity is a separate representation change.
#[derive(Debug, Clone, Copy)]
pub struct NativeCallbackThunkSettlement<'artifact> {
    pub terminal_operation: semantic_vocabulary::OperationId,
    pub placement_index: usize,
    pub callback_function: function_identity::MachineFunctionIdentity,
    pub private_symbol: &'artifact str,
    pub artifact: &'artifact terminal_codec::CanonicalTerminalArtifact,
    pub boundary_entry_plan: &'artifact calling_conventions::BoundaryEntryPlan,
}


/// Replay complete retained placement coverage, native destinations and exact
/// source-selection custody before a retained product can be realized.
pub fn validate_callback_occurrence_placements(
    placements: &[BoundNominalCallbackPlacement],
    occurrences: &[TerminalCallbackOccurrenceProposal],
) -> Result<(), &'static str> {
    if occurrences.len() != placements.len() {
        return Err("Terminal native proposal does not cover every retained callback placement");
    }
    for (placement_index, placement) in placements.iter().enumerate() {
        let matching = occurrences
            .iter()
            .filter(|occurrence| occurrence.placement_index == placement_index)
            .collect::<Vec<_>>();
        let [occurrence] = matching.as_slice() else {
            return Err(
                "Terminal native proposal does not uniquely bind a retained callback placement",
            );
        };
        let expected_application = placement
            .private_materialization
            .as_ref()
            .map(|materialization| materialization.destination_parameter_application())
            .transpose()
            .map_err(|_| "retained Terminal product contains an invalid callback placement")?;
        if occurrence.destination_parameter_application() != expected_application {
            return Err(
                "Terminal callback occurrence native parameter application drifted from its retained placement",
            );
        }
        let expected_thunk = canonical_callback_thunk_identity(placement_index, placement)
            .ok_or("retained callback placement cannot derive one valid callback-thunk identity")?;
        if occurrence.callback_thunk_identity() != expected_thunk {
            return Err(
                "Terminal callback occurrence thunk continuation drifted from its retained placement",
            );
        }
        let expected_symbol = canonical_callback_private_symbol(placement);
        if occurrence.callback_thunk_artifact().private_symbol() != &expected_symbol {
            return Err(
                "Terminal callback occurrence private symbol drifted from its retained placement",
            );
        }
        let receipt = occurrence.callback_thunk_artifact().lowering_receipt();
        if receipt.source_machine != placement.selected_machine
            || receipt.source_entry != placement.selected_entry
        {
            return Err(
                "Terminal callback occurrence lowering receipt drifted from its retained placement",
            );
        }
    }
    Ok(())
}

/// Recheck the retained source, symbol and function joins, then borrow native
/// inputs in occurrence order without carrying the source-to-Terminal receipt.
///
/// This is the body/placement projection, not complete callback admission.
/// Callers separately validate occurrence coverage, registrar destinations,
/// the thunk's native interface, and the canonical program and proof.
pub fn admitted_native_callback_thunks<'artifact>(
    placements: &'artifact [BoundNominalCallbackPlacement],
    occurrences: &'artifact [TerminalCallbackOccurrenceProposal],
) -> Result<Vec<NativeCallbackThunkSettlement<'artifact>>, &'static str> {
    let mut admitted = Vec::with_capacity(occurrences.len());
    for occurrence in occurrences {
        let placement = placements
            .get(occurrence.placement_index())
            .ok_or("callback thunk occurrence names an absent retained placement")?;
        let thunk = occurrence.callback_thunk_artifact();
        let receipt = thunk.lowering_receipt();
        let expected_symbol = canonical_callback_private_symbol(placement);
        let expected_function =
            canonical_callback_thunk_identity(occurrence.placement_index(), placement);
        if receipt.source_machine != placement.selected_machine
            || receipt.source_entry != placement.selected_entry
            || thunk.private_symbol() != &expected_symbol
            || expected_function != Some(occurrence.callback_thunk_identity())
        {
            return Err(
                "callback thunk body, symbol, or function identity drifted from its retained placement",
            );
        }
        admitted.push(NativeCallbackThunkSettlement {
            terminal_operation: occurrence.terminal_operation(),
            placement_index: occurrence.placement_index(),
            callback_function: occurrence.callback_thunk_identity(),
            private_symbol: thunk.private_symbol(),
            artifact: thunk.artifact(),
            boundary_entry_plan: &placement.boundary_entry_plan,
        });
    }
    Ok(admitted)
}

#[cfg(test)]
mod tests;
