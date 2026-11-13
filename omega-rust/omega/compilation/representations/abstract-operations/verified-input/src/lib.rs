#![forbid(unsafe_code)]

/*
  Native lowering must use the complete set of selected providers, callbacks,
  component calls and entry calls. This owner checks that set once and keeps it
  with the lowered program, so later stages cannot substitute a smaller scope.

  The checked native proposal supplies execution coverage here. Omega 00 lowers
  the selected functions and admits realization; subsequent stages read the
  resulting input without depending on that earlier pipeline stage.
*/

use abstract_operations::AbstractOperationPlan;
use installation_evidence::InstalledAssumedBindingEvidence;
use terminal_installation::{
    AdmittedScalarApplications, AdmittedTargetLayouts, AdmittedTargetMachineSelections,
};

mod native_execution_scope;
pub use native_execution_scope::{NativeExecutionScope, NativeExecutionScopeError};

pub mod callback_settlement;
pub mod component_inputs;
pub mod entry_settlement;
pub mod native_proposal;
pub mod settlement;
pub use component_inputs::{
    AdmittedNativeComponentCall, ComponentExportInput, ComponentNativeInputs,
    RetainedComponentNativeInputs,
};
mod native_execution;
pub use backend_plan::{TerminalCallbackOccurrenceProposal, TerminalCallbackThunkArtifact};
pub use callback_settlement::{
    CallbackRegistrarOccurrence, settle_callback_registrar_imports,
    settle_native_callback_placements,
};
pub use entry_settlement::{
    NativeProgramEntrySettlement, NativeProgramEntrySettlementError, PhysicalProgramEntry,
    ValidatedNativeProgramEntrySettlement, validate_native_program_entry_settlement,
};
pub use native_execution::PreparedNativeExecution;
pub use native_proposal::{
    RetainedTerminalArtifact, TerminalNativeProposalRequest, TerminalNativeRealizationProposal,
};
pub use provider_admission::{
    InstalledInterruptNativeEntryProposal, NativeCallbackArgumentProposal, SelectedProviderAdapter,
};
pub use settlement::boundary_application_settlement::BoundaryApplicationSettlement;

mod entry_call;
pub use entry_call::SelectedProgramEntryCall;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPsiOptimizationInput {
    native_execution_scope: Option<std::sync::Arc<NativeExecutionScope>>,
    selected_provider_closure: Option<[u8; 32]>,
    scalar_applications: Option<AdmittedScalarApplications>,
    selected_program_entry_call: Option<SelectedProgramEntryCall>,
    plan: AbstractOperationPlan,
    context: VerifiedPsiOptimizationContext,
    admitted_assumed_bindings: Vec<InstalledAssumedBindingEvidence>,
    admitted_native_callbacks: provider_admission::AdmittedNativeCallbackCohort,
    admitted_installed_interrupts: provider_admission::AdmittedInstalledInterruptCohort,
    target_machine_selections: Option<AdmittedTargetMachineSelections>,
    target_layout_selections: Option<AdmittedTargetLayouts>,
    target_family_realizations: Vec<effects::SelectedTargetFamilyRealization>,
    external_requirement_applications: Vec<effects::SelectedExternalRequirementApplication>,
    selected_provider_requirement_identities: terminal_psi::TerminalIdentityTable,
    evidence: ObligationEvidence,
}

/// Where an input's obligation evidence came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObligationEvidence {
    /// Lowered from bytes: an artifact, or a module a cache rebuilt from bytes
    /// (`CanonicalTerminalArtifact::from_bytes`). Terminal Psi's bytes carry
    /// no evidence, so every accepted fact keeps the zero fingerprint. This is
    /// never an error, and no seal can be attached to such an input.
    ReadFromBytes,
    /// Seals produced beside the exact in-memory module this input was
    /// lowered from (`lower_module_with_evidence`). They are lost on any
    /// byte round trip.
    InMemory(Vec<terminal_codec::ObligationEvidenceSeal>),
}

