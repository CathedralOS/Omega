//! Keep each entry's reachable machines with the program and provider choices used to find them.
use crate::entry_settlement::ValidatedNativeProgramEntrySettlement;
use crate::{SelectedProviderAdapter, SelectedProviderAdapters, TargetBoundReach};
use build_providers::selections::SelectedProviderPlanFacts;
use diagnostics::Diagnostic;
use native_execution_reach::ExecutionReach;
use semantic_vocabulary::MachineId;
use std::sync::Arc;
use terminal_codec::{CanonicalTerminalArtifact, TerminalArtifactIdentity, TerminalArtifactView};

/// The external nomination that justifies traversing a native body without a parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeRoot {
    Program(ValidatedNativeProgramEntrySettlement),
    Component {
        target: target::TargetProfile,
        export: psi_provider_selections::AdmittedComponentProvider,
    },
}

impl NativeRoot {
    pub fn target(&self) -> target::TargetProfile {
        match self {
            Self::Program(entry) => entry.source().target_slot().owner,
            Self::Component { target, .. } => *target,
        }
    }

    pub fn callable(&self) -> terminal_psi::TargetMachineCallable {
        match self {
            Self::Program(entry) => entry.target_callable(),
            Self::Component { export, .. } => {
                terminal_psi::TargetMachineCallable::Machine(export.candidate())
            }
        }
    }

    pub fn semantic_entry(&self) -> MachineId {
        match self {
            Self::Program(entry) => entry.checked_entry().terminal_entry(),
            Self::Component { export, .. } => export.candidate(),
        }
    }

    pub fn program(&self) -> Option<&ValidatedNativeProgramEntrySettlement> {
        match self {
            Self::Program(entry) => Some(entry),
            Self::Component { .. } => None,
        }
    }
}

/// These inputs are fixed when the native proposal is published. Private entries reuse them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeScopeSource {
    artifact: TerminalArtifactIdentity,
    root: NativeRoot,
    entry_call: Option<crate::SelectedProgramEntryCall>,
    selection: Arc<SelectedProviderPlanFacts>,
    adapters: SelectedProviderAdapters,
    callbacks: Arc<[provider_correspondence::callbacks::TerminalCallbackOccurrenceProposal]>,
    native_bindings: [u8; 32],
}

impl NativeScopeSource {
    pub fn admit(
        authorization: &psi_authorization::AuthorizedTerminalArtifact,
        root: NativeRoot,
        selection: Arc<SelectedProviderPlanFacts>,
        adapters: SelectedProviderAdapters,
        callbacks: Arc<[provider_correspondence::callbacks::TerminalCallbackOccurrenceProposal]>,
        native_bindings: [u8; 32],
        executions: &[Vec<Option<native_intrinsics::CompilerIntrinsicExecutionIdentity>>],
    ) -> Result<Self, Vec<Diagnostic>> {
        let artifact = authorization.artifact();
        adapters
            .require_selection(&selection)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let entry_call = match &root {
            NativeRoot::Program(program) => {
                if program.artifact_identity() != authorization.identity() {
                    return Err(vec![Diagnostic::error(
                        "native proposal belongs to another canonical artifact",
                    )]);
                }
                if program
                    .physical_adapter()
                    .is_some_and(|entry| entry.provider_selection != adapters.selection_digest())
                {
                    return Err(vec![Diagnostic::error(
                        "physical program entry belongs to another provider selection",
                    )]);
                }
                crate::entry_settlement::bind_selected_program_entry_call(
                    artifact, program, &selection, executions,
                )
                .map_err(|reason| vec![Diagnostic::error(reason)])?
            }
            NativeRoot::Component { export, .. } => {
                if export.provider() != artifact.view().identity() {
                    return Err(vec![Diagnostic::error(
                        "component export belongs to another supplier",
                    )]);
                }
                None
            }
        };
        Ok(Self {
            artifact: authorization.identity(),
            root,
            entry_call,
            selection,
            adapters,
            callbacks,
            native_bindings,
        })
    }

    pub const fn root(&self) -> &NativeRoot {
        &self.root
    }

    pub fn program<'a>(
        self: &'a Arc<Self>,
        authorization: &'a psi_authorization::AuthorizedTerminalArtifact,
    ) -> Result<NativeScopeBinding<'a>, Vec<Diagnostic>> {
        if self.artifact != authorization.identity() {
            return Err(vec![Diagnostic::error(
                "native proposal belongs to another canonical artifact",
            )]);
        }
        NativeScopeBinding::checked(authorization.artifact(), self, self.root.callable(), None)
    }
}

