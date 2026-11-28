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
use psi_target_layouts::TerminalTargetContext;
use psi_scalar_applications::AdmittedScalarApplications;

pub mod borrowed_storage;
mod emission_admission;
pub mod hosted_receiver;
pub use emission_admission::{AdmittedPhysicalEntry, EmissionAdmission};
pub mod publication;

mod callable_bindings;
pub use callable_bindings::NativeCallableBindings;
pub use target_bound_program::NativeExecutionScope;

pub mod callback_settlement;
pub mod component_inputs;
pub use target_bound_program::entry_settlement;
pub mod native_proposal;
pub mod settlement;
pub use component_inputs::{AdmittedNativeComponentCall, ComponentImport};
pub use target_bound_program::ScopedProviderSelection;
mod runtime_input;
pub mod runtime_provisions;
pub use callback_settlement::{
    CallbackRegistrarOccurrence, settle_callback_registrar_imports,
    settle_native_callback_placements,
};
pub use entry_settlement::{
    NativeProgramEntrySettlement, NativeProgramEntrySettlementError, PhysicalProgramEntry,
    ValidatedNativeProgramEntrySettlement, validate_native_program_entry_settlement,
};
pub use native_proposal::{
    NativeRootNomination, RetainedTerminalArtifact, TerminalNativeProposalRequest,
    TerminalNativeRealizationProposal,
};
pub use provider_admission::{
    InstalledInterruptNativeEntryProposal, NativeCallbackArgumentProposal, SelectedProviderAdapter,
};
pub use provider_correspondence::callbacks::{
    TerminalCallbackOccurrenceProposal, TerminalCallbackThunkArtifact,
};
pub use runtime_input::AdmittedRuntimeInput;
pub use settlement::boundary_application_settlement::BoundaryApplicationSettlement;

// Saved native inputs are read and written beside the retained artifact they
// rebuild, so a saved product is recovered without running an Omega stage.

pub use target_bound_program::SelectedProgramEntryCall;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPsiOptimizationInput {
    borrowed_storage: borrowed_storage::AdmittedBorrowedStorage,
    native_execution_scope: Option<std::sync::Arc<NativeExecutionScope>>,
    selected_provider_closure: Option<[u8; 32]>,
    scalar_applications: Option<AdmittedScalarApplications>,
    selected_program_entry_call: Option<SelectedProgramEntryCall>,
    plan: AbstractOperationPlan,
    context: VerifiedPsiOptimizationContext,
    admitted_native_callbacks: provider_admission::AdmittedNativeCallbackCohort,
    admitted_installed_interrupts: provider_admission::AdmittedInstalledInterruptCohort,
    target_context: Option<TerminalTargetContext>,
    external_requirement_applications:
        Vec<build_providers::selections::SelectedExternalRequirementApplication>,
    selected_provider_requirement_identities: terminal_psi::TerminalIdentityTable,
    evidence: ObligationEvidence,
}

/// Optional seals retained beside an already host-authorized input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObligationEvidence {
    /// Checked judgments are reused under host authorization without local
    /// seals. Accepted facts retain zero seal fingerprints; that is not a
    /// missing producer-trust decision.
    NoSeals,
    /// Seals supplied for the exact authorized module. They remain separate
    /// from host trust and are not recovered from the canonical envelope.
    Retained(Vec<terminal_codec::ObligationEvidenceSeal>),
}

