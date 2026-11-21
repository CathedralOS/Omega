//! The target-selected reach and the checked proposal which issued it travel together through lowering.

use native_execution_reach::ExecutionReach;
use provider_admission::provider_installation::SelectedProviderAdapter;
use semantic_vocabulary::MachineId;
use target_bound_program::TargetBoundReach;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeExecutionScope {
    reach: TargetBoundReach,
    selected_provider_closure: [u8; 32],
    source_native_bindings: Option<[u8; 32]>,
    source_provider_selection: selected_provider_plans::SelectedProviderPlanFacts,
    source_checked_entry: Option<psi_publication_records::CheckedProgramEntryTerminalReceipt>,
    local_adapters: Vec<SelectedProviderAdapter>,
    entry_call: Option<crate::SelectedProgramEntryCall>,
    callbacks: Vec<provider_correspondence::callbacks::TerminalCallbackOccurrenceProposal>,
}

/// Identity of the artifact, native entry and provider selection that a reach must retain.
pub struct NativeScopeBinding<'a> {
    artifact: terminal_codec::TerminalArtifactView<'a>,
    entry: MachineId,
    targets: psi_target_layouts::TerminalTargetContext,
    entry_call: Option<crate::SelectedProgramEntryCall>,
    proposal: &'a crate::TerminalNativeRealizationProposal,
    parent: Option<&'a crate::ScopedProviderSelection>,
}

impl<'a> NativeScopeBinding<'a> {
    pub fn checked(
        artifact: &'a terminal_codec::CanonicalTerminalArtifact,
        proposal: &'a crate::TerminalNativeRealizationProposal,
        targets: psi_target_layouts::TerminalTargetContext,
        root: terminal_psi::TargetMachineCallable,
        parent: Option<&'a crate::ScopedProviderSelection>,
    ) -> Result<Self, Vec<diagnostics::Diagnostic>> {
        use diagnostics::Diagnostic;
        let program = proposal.program_entry();
        let view = artifact.view();
        if program.checked_entry().terminal_psi_identity() != view.identity() {
            return Err(vec![Diagnostic::error(
                "native proposal belongs to another canonical artifact",
            )]);
        }
        if targets.terminal_psi() != view.identity()
            || targets.target() != proposal.target_profile().target_name()
        {
            return Err(vec![Diagnostic::error(
                "native scope belongs to another canonical artifact or target",
            )]);
        }
        let entry_call = if let Some(parent) = parent {
            parent
                .authorization()
                .require_artifact(artifact)
                .map_err(|reason| vec![Diagnostic::error(reason.to_string())])?;
            parent.scope().require_proposal_context(view, proposal)?;
            None
        } else {
            if root != proposal.target_program_entry() {
                return Err(vec![Diagnostic::error(
                    "native root scope differs from the proposal's selected program entry",
                )]);
            }
            crate::entry_settlement::bind_selected_program_entry_call(
                artifact,
                program,
                &proposal.providers().selection,
                &proposal.providers().intrinsic_executions,
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?
        };
        let entry = targets
            .entry(view.identity(), artifact.semantic_module(), root)
            .map_err(|reason| vec![Diagnostic::error(reason.to_string())])?;
        Ok(Self {
            artifact: view,
            entry,
            targets,
            entry_call,
            proposal,
            parent,
        })
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }
    pub fn entry_call(&self) -> Option<&crate::SelectedProgramEntryCall> {
        self.entry_call.as_ref()
    }
    pub fn local_adapters(&self) -> &[SelectedProviderAdapter] {
        self.parent
            .map_or(&self.proposal.providers().checked_adapters, |parent| {
                parent.scope().local_adapters()
            })
    }

    /// Seal one stage-01 traversal against the program and deployment that requested it.
    pub fn bind_target_reach(
        self,
        emitted: ExecutionReach,
        selected_candidate_positions: Vec<usize>,
    ) -> Result<NativeExecutionScope, String> {
        let local_adapters = self.local_adapters().to_vec();
        let proposal = self.proposal;
        let selection = &proposal.providers().selection;
        let reach = TargetBoundReach::bind(
            self.artifact,
            self.targets,
            proposal.target_profile(),
            self.entry,
            emitted,
            selected_candidate_positions,
            self.parent.map(|parent| parent.scope().reach()),
        )?;
        Ok(NativeExecutionScope {
            reach,
            source_provider_selection: selection.clone(),
            selected_provider_closure: *self
                .parent
                .map(|parent| parent.selection().as_ref())
                .unwrap_or(selection)
                .identity_digest()
                .as_bytes(),
            source_checked_entry: Some(proposal.program_entry().checked_entry().clone()),
            source_native_bindings: Some(proposal.providers().native_bindings.identity_digest()),
            entry_call: self.entry_call,
            local_adapters,
            callbacks: proposal.occurrences().callbacks.clone(),
        })
    }
}

impl NativeExecutionScope {
    pub fn require_source_input(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        entry: MachineId,
        profile: target::TargetProfile,
        selection: &selected_provider_plans::SelectedProviderPlanFacts,
        adapters: &[SelectedProviderAdapter],
    ) -> Result<(), &'static str> {
        if artifact.identity() != self.reach.target_context().terminal_psi()
            || entry != self.reach.entry()
            || profile.target_name() != self.reach.target_context().target()
            || selection != &self.source_provider_selection
            || adapters != self.local_adapters
        {
            return Err("native preparation differs from its checked proposal inputs");
        }
        Ok(())
    }

