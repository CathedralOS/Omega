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
    AdmittedScalarApplications, AdmittedTargetLayouts, TerminalTargetContext,
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
pub use native_execution::{PreparedNativeExecution, requires_scalar_values};
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
    target_context: Option<TerminalTargetContext>,
    target_layout_selections: Option<AdmittedTargetLayouts>,
    external_requirement_applications: Vec<effects::SelectedExternalRequirementApplication>,
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
        artifact: terminal_installation::AuthorizedTerminalArtifact,
        target_context: Option<TerminalTargetContext>,
    ) -> Result<Self, &'static str> {
        if plan.psi != artifact.artifact().view().identity() {
            return Err("lowered plan belongs to another host-authorized artifact");
        }
        validate_cleanup_source(&plan, artifact.artifact().semantic_module())?;
        Ok(Self {
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
            admitted_assumed_bindings: Vec::new(),
            target_context,
            target_layout_selections: None,
            external_requirement_applications: Vec::new(),
            selected_provider_requirement_identities: terminal_psi::TerminalIdentityTable::from(
                Vec::new(),
            ),
            evidence: ObligationEvidence::NoSeals,
        })
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
                self.target_context.as_ref(),
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

    /// Retain exact selected external requirement applications and their identity table.
    pub fn retain_selected_providers(
        &mut self,
        external_requirement_applications: Vec<effects::SelectedExternalRequirementApplication>,
        identities: terminal_psi::TerminalIdentityTable,
        selected_provider_closure: [u8; 32],
    ) -> Result<(), &'static str> {
        if let Some(scope) = self.native_execution_scope.as_ref() {
            scope.require_input_context(
                &self.plan,
                self.target_context.as_ref(),
                selected_provider_closure,
            )?;
        }
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
            artifact: self.context.artifact,
            assumed_bindings: self.admitted_assumed_bindings,
            target_context: self.target_context,
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

    pub fn target_context(&self) -> Option<&TerminalTargetContext> {
        self.target_context.as_ref()
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
        let declaration = boundary;
        self.context.module().calling_abi_identity(declaration)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPsiOptimizationContext {
    artifact: terminal_installation::AuthorizedTerminalArtifact,
}

impl VerifiedPsiOptimizationContext {
    pub fn module(&self) -> &terminal_psi::TerminalModule {
        self.artifact.artifact().semantic_module()
    }

    pub fn closed_module(&self) -> &terminal_codec::ClosedTerminalModule {
        self.artifact.artifact().closed_module()
    }

    pub const fn authorization(&self) -> &terminal_installation::AuthorizedTerminalArtifact {
        &self.artifact
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
    artifact: terminal_installation::AuthorizedTerminalArtifact,
    program_entry: Option<SelectedProgramEntryCall>,
    assumed_bindings: Vec<InstalledAssumedBindingEvidence>,
    target_context: Option<TerminalTargetContext>,
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
                let mut entries = Vec::new();
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
                    entries.sort_unstable();
                    entries.dedup();
                    roots.push(
                        terminal_semantics::callable_values::CallableInvocationRoot {
                            caller: machine.id,
                            operation: operation.id,
                            entries,
                        },
                    );
                }
            }
        }
        roots.sort_unstable_by_key(|row| (row.caller, row.operation));
        if roots
            .windows(2)
            .any(|pair| (pair[0].caller, pair[0].operation) == (pair[1].caller, pair[1].operation))
        {
            return Err("callable entry roots have ambiguous registrar occurrences".into());
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
                match roots.binary_search_by_key(&(function.machine, *psi_operation), |row| {
                    (row.caller, row.operation)
                }) {
                    Ok(position) => {
                        let entries = &mut roots[position].entries;
                        if let Err(position) = entries.binary_search(callee) {
                            entries.insert(position, *callee);
                        }
                    }
                    Err(position) => roots.insert(
                        position,
                        terminal_semantics::callable_values::CallableInvocationRoot {
                            caller: function.machine,
                            operation: *psi_operation,
                            entries: vec![*callee],
                        },
                    ),
                }
            }
        }
        Ok(roots)
    }

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
        if let Some(selections) = &self.target_context {
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

    pub const fn authorization(&self) -> &terminal_installation::AuthorizedTerminalArtifact {
        &self.artifact
    }

    pub const fn program_entry(&self) -> Option<&SelectedProgramEntryCall> {
        self.program_entry.as_ref()
    }

    pub fn assumed_bindings(&self) -> &[InstalledAssumedBindingEvidence] {
        &self.assumed_bindings
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