impl VerifiedPsiOptimizationInput {
    /// The input Omega 00 retains as soon as it has lowered `plan` from
    /// `module`: no selections, callbacks or interrupts admitted yet, and
    /// evidence read from bytes. Omega 00 fills the rest through the
    /// `retain_*` and `attach_*` methods as each admission step succeeds.
    pub fn lowered(
        plan: AbstractOperationPlan,
        module: terminal_psi::TerminalModule,
        target_machine_selections: Option<AdmittedTargetMachineSelections>,
    ) -> Self {
        Self {
            native_execution_scope: None,
            selected_provider_closure: None,
            scalar_applications: None,
            selected_program_entry_call: None,
            admitted_native_callbacks: provider_admission::AdmittedNativeCallbackCohort::empty(
                plan.psi,
            ),
            admitted_installed_interrupts:
                provider_admission::AdmittedInstalledInterruptCohort::empty(plan.psi),
            plan,
            context: VerifiedPsiOptimizationContext { module },
            admitted_assumed_bindings: Vec::new(),
            target_machine_selections,
            target_layout_selections: None,
            target_family_realizations: Vec::new(),
            external_requirement_applications: Vec::new(),
            selected_provider_requirement_identities: terminal_psi::TerminalIdentityTable::from(
                Vec::new(),
            ),
            evidence: ObligationEvidence::ReadFromBytes,
        }
    }

