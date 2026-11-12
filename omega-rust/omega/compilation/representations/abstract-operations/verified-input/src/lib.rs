#![forbid(unsafe_code)]

/*
  The admitted program that Omega 00 hands to Omega 01: the lowered
  `AbstractOperationPlan`, the Terminal module it came from, and the provider,
  target, callback, interrupt and evidence selections admitted beside it.

  Omega 00 (`00_terminal-psi-to-abstract-operations`) is the only producer: it
  admits a canonical Terminal artifact, lowers it, and fills these selections
  as each admission step succeeds. Omega 01 and later realization stages read
  them. The type lives here, not in either stage, so that Omega 01 never
  depends on Omega 00 to name its own input (pipeline.md "Public contracts").
*/

use abstract_operations::AbstractOperationPlan;
use installation_evidence::InstalledAssumedBindingEvidence;
use terminal_installation::{
    AdmittedScalarApplications, AdmittedTargetLayouts, AdmittedTargetMachineSelections,
};

mod entry_call;
pub use entry_call::SelectedProgramEntryCall;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPsiOptimizationInput {
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
    ) {
        self.selected_program_entry_call = entry_call;
        self.target_layout_selections = Some(layouts);
        self.scalar_applications = scalar_applications;
    }

    /// Records the provider rows that realize target families and external
    /// requirements, with the identity table their rows index.
    pub fn retain_selected_providers(
        &mut self,
        family_realizations: Vec<effects::SelectedTargetFamilyRealization>,
        external_requirement_applications: Vec<effects::SelectedExternalRequirementApplication>,
        identities: terminal_psi::TerminalIdentityTable,
    ) {
        self.target_family_realizations = family_realizations;
        self.external_requirement_applications = external_requirement_applications;
        self.selected_provider_requirement_identities = identities;
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

    pub fn into_lowering_facts(self) -> provider_admission::AdmittedLoweringFacts {
        provider_admission::AdmittedLoweringFacts {
            program_entry: self.selected_program_entry_call.as_ref().map(|call| {
                provider_admission::LoweringEntryCall {
                    boundary: call.boundary(),
                    entry: call.entry(),
                    source_signature: call.source_signature(),
                }
            }),
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