    pub fn callbacks(
        &self,
    ) -> &[provider_correspondence::callbacks::TerminalCallbackOccurrenceProposal] {
        &self.callbacks
    }

    pub fn reach(&self) -> &TargetBoundReach {
        &self.reach
    }

    pub(crate) fn checked_entry(&self) -> Option<&psi_publication_records::CheckedProgramEntryTerminalReceipt> {
        self.source_checked_entry.as_ref()
    }

    pub(crate) fn source_native_bindings(&self) -> Option<[u8; 32]> {
        self.source_native_bindings
    }

    pub fn source_provider_selection(&self) -> &selected_provider_plans::SelectedProviderPlanFacts {
        &self.source_provider_selection
    }

    pub fn local_adapters(&self) -> &[SelectedProviderAdapter] {
        &self.local_adapters
    }
    pub fn entry_call(&self) -> Option<&crate::SelectedProgramEntryCall> {
        self.entry_call.as_ref()
    }
    pub const fn selected_provider_closure(&self) -> [u8; 32] {
        self.selected_provider_closure
    }

    pub(crate) fn require_proposal_context(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        proposal: &crate::TerminalNativeRealizationProposal,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        self.require_source_selection(&proposal.providers().selection)?;
        if self.reach.target_context().terminal_psi() != artifact.identity()
            || self.reach.target_context().target() != proposal.target_profile().target_name()
            || self.source_checked_entry.as_ref() != Some(proposal.program_entry().checked_entry())
            || self.callbacks != proposal.occurrences().callbacks
            || self.source_native_bindings
                != Some(proposal.providers().native_bindings.identity_digest())
        {
            return Err(vec![diagnostics::Diagnostic::error(
                "native execution belongs to another checked proposal context",
            )]);
        }
        Ok(())
    }

    pub(crate) fn require_source_selection(
        &self,
        selection: &selected_provider_plans::SelectedProviderPlanFacts,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        if &self.source_provider_selection != selection {
            return Err(vec![diagnostics::Diagnostic::error(
                "native preparation belongs to another authored provider selection",
            )]);
        }
        Ok(())
    }

    pub(crate) fn bind_completed_provider_selection(
        &mut self,
        selection: &selected_provider_plans::SelectedProviderPlanFacts,
    ) {
        self.selected_provider_closure = *selection.identity_digest().as_bytes();
    }
}