// The authorization belongs to the original module, not to a plan that merely
// repeats its fingerprint. Preserve every original cleanup requirement and
// exact subject at the first native boundary, before optimization may prune edges.
fn validate_cleanup_source(
    plan: &AbstractOperationPlan,
    module: &terminal_psi::TerminalModule,
) -> Result<(), &'static str> {
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::TerminalAffineCleanupAction::InvokeOwned;
    let mut functions = BTreeSet::new();
    let mut actual = BTreeMap::new();
    let mut expected = BTreeMap::new();
    for function in &plan.functions {
        if !functions.insert(function.machine) {
            return Err("lowered cleanup custody repeats a machine");
        }
        let source = module
            .machine(function.machine)
            .ok_or("lowered cleanup custody names an absent source machine")?;
        for block in &source.blocks {
            for (edge, actions) in block.terminator.cleanup_rosters() {
                for (ordinal, action) in actions.iter().enumerate() {
                    if let InvokeOwned(cleanup) = action {
                        let ordinal = u32::try_from(ordinal)
                            .map_err(|_| "source cleanup ordinal exceeds canonical bound")?;
                        if expected
                            .insert((function.machine, edge, ordinal), cleanup)
                            .is_some()
                        {
                            return Err("source cleanup custody repeats an action owner");
                        }
                    }
                }
            }
        }
        for block in &function.blocks {
            for operation in &block.operations {
                for (edge, actions) in operation.cleanup_rosters() {
                    for (ordinal, action) in actions.iter().enumerate() {
                        if let InvokeOwned(cleanup) = action {
                            let ordinal = u32::try_from(ordinal)
                                .map_err(|_| "lowered cleanup ordinal exceeds canonical bound")?;
                            if actual
                                .insert((function.machine, edge, ordinal), cleanup)
                                .is_some()
                            {
                                return Err("lowered cleanup custody repeats an action owner");
                            }
                        }
                    }
                }
            }
        }
    }
    if actual != expected {
        return Err(
            "lowered cleanup differs from the authorized source action, subject or requirement evidence",
        );
    }
    Ok(())
}