/// The internal convention selected for an assembly body and its proved scalar result view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblyCallingPlan {
    pub machine: MachineId,
    pub result: terminal_psi::AsmRegisterView,
    pub assembly_register_versions: Vec<operation_operands::values::AssemblyRegisterVersion>,
    pub call: calling_conventions::CallPlan,
}

/// A root or private entry, resolved against one target before the traversal starts.
pub struct NativeScopeBinding<'a> {
    artifact: TerminalArtifactView<'a>,
    entry: MachineId,
    targets: psi_target_layouts::TerminalTargetContext,
    source: &'a Arc<NativeScopeSource>,
    parent: Option<&'a NativeExecutionScope>,
}

impl<'a> NativeScopeBinding<'a> {
    fn checked(
        artifact: &'a CanonicalTerminalArtifact,
        source: &'a Arc<NativeScopeSource>,
        root: terminal_psi::TargetMachineCallable,
        parent: Option<&'a NativeExecutionScope>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let view = artifact.view();
        let target = source.root.target();
        let targets = psi_target_layouts::terminal_target_context(view, target.target_name());
        let entry = targets
            .entry(view.identity(), view.module(), root)
            .map_err(|reason| vec![Diagnostic::error(reason.to_string())])?;
        Ok(Self {
            artifact: view,
            entry,
            targets,
            source,
            parent,
        })
    }

    pub fn target(&self) -> &str {
        self.targets.target()
    }