    /// Records the selected entry call, layouts and scalar applications that
    /// native admission settled for this exact module.
    pub fn retain_native_selections(
        &mut self,
        entry_call: Option<SelectedProgramEntryCall>,
        layouts: AdmittedTargetLayouts,
        scalar_applications: Option<AdmittedScalarApplications>,
        scope: Option<std::sync::Arc<NativeExecutionScope>>,
        selected_provider_closure: [u8; 32],
    ) -> Result<(), &'static str> {
        if let Some(scope) = scope.as_ref() {
            scope.require_input_context(
                &self.plan,
                self.target_machine_selections.as_ref(),
                selected_provider_closure,
            )?;
            if scope.entry_call() != entry_call.as_ref() {
                return Err("native input changes its checked semantic entry call");
            }
        }
        if self.native_execution_scope.is_some() && self.native_execution_scope != scope {
            return Err("native input already retains a different checked execution scope");
        }
        self.selected_program_entry_call = entry_call;
        self.target_layout_selections = Some(layouts);
        self.scalar_applications = scalar_applications;
        self.native_execution_scope = scope;
        self.selected_provider_closure = Some(selected_provider_closure);
        Ok(())
    }

    /// Records the provider rows that realize target families and external
    /// requirements, with the identity table their rows index.
    pub fn retain_selected_providers(
        &mut self,
        family_realizations: Vec<effects::SelectedTargetFamilyRealization>,
        external_requirement_applications: Vec<effects::SelectedExternalRequirementApplication>,
        identities: terminal_psi::TerminalIdentityTable,
        selected_provider_closure: [u8; 32],
    ) -> Result<(), &'static str> {
        if let Some(scope) = self.native_execution_scope.as_ref() {
            scope.require_input_context(
                &self.plan,
                self.target_machine_selections.as_ref(),
                selected_provider_closure,
            )?;
        }
        self.target_family_realizations = family_realizations;
        self.external_requirement_applications = external_requirement_applications;
        self.selected_provider_requirement_identities = identities;
        self.selected_provider_closure = Some(selected_provider_closure);
        Ok(())
    }

    pub fn retain_admitted_assumed_bindings(
        &mut self,
        calls: Vec<InstalledAssumedBindingEvidence>,
    ) {
        self.admitted_assumed_bindings = calls;
    }

    pub fn retain_admitted_native_callbacks(
        &mut self,
        callbacks: provider_admission::AdmittedNativeCallbackCohort,
    ) {
        self.admitted_native_callbacks = callbacks;
    }

    pub fn retain_admitted_installed_interrupts(
        &mut self,
        entries: provider_admission::AdmittedInstalledInterruptCohort,
    ) {
        self.admitted_installed_interrupts = entries;
    }

    /// Attaches seals produced beside this exact in-memory module; the caller
    /// has checked that they name this module's identity.
    pub fn attach_in_memory_evidence(
        &mut self,
        seals: Vec<terminal_codec::ObligationEvidenceSeal>,
    ) {
        self.evidence = ObligationEvidence::InMemory(seals);
    }

    pub fn into_plan(self) -> AbstractOperationPlan {
        self.plan
    }

    pub const fn evidence(&self) -> &ObligationEvidence {
        &self.evidence
    }

    /// The seals Omega 01 checks before it fills accepted facts'
    /// fingerprints; none for an input read from bytes.
    pub fn evidence_seals(&self) -> &[terminal_codec::ObligationEvidenceSeal] {
        match &self.evidence {
            ObligationEvidence::ReadFromBytes => &[],
            ObligationEvidence::InMemory(seals) => seals,
        }
    }

    pub fn scalar_applications(&self) -> Option<&AdmittedScalarApplications> {
        self.scalar_applications.as_ref()
    }

    pub fn selected_program_entry_call(&self) -> Option<&SelectedProgramEntryCall> {
        self.selected_program_entry_call.as_ref()
    }
    pub fn admitted_assumed_bindings(&self) -> &[InstalledAssumedBindingEvidence] {
        &self.admitted_assumed_bindings
    }

    pub fn admitted_native_callbacks(
        &self,
    ) -> &[provider_admission::provider_admission::AdmittedNativeCallbackArgument] {
        self.admitted_native_callbacks.callbacks()
    }

    pub const fn admitted_installed_interrupts(
        &self,
    ) -> &provider_admission::AdmittedInstalledInterruptCohort {
        &self.admitted_installed_interrupts
    }

    pub fn into_lowering_facts(self) -> AdmittedLoweringFacts {
        AdmittedLoweringFacts {
            terminal: self.plan.psi,
            entry: self.plan.entry,
            native_execution_scope: self.native_execution_scope,
            selected_provider_closure: self.selected_provider_closure,
            program_entry: self.selected_program_entry_call,
            module: self.context.module,
            assumed_bindings: self.admitted_assumed_bindings,
            target_machine_selections: self.target_machine_selections,
            native_callbacks: self.admitted_native_callbacks,
            installed_interrupts: self.admitted_installed_interrupts,
        }
    }

    pub const fn plan(&self) -> &AbstractOperationPlan {
        &self.plan
    }

    pub const fn context(&self) -> &VerifiedPsiOptimizationContext {
        &self.context
    }

    pub fn target_machine_selections(&self) -> Option<&AdmittedTargetMachineSelections> {
        self.target_machine_selections.as_ref()
    }

    pub fn target_family_realizations(&self) -> &[effects::SelectedTargetFamilyRealization] {
        &self.target_family_realizations
    }

    pub fn selected_provider_requirement_identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.selected_provider_requirement_identities
    }

    pub fn target_layout_selections(&self) -> Option<&AdmittedTargetLayouts> {
        self.target_layout_selections.as_ref()
    }

    pub fn selected_calling_abi_identity(
        &self,
        boundary: semantic_vocabulary::BoundaryMachineId,
    ) -> Result<semantic_vocabulary::CallingAbiIdentity, String> {
        let declaration = selected_calling_boundary(
            self.context.module(),
            self.target_machine_selections(),
            boundary,
        )?;
        self.context.module().calling_abi_identity(declaration)
    }

    pub fn target_family_member(
        &self,
        boundary: semantic_vocabulary::BoundaryMachineId,
    ) -> Result<Option<terminal_psi::TargetMachineCallable>, String> {
        target_family_member(
            self.context.module(),
            self.target_machine_selections(),
            boundary,
        )
    }
}