impl VerifiedPsiOptimizationInput {
    /// Retains a lowered plan beside the exact host-authorized artifact.
    /// No selections, callbacks, interrupts or optional seals are admitted yet.
    /// Omega 00 fills the rest through the
    /// `retain_*` and `attach_*` methods as each admission step succeeds.
    pub fn lowered(
        plan: AbstractOperationPlan,
        artifact: psi_authorization::AdmittedTerminalArtifact,
        target_context: Option<TerminalTargetContext>,
    ) -> Result<Self, &'static str> {
        if plan.psi != artifact.artifact().view().identity() {
            return Err("lowered plan belongs to another host-authorized artifact");
        }
        validate_cleanup_source(&plan, artifact.artifact().semantic_module())?;
        let borrowed_storage = borrowed_storage::AdmittedBorrowedStorage::from_plan(&plan)?;
        Ok(Self {
            borrowed_storage,
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
            context: VerifiedPsiOptimizationContext { artifact },
            target_context,
            external_requirement_applications: Vec::new(),
            selected_provider_requirement_identities: terminal_psi::TerminalIdentityTable::default(
            ),
            evidence: ObligationEvidence::NoSeals,
        })
    }

    fn require_native_scope(
        &self,
        scope: &NativeExecutionScope,
        selected_provider_closure: [u8; 32],
    ) -> Result<(), &'static str> {
        let reach = scope.reach();
        if scope.artifact_identity() != self.context.artifact.identity()
            || reach.target_context().terminal_psi() != self.plan.psi
            || reach.entry() != self.plan.entry
            || Some(reach.target_context()) != self.target_context.as_ref()
            || scope.selected_provider_closure() != selected_provider_closure
        {
            return Err(
                "native execution scope belongs to another artifact, entry, target or provider selection",
            );
        }
        Ok(())
    }

    /// Records the selected entry call, scope and scalar applications that
    /// native admission settled for this exact module.
    pub fn retain_native_selections(
        &mut self,
        entry_call: Option<SelectedProgramEntryCall>,
        scalar_applications: Option<AdmittedScalarApplications>,
        scope: Option<std::sync::Arc<NativeExecutionScope>>,
        selected_provider_closure: [u8; 32],
    ) -> Result<(), &'static str> {
        if let Some(scope) = scope.as_ref() {
            self.require_native_scope(scope, selected_provider_closure)?;
            if scope.entry_call() != entry_call.as_ref() {
                return Err("native input changes its checked semantic entry call");
            }
        }
        if self.native_execution_scope.is_some() && self.native_execution_scope != scope {
            return Err("native input already retains a different checked execution scope");
        }
        self.selected_program_entry_call = entry_call;
        self.scalar_applications = scalar_applications;
        self.native_execution_scope = scope;
        self.selected_provider_closure = Some(selected_provider_closure);
        Ok(())
    }

    /// Retain exact selected external requirement applications and their identity table.
    pub fn retain_selected_providers(
        &mut self,
        external_requirement_applications: Vec<
            build_providers::selections::SelectedExternalRequirementApplication,
        >,
        identities: terminal_psi::TerminalIdentityTable,
        selected_provider_closure: [u8; 32],
    ) -> Result<(), &'static str> {
        if let Some(scope) = self.native_execution_scope.as_ref() {
            self.require_native_scope(scope, selected_provider_closure)?;
        }
        self.external_requirement_applications = external_requirement_applications;
        self.selected_provider_requirement_identities = identities;
        self.selected_provider_closure = Some(selected_provider_closure);
        Ok(())
    }

    pub fn validate_behavior_exclusions(
        &self,
        exclusions: &native_behavior_exclusions::BehaviorExclusions,
        providers: &build_providers::selections::SelectedProviderPlanFacts,
        profile: target::TargetProfile,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        if !exclusions.has_semantic_exclusions() {
            return Ok(());
        }
        let machines = match self.native_execution_scope.as_deref() {
            Some(scope) => {
                if scope.selected_provider_closure() != *providers.identity_digest().as_bytes()
                    || scope.reach().target_context().target() != profile.target_name()
                {
                    return Err(vec![diagnostics::Diagnostic::error(
                        "behavior exclusions belong to another target or selected provider closure",
                    )]);
                }
                Some(scope.reach().machines())
            }
            None => None,
        };
        exclusions.validate_semantic_scope(
            self.context.module(),
            self.plan.entry,
            machines,
            providers,
            profile.target_name(),
        )
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
    pub fn retain_evidence_seals(&mut self, seals: Vec<terminal_codec::ObligationEvidenceSeal>) {
        self.evidence = ObligationEvidence::Retained(seals);
    }

    pub fn into_plan(self) -> AbstractOperationPlan {
        self.plan
    }

    pub const fn evidence(&self) -> &ObligationEvidence {
        &self.evidence
    }

    /// The optional seals Omega 01 checks before it fills accepted facts'
    /// fingerprints; host authorization does not require local seals.
    pub fn evidence_seals(&self) -> &[terminal_codec::ObligationEvidenceSeal] {
        match &self.evidence {
            ObligationEvidence::NoSeals => &[],
            ObligationEvidence::Retained(seals) => seals,
        }
    }

    pub fn scalar_applications(&self) -> Option<&AdmittedScalarApplications> {
        self.scalar_applications.as_ref()
    }

    pub fn selected_program_entry_call(&self) -> Option<&SelectedProgramEntryCall> {
        self.selected_program_entry_call.as_ref()
    }
    /// Pair the program target lowering receives with the facts that admitted this
    /// input. `program` receives this input's plan by value, so the body never
    /// carries a plan other than the one these facts were admitted with.
    pub fn into_optimized_body<Error>(
        self,
        program: impl FnOnce(
            AbstractOperationPlan,
        ) -> Result<optimization_unit::OptimizedAbstractProgram, Error>,
    ) -> Result<OptimizedAbstractBody, Error> {
        let (plan, lowering_facts) = self.into_plan_and_lowering_facts();
        Ok(OptimizedAbstractBody {
            program: program(plan)?,
            lowering_facts,
        })
    }

    pub fn into_lowering_facts(self) -> AdmittedLoweringFacts {
        self.into_plan_and_lowering_facts().1
    }

    fn into_plan_and_lowering_facts(self) -> (AbstractOperationPlan, AdmittedLoweringFacts) {
        let facts = AdmittedLoweringFacts {
            borrowed_storage: self.borrowed_storage,
            terminal: self.plan.psi,
            entry: self.plan.entry,
            native_execution_scope: self.native_execution_scope,
            selected_provider_closure: self.selected_provider_closure,
            program_entry: self.selected_program_entry_call,
            artifact: self.context.artifact,
            target_context: self.target_context,
            native_callbacks: self.admitted_native_callbacks,
            installed_interrupts: self.admitted_installed_interrupts,
        };
        (self.plan, facts)
    }

    pub const fn plan(&self) -> &AbstractOperationPlan {
        &self.plan
    }

    pub const fn context(&self) -> &VerifiedPsiOptimizationContext {
        &self.context
    }

    pub fn target_context(&self) -> Option<&TerminalTargetContext> {
        self.target_context.as_ref()
    }

    pub fn selected_provider_requirement_identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.selected_provider_requirement_identities
    }

    pub fn selected_calling_abi_identity(
        &self,
        boundary: semantic_vocabulary::BoundaryMachineId,
    ) -> Result<semantic_vocabulary::CallingAbiIdentity, String> {
        let declaration = boundary;
        self.context.module().calling_abi_identity(declaration)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPsiOptimizationContext {
    artifact: psi_authorization::AdmittedTerminalArtifact,
}

impl VerifiedPsiOptimizationContext {
    pub fn module(&self) -> &terminal_psi::TerminalModule {
        self.artifact.artifact().semantic_module()
    }

    pub fn closed_module(&self) -> &terminal_codec::ClosedTerminalModule {
        self.artifact.artifact().closed_module()
    }

    pub const fn authorization(&self) -> &psi_authorization::AdmittedTerminalArtifact {
        &self.artifact
    }
}

// Keep the selected functions with the entry, callbacks and provider choices
// that established them. Only this input owner can separate those facts from
// its verified input; later stages borrow the sealed association.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedLoweringFacts {
    borrowed_storage: borrowed_storage::AdmittedBorrowedStorage,
    terminal: terminal_psi::TerminalPsiIdentity,
    entry: semantic_vocabulary::MachineId,
    native_execution_scope: Option<std::sync::Arc<NativeExecutionScope>>,
    selected_provider_closure: Option<[u8; 32]>,
    artifact: psi_authorization::AdmittedTerminalArtifact,
    program_entry: Option<SelectedProgramEntryCall>,
    target_context: Option<TerminalTargetContext>,
    native_callbacks: provider_admission::AdmittedNativeCallbackCohort,
    installed_interrupts: provider_admission::AdmittedInstalledInterruptCohort,
}