    pub fn artifact(&self) -> TerminalArtifactView<'a> {
        self.artifact
    }

    pub fn callbacks(
        &self,
    ) -> &[provider_correspondence::callbacks::TerminalCallbackOccurrenceProposal] {
        &self.source.callbacks
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub fn entry_call(&self) -> Option<&crate::SelectedProgramEntryCall> {
        if self.parent.is_some() {
            None
        } else {
            self.source.entry_call.as_ref()
        }
    }

    pub fn local_adapters(&self) -> &[SelectedProviderAdapter] {
        self.source.adapters.rows()
    }

    pub fn profile(&self) -> target::TargetProfile {
        self.source.root.target()
    }

    pub fn bind_target_reach(
        self,
        emitted: ExecutionReach,
        selected_candidate_positions: Vec<usize>,
        mut assembly_returns: Vec<AssemblyCallingPlan>,
    ) -> Result<NativeExecutionScope, String> {
        assembly_returns.sort_unstable_by_key(|plan| plan.machine);
        let reach = TargetBoundReach::bind(
            self.artifact,
            self.targets,
            self.source.root.target(),
            self.entry,
            emitted,
            selected_candidate_positions,
            self.parent.map(NativeExecutionScope::reach),
        )?;
        Ok(NativeExecutionScope {
            assembly_returns,
            reach,
            source: Arc::clone(self.source),
            program_entry: self.parent.is_none(),
            selected_provider_closure: self.parent.map_or_else(
                || *self.source.adapters.selection_digest().as_bytes(),
                NativeExecutionScope::selected_provider_closure,
            ),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeExecutionScope {
    assembly_returns: Vec<AssemblyCallingPlan>,
    reach: TargetBoundReach,
    source: Arc<NativeScopeSource>,
    program_entry: bool,
    selected_provider_closure: [u8; 32],
}

impl NativeExecutionScope {
    pub fn assembly_call_plan(&self, machine: MachineId) -> Option<&AssemblyCallingPlan> {
        self.assembly_returns
            .binary_search_by_key(&machine, |plan| plan.machine)
            .ok()
            .map(|index| &self.assembly_returns[index])
    }

    pub fn assembly_return_environment(
        &self,
        artifact: TerminalArtifactView<'_>,
    ) -> Result<Option<psi_target_layouts::AssemblyReturnPlans>, &'static str> {
        if self.assembly_returns.is_empty() {
            return Ok(None);
        }
        let machines = self.assembly_returns.iter().map(|plan| plan.machine);
        psi_target_layouts::AssemblyReturnPlans::admit(
            artifact,
            self.reach.target_context(),
            machines,
        )
        .map(Some)
    }

    pub fn artifact_identity(&self) -> TerminalArtifactIdentity {
        self.source.artifact
    }
    pub fn require_source_input(
        &self,
        artifact: &CanonicalTerminalArtifact,
        entry: MachineId,
        profile: target::TargetProfile,
        selection: &build_providers::selections::SelectedProviderPlanFacts,
        adapters: &[SelectedProviderAdapter],
    ) -> Result<(), &'static str> {
        if artifact.manifest().identity() != self.artifact_identity()
            || entry != self.reach.entry()
            || profile.target_name() != self.reach.target_context().target()
            || selection != self.source.selection.as_ref()
            || adapters != self.source.adapters.rows()
        {
            return Err("native preparation differs from its checked proposal inputs");
        }
        Ok(())
    }

    pub fn callbacks(
        &self,
    ) -> &[provider_correspondence::callbacks::TerminalCallbackOccurrenceProposal] {
        &self.source.callbacks
    }

    pub fn reach(&self) -> &TargetBoundReach {
        &self.reach
    }

    pub fn checked_entry(
        &self,
    ) -> Option<&psi_publication_records::CheckedProgramEntryTerminalReceipt> {
        self.source
            .root
            .program()
            .map(|entry| entry.checked_entry())
    }

    pub fn source_native_bindings(&self) -> [u8; 32] {
        self.source.native_bindings
    }

    pub fn source_provider_selection(
        &self,
    ) -> &build_providers::selections::SelectedProviderPlanFacts {
        self.source.selection.as_ref()
    }

    pub fn local_adapters(&self) -> &[SelectedProviderAdapter] {
        self.source.adapters.rows()
    }
    pub fn entry_call(&self) -> Option<&crate::SelectedProgramEntryCall> {
        if self.program_entry {
            self.source.entry_call.as_ref()
        } else {
            None
        }
    }
    pub const fn selected_provider_closure(&self) -> [u8; 32] {
        self.selected_provider_closure
    }

    pub fn require_source(&self, source: &Arc<NativeScopeSource>) -> Result<(), Vec<Diagnostic>> {
        if self.reach.target_context().target() != source.root.target().target_name() {
            return Err(vec![Diagnostic::error(
                "native execution belongs to another target",
            )]);
        }
        if &self.source != source {
            return Err(vec![Diagnostic::error(
                "native execution belongs to another checked proposal context",
            )]);
        }
        Ok(())
    }
}

/// The authorized program and the providers selected for its reachable boundaries.
pub struct ScopedProviderSelection {
    authorization: psi_authorization::AuthorizedTerminalArtifact,
    scope: NativeExecutionScope,
    selection: Arc<SelectedProviderPlanFacts>,
}

impl ScopedProviderSelection {
    pub fn bind(
        authorization: &psi_authorization::AuthorizedTerminalArtifact,
        source: &Arc<NativeScopeSource>,
        mut scope: NativeExecutionScope,
        leaves: &[direct_boundary_bindings::DirectBoundaryLeaf],
        verified: direct_boundary_bindings::VerifiedDirectProviderSelection<'_>,
    ) -> Result<Self, Vec<Diagnostic>> {
        if authorization.identity() != source.artifact {
            return Err(vec![Diagnostic::error(
                "native proposal belongs to another canonical artifact",
            )]);
        }
        scope.require_source(source)?;
        let selection = verified
            .into_selection(
                authorization.artifact().semantic_module(),
                &source.selection,
                leaves,
                scope.reach.emitted_reach(),
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        scope.selected_provider_closure = *selection.identity_digest().as_bytes();
        Ok(Self {
            authorization: authorization.clone(),
            scope,
            selection: Arc::new(selection),
        })
    }

    pub const fn authorization(&self) -> &psi_authorization::AuthorizedTerminalArtifact {
        &self.authorization
    }
    pub const fn scope(&self) -> &NativeExecutionScope {
        &self.scope
    }
    pub const fn selection(&self) -> &Arc<SelectedProviderPlanFacts> {
        &self.selection
    }

    pub fn callable<'a>(
        &'a self,
        artifact: &'a CanonicalTerminalArtifact,
        source: &'a Arc<NativeScopeSource>,
        entry: terminal_psi::TargetMachineCallable,
    ) -> Result<NativeScopeBinding<'a>, Vec<Diagnostic>> {
        self.authorization
            .require_artifact(artifact)
            .map_err(|reason| vec![Diagnostic::error(reason.to_string())])?;
        self.scope.require_source(source)?;
        NativeScopeBinding::checked(artifact, source, entry, Some(&self.scope))
    }
}