use provider_admission::selected_calling_boundary;
use terminal_installation::admitted_boundary_member as target_family_member;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPsiOptimizationContext {
    module: terminal_psi::TerminalModule,
}

impl VerifiedPsiOptimizationContext {
    pub const fn module(&self) -> &terminal_psi::TerminalModule {
        &self.module
    }
}

// Keep the selected functions with the entry, callbacks and provider choices
// that established them. Only this input owner can separate those facts from
// its verified input; later stages borrow the sealed association.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedLoweringFacts {
    terminal: terminal_psi::TerminalPsiIdentity,
    entry: semantic_vocabulary::MachineId,
    native_execution_scope: Option<std::sync::Arc<NativeExecutionScope>>,
    selected_provider_closure: Option<[u8; 32]>,
    module: terminal_psi::TerminalModule,
    program_entry: Option<SelectedProgramEntryCall>,
    assumed_bindings: Vec<InstalledAssumedBindingEvidence>,
    target_machine_selections: Option<AdmittedTargetMachineSelections>,
    native_callbacks: provider_admission::AdmittedNativeCallbackCohort,
    installed_interrupts: provider_admission::AdmittedInstalledInterruptCohort,
}

impl AdmittedLoweringFacts {
    /// Retain private entry bindings at their real registrar operations. The call
    /// graph activates these rows only when execution reaches the registrar.
    pub fn callable_invocation_roots(
        &self,
        target: &target_operations::TargetOperationPlan,
    ) -> Result<terminal_semantics::callable_values::CallableInvocationRoots, String> {
        if target.psi != self.terminal || target.entry != self.entry {
            return Err("callable roots belong to another Terminal program or entry".into());
        }
        if let Some(selections) = &self.target_machine_selections {
            let profile = target::TargetProfile::from_canonical_target_name(selections.target())
                .map_err(|_| "callable roots name an unknown selected target")?;
            if profile.native_target() != target.target || selections.terminal_psi() != target.psi {
                return Err("callable roots belong to another native target".into());
            }
        }
        let module = self.module();
        let installed_helpers =
            provider_selection::CallbackProviderDeclarations::installed_helper_roots(module);
        let mut roots = terminal_semantics::callable_values::CallableInvocationRoots::new();
        for machine in &module.machines {
            for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
                if !matches!(
                    operation.kind,
                    terminal_psi::OperationKind::BoundaryCall { .. }
                ) {
                    continue;
                }
                let mut entries = std::collections::BTreeSet::new();
                // Admission owns which helper operations can execute. Ignore an
                // unused proposal; retain the admitted helper's exact declarations.
                if self
                    .installed_interrupts()
                    .helpers()
                    .iter()
                    .any(|helper| helper.installation() == operation.id)
                {
                    let index = installed_helpers
                        .binary_search_by_key(&operation.id, |(operation, _)| *operation)
                        .map_err(|_| "admitted interrupt installation lost its exact checked helper declarations")?;
                    entries.extend(
                        installed_helpers[index]
                            .1
                            .as_ref()
                            .map_err(Clone::clone)?
                            .iter()
                            .copied(),
                    );
                }
                entries.extend(
                    self.native_callbacks()
                        .callbacks()
                        .iter()
                        .filter(|callback| callback.terminal_operation == operation.id)
                        .map(|callback| callback.entry().machine()),
                );
                entries.extend(
                    self.installed_interrupts()
                        .entries()
                        .iter()
                        .filter(|entry| entry.operation == operation.id)
                        .map(|entry| entry.entry().machine()),
                );
                if !entries.is_empty() {
                    roots.insert((machine.id, operation.id), entries);
                }
            }
        }
        // Target lowering retains the admitted selected body for each boundary
        // invocation. Activate that body at this exact source operation, while its
        // boundary inputs remain external inputs rather than invented actual facts.
        for function in &target.functions {
            for operation in function.graph.operations() {
                let target_operations::TargetUnitOperation::Call {
                    origin,
                    psi_operation,
                    target: target_operations::NativeCallTarget::Machine(callee),
                    ..
                } = operation
                else {
                    continue;
                };
                let boundary = match origin {
                    target_operations::NativeCallOrigin::Authored => continue,
                    target_operations::NativeCallOrigin::ProgramEntry { boundary, .. }
                    | target_operations::NativeCallOrigin::TargetFamily { boundary, .. }
                    | target_operations::NativeCallOrigin::InstalledProvider { boundary, .. } => {
                        *boundary
                    }
                };
                let owner = module
                    .machine(function.machine)
                    .ok_or("selected native boundary caller is absent from the retained module")?;
                let mut sources = owner
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter(|source| source.id == *psi_operation);
                let (Some(source), None) = (sources.next(), sources.next()) else {
                    return Err(
                        "selected native boundary call has no exact source occurrence".into(),
                    );
                };
                if !matches!(source.kind, terminal_psi::OperationKind::BoundaryCall { boundary: found, .. } if found == boundary)
                    || module.machine(*callee).is_none()
                {
                    return Err(
                        "selected native boundary body does not rejoin its admitted source call"
                            .into(),
                    );
                }
                roots
                    .entry((function.machine, *psi_operation))
                    .or_default()
                    .insert(*callee);
            }
        }
        Ok(roots)
    }

    pub fn require_lowering_context(
        &self,
        plan: &AbstractOperationPlan,
        target: target::NativeTarget,
    ) -> Result<(), &'static str> {
        if self.terminal != plan.psi || self.entry != plan.entry {
            return Err("lowering facts belong to another Terminal program or entry");
        }
        if let Some(selections) = &self.target_machine_selections {
            let profile = target::TargetProfile::from_canonical_target_name(selections.target())
                .map_err(|_| "lowering facts name an unknown target")?;
            if selections.terminal_psi() != plan.psi || profile.native_target() != target {
                return Err("lowering facts belong to another target selection");
            }
        } else if self.native_execution_scope.is_some() {
            return Err("native lowering facts have no target selection custody");
        }
        Ok(())
    }

    /// Returns the functions selected for this native entry and provider choices.
    /// Absence requires full-module validation or rejection, never empty coverage.
    pub fn native_execution_scope_for(
        &self,
        terminal: terminal_psi::TerminalPsiIdentity,
        target: target::NativeTarget,
        entry: semantic_vocabulary::MachineId,
        selected_provider_closure: [u8; 32],
    ) -> Result<Option<&NativeExecutionScope>, &'static str> {
        let Some(scope) = self.native_execution_scope.as_deref() else {
            return Ok(None);
        };
        let selections = self
            .target_machine_selections
            .as_ref()
            .ok_or("native execution scope has no target selection custody")?;
        let profile = target::TargetProfile::from_canonical_target_name(selections.target())
            .map_err(|_| "native execution scope names an unknown target")?;
        if self.terminal != terminal
            || selections.terminal_psi() != terminal
            || profile.native_target() != target
            || self.entry != entry
            || self.selected_provider_closure != Some(selected_provider_closure)
        {
            return Err(
                "native execution scope belongs to another artifact, target, entry or provider selection",
            );
        }
        Ok(Some(scope))
    }

    pub const fn module(&self) -> &terminal_psi::TerminalModule {
        &self.module
    }

    pub const fn program_entry(&self) -> Option<&SelectedProgramEntryCall> {
        self.program_entry.as_ref()
    }

    pub fn assumed_bindings(&self) -> &[InstalledAssumedBindingEvidence] {
        &self.assumed_bindings
    }

    pub const fn target_machine_selections(&self) -> Option<&AdmittedTargetMachineSelections> {
        self.target_machine_selections.as_ref()
    }

    pub const fn native_callbacks(&self) -> &provider_admission::AdmittedNativeCallbackCohort {
        &self.native_callbacks
    }

    pub const fn installed_interrupts(
        &self,
    ) -> &provider_admission::AdmittedInstalledInterruptCohort {
        &self.installed_interrupts
    }
}