/// Target lowering consumes the optimized operations and the admitted facts that authorize them together.
pub struct OptimizedAbstractBody {
    program: optimization_unit::OptimizedAbstractProgram,
    lowering_facts: AdmittedLoweringFacts,
}

impl OptimizedAbstractBody {
    pub fn parts(
        &self,
    ) -> (
        &optimization_unit::OptimizedAbstractProgram,
        &AdmittedLoweringFacts,
    ) {
        (&self.program, &self.lowering_facts)
    }

    pub fn into_parts(
        self,
    ) -> (
        optimization_unit::OptimizedAbstractProgram,
        AdmittedLoweringFacts,
    ) {
        (self.program, self.lowering_facts)
    }
}

impl AdmittedLoweringFacts {
    pub fn require_lowering_context(
        &self,
        plan: &AbstractOperationPlan,
        target: target::NativeTarget,
    ) -> Result<(), &'static str> {
        if self.terminal != plan.psi
            || self.entry != plan.entry
            || self.artifact.artifact().view().identity() != self.terminal
        {
            return Err("lowering facts belong to another Terminal program or entry");
        }
        self.require_target(target)
    }

    pub fn borrowed_storage_layouts(
        &self,
        target: target::NativeTarget,
    ) -> Result<runtime_abi::NativeBorrowedStorageLayouts, &'static str> {
        self.require_target(target)?;
        Ok(self.borrowed_storage.layouts().clone())
    }

    pub fn replay_borrowed_storage_layouts(
        &self,
        target: target::NativeTarget,
    ) -> Result<runtime_abi::NativeBorrowedStorageLayouts, &'static str> {
        self.require_target(target)?;
        self.borrowed_storage.replay()
    }

    fn require_target(&self, target: target::NativeTarget) -> Result<(), &'static str> {
        if let Some(selections) = &self.target_context {
            let profile = target::TargetProfile::from_canonical_target_name(selections.target())
                .map_err(|_| "lowering facts name an unknown target")?;
            if selections.terminal_psi() != self.terminal || profile.native_target() != target {
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
            .target_context
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

    pub fn module(&self) -> &terminal_psi::TerminalModule {
        self.artifact.artifact().semantic_module()
    }

    pub fn closed_module(&self) -> &terminal_codec::ClosedTerminalModule {
        self.artifact.artifact().closed_module()
    }

    pub const fn authorization(&self) -> &psi_authorization::AdmittedTerminalArtifact {
        &self.artifact
    }

    pub const fn program_entry(&self) -> Option<&SelectedProgramEntryCall> {
        self.program_entry.as_ref()
    }

    pub const fn target_context(&self) -> Option<&TerminalTargetContext> {
        self.target_context.as_ref()
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
