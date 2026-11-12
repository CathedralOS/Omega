/*
  Retained Terminal product (`RetainedTerminalArtifact`) and target-constrained
  native realization proposal (`TerminalNativeRealizationProposal`,
  `project_terminal_native_realization_proposal`).

  Joins canonical semantics, validated program-entry custody, provider demands,
  callbacks, comparisons, boundary coverage, and registrar imports once in
  memory so downstream native admission and reports consume the sealed proposal
  without replaying source joins.
*/
#[path = "retained_terminal_codec.rs"]
mod codec;
pub use codec::{
    decode_installation_assumptions, encode_installation_assumptions,
    recover_native_derivation_inputs, retain_native_derivation_inputs,
};

mod component_inputs {
    use super::{RetainedTerminalArtifact, TerminalNativeRealizationProposal};
    use calling_conventions::BoundaryCallingApplication;
    use component_description::VerifiedComponent;
    use effects::provider_plan::ProviderPlan;
    use std::sync::Arc;
    use terminal_psi::ProviderCandidateConformance;

    pub struct ComponentExportInput<'input> {
        pub candidate: &'input ProviderCandidateConformance,
        pub calling_application: &'input BoundaryCallingApplication,
    }

    pub struct ComponentNativeInputs<'input> {
        component: &'input VerifiedComponent,
        proposal: &'input TerminalNativeRealizationProposal,
        exports: Vec<ComponentExportInput<'input>>,
        plan_digest: effects::provider_plan::ProviderPlanDigest,
    }

    #[derive(Debug)]
    pub struct AdmittedNativeComponentCall<'input> {
        selected: terminal_installation::AdmittedComponentProvider,
        component: &'input VerifiedComponent,
        application: &'input BoundaryCallingApplication,
        plan_digest: effects::provider_plan::ProviderPlanDigest,
    }

    impl<'input> AdmittedNativeComponentCall<'input> {
        pub const fn facts(&self) -> provider_admission::ComponentCallFacts<'_> {
            provider_admission::ComponentCallFacts {
                selection: &self.selected,
                component: self.component,
                application: self.application,
                plan_digest: &self.plan_digest,
            }
        }

        pub const fn selection(&self) -> &terminal_installation::AdmittedComponentProvider {
            &self.selected
        }
        pub const fn component(&self) -> &'input VerifiedComponent {
            self.component
        }
        pub const fn application(&self) -> &'input BoundaryCallingApplication {
            self.application
        }
        pub const fn plan_digest(&self) -> &effects::provider_plan::ProviderPlanDigest {
            &self.plan_digest
        }

        pub(crate) fn replaces_local_adapter(
            &self,
            adapter: &crate::SelectedProviderAdapter,
        ) -> bool {
            adapter.provider_plan_digest == self.plan_digest
                && self
                    .component
                    .module()
                    .boundary_machine(self.selected.provider_boundary())
                    .and_then(|boundary| self.component.module().identities.text(boundary.identity))
                    == Some(adapter.requirement_identity.as_str())
        }
    }

    impl<'input> ComponentNativeInputs<'input> {
        pub fn admit_native_calls(
            &self,
            receiver: terminal_codec::TerminalArtifactView<'_>,
        ) -> Result<Vec<AdmittedNativeComponentCall<'input>>, String> {
            let selections = self.admit_receiver(receiver)?;
            if selections.len() != self.exports.len() {
                return Err(
                    "component native call admission changed its exact export roster".into(),
                );
            }
            Ok(selections
                .into_iter()
                .zip(&self.exports)
                .map(|(selected, export)| AdmittedNativeComponentCall {
                    selected,
                    component: self.component,
                    application: export.calling_application,
                    plan_digest: self.plan_digest,
                })
                .collect())
        }

        pub const fn component(&self) -> &'input VerifiedComponent {
            self.component
        }
        pub const fn proposal(&self) -> &'input TerminalNativeRealizationProposal {
            self.proposal
        }
        pub fn exports(&self) -> &[ComponentExportInput<'input>] {
            &self.exports
        }
        pub const fn plan_digest(&self) -> &effects::provider_plan::ProviderPlanDigest {
            &self.plan_digest
        }

        pub fn admit_receiver(
            &self,
            receiver: terminal_codec::TerminalArtifactView<'_>,
        ) -> Result<Vec<terminal_installation::AdmittedComponentProvider>, String> {
            let module = self.component.module();
            self.exports
                .iter()
                .map(|export| {
                    let requirement = module
                        .identities
                        .text(export.candidate.requirement_identity)
                        .ok_or("component export lost its exact invocation identity")?;
                    let mut receiving =
                        receiver
                            .module()
                            .boundary_machines
                            .iter()
                            .filter(|boundary| {
                                receiver.module().identities.text(boundary.identity)
                                    == Some(requirement)
                                    && receiver.module().calling_abi_identity(boundary.id).as_ref()
                                        == Ok(&export.calling_application.abi)
                            });
                    let boundary = receiving.next().ok_or_else(|| {
                    format!(
                        "component import `{requirement}` has no receiving Terminal declaration",
                    )
                })?;
                    if receiving.next().is_some() {
                        return Err(format!(
                            "component import `{requirement}` has competing Terminal declarations"
                        ));
                    }
                    let candidate = export.candidate;
                    let selected = terminal_installation::ProviderInstallationSelection {
                        boundary: candidate.boundary,
                        provider_identity: module
                            .identities
                            .text(candidate.provider_identity)
                            .ok_or("component export lost its provider identity")?
                            .to_owned(),
                        provider_declaration: module
                            .identities
                            .text(candidate.provider_declaration.identity())
                            .ok_or("component export lost its selecting declaration")?
                            .to_owned(),
                        candidate: candidate.candidate,
                    };
                    let admitted = terminal_installation::admit_component_provider(
                        receiver,
                        boundary.id,
                        self.component.artifact().view(),
                        &selected,
                    )?;
                    if !admitted.selection().contains_candidate(candidate) {
                        return Err(
                            "component export admission changed its exact checked candidate".into(),
                        );
                    }
                    Ok(admitted)
                })
                .collect()
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct RetainedComponentNativeInputs {
        component: Arc<VerifiedComponent>,
        retained: Arc<RetainedTerminalArtifact>,
        exports: Vec<(usize, usize)>,
        plan_digest: effects::provider_plan::ProviderPlanDigest,
    }

    impl RetainedComponentNativeInputs {
        pub fn borrow(&self) -> Result<ComponentNativeInputs<'_>, String> {
            let proposal = self
                .retained
                .native_realization_proposal()
                .ok_or("retained component lost its native proposal")?;
            let exports = self
                .exports
                .iter()
                .map(|&(candidate_position, application_position)| {
                    Ok(ComponentExportInput {
                        candidate: self
                            .component
                            .module()
                            .provider_candidates
                            .get(candidate_position)
                            .ok_or("retained component lost its selected export")?,
                        calling_application: proposal
                            .boundary_applications()
                            .calling
                            .get(application_position)
                            .ok_or("retained component lost its selected calling application")?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(ComponentNativeInputs {
                component: &self.component,
                proposal,
                exports,
                plan_digest: self.plan_digest,
            })
        }
        pub fn join(
            component: Arc<VerifiedComponent>,
            retained: Arc<RetainedTerminalArtifact>,
            plan: &ProviderPlan,
            selecting_declaration: &str,
            receiving_applications: &[BoundaryCallingApplication],
        ) -> Result<Self, String> {
            if component.artifact() != retained.artifact() {
                return Err(
                    "component verdict and native product retain different canonical artifacts"
                        .into(),
                );
            }
            let proposal = retained
                .native_realization_proposal()
                .ok_or("independent component has no completed native realization proposal")?;
            if proposal.target_profile().target_name() != plan.target {
                return Err(
                    "component native target differs from the selected provider plan".into(),
                );
            }
            let candidates = component
                .selected_plan_export_positions(plan, selecting_declaration)
                .map_err(|error| error.to_string())?;
            let mut exports = Vec::with_capacity(candidates.len());
            for candidate_position in candidates {
                let candidate = &component.module().provider_candidates[candidate_position];
                let requirement = component
                    .module()
                    .identities
                    .text(candidate.requirement_identity)
                    .ok_or("component export has no canonical requirement identity")?;
                let abi = component
                    .module()
                    .calling_abi_identity(candidate.boundary)?;
                let expected = plan.schema.calling_application(&abi);
                let target = proposal.target_profile().native_target();
                let (producer_position, producer) = one_application(
                    &proposal.boundary_applications().calling,
                    &abi,
                    expected,
                    target,
                    "component export",
                )?;
                let (_, receiver) = one_application(
                    receiving_applications,
                    &abi,
                    expected,
                    target,
                    "component import",
                )?;
                for application in [producer, receiver] {
                    if application.dispatch_only_parameter_count != 0 {
                        return Err(
                            "component calling application cannot omit dispatch-only formals"
                                .into(),
                        );
                    }
                    if application.recomputed_commitment()? != application.commitment {
                        return Err(format!(
                            "component calling application for `{requirement}` has a stale commitment"
                        ));
                    }
                }
                if producer != receiver {
                    return Err(format!(
                        "component import/export calling applications differ for `{requirement}`"
                    ));
                }
                exports.push((candidate_position, producer_position));
            }
            Ok(Self {
                component,
                retained,
                exports,
                plan_digest: plan.identity_digest(),
            })
        }
    }

    fn one_application<'input>(
        applications: &'input [BoundaryCallingApplication],
        abi: &semantic_vocabulary::CallingAbiIdentity,
        expected: Option<&effects::provider_plan::ServiceCallingPlan>,
        target: target::NativeTarget,
        role: &str,
    ) -> Result<(usize, &'input BoundaryCallingApplication), String> {
        let mut matching = applications.iter().enumerate().filter(|(_, row)| {
            &row.abi == abi
                && row.target == target
                && expected.map_or_else(
                    || {
                        row.plan.plan().call.policy
                            == calling_conventions::CallingPolicy::native_for_target(target)
                    },
                    |expected| row.commitment == expected.commitment.as_bytes(),
                )
        });
        let requirement = &abi.requirement_identity;
        let application = matching.next().ok_or_else(|| {
            format!("{role} `{requirement}` has no complete calling application",)
        })?;
        if matching.next().is_some() {
            return Err(format!(
                "{role} `{requirement}` has competing calling applications"
            ));
        }
        if expected
            .is_some_and(|expected| expected.report_fingerprint != application.1.report_fingerprint)
        {
            return Err(format!(
                "{role} `{requirement}` has a stale calling report coordinate"
            ));
        }
        Ok(application)
    }
}
pub use component_inputs::{
    AdmittedNativeComponentCall, ComponentExportInput, ComponentNativeInputs,
    RetainedComponentNativeInputs,
};

mod native_providers {
    use super::TerminalNativeRealizationProposal;
    use crate::providers::provider_admission::{
        NativeCompilerBuiltinSettlement, NativeProviderSettlement, PreparedSourceEvaluatedBindings,
        ProviderAdmissionRequest, SourceEvaluatedImportSettlement,
        TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy,
        prepare_source_evaluated_bindings,
    };
    use diagnostics::Diagnostic;
    use std::collections::BTreeSet;

    pub struct RetainedNativeProviderInputs<'input> {
        pub execution: &'input PreparedNativeExecution,
        pub optimization_selections: &'input optimization_core::PostTerminalOptimizationSelections,
        pub accepted_package_permissions: &'input TerminalAuthorityPermissionPolicy,
        pub imports: &'input [SourceEvaluatedImportSettlement<'input>],
    }

    pub struct PreparedNativeExecution {
        selection: effects::SelectedProviderPlanFacts,
        external_bindings: Vec<calling_conventions::ExternalBindingRow>,
        demands: BTreeSet<String>,
        compiler_builtins:
            Vec<crate::providers::provider_admission::TerminalCompilerBuiltinProposal>,
        local_adapters: Vec<crate::SelectedProviderAdapter>,
    }

    impl PreparedNativeExecution {
        pub fn lower_callable(
            &self,
            artifact: &terminal_codec::CanonicalTerminalArtifact,
            proposal: &TerminalNativeRealizationProposal,
            entry: terminal_psi::TargetMachineCallable,
            scalar_applications: Option<crate::NativeScalarApplications<'_>>,
        ) -> Result<crate::VerifiedNativeArtifactInput, Vec<Diagnostic>> {
            let selection = crate::NativeExecutionSelection {
                scalar_applications,
                providers: &self.local_adapters,
                plans: &proposal.providers().selection,
                callbacks: &proposal.occurrences().callbacks,
                entry_call: None,
            };
            let scope = crate::artifact_admission::NativeScopePreparation::prepare(
                artifact,
                proposal.target_profile(),
                entry,
                &selection,
            )
            .map_err(crate::native_admission::artifact_lowering_error)?;
            let input = scope
                .lower(artifact, &[], &selection)
                .map_err(crate::native_admission::artifact_lowering_error)?;
            input
                .bind_selected_provider_coordinates(artifact, &self.selection)
                .map_err(|reason| vec![Diagnostic::error(reason)])
        }
    }

    pub struct PreparedNativeWork<'input> {
        artifact: &'input terminal_codec::CanonicalTerminalArtifact,
        proposal: &'input TerminalNativeRealizationProposal,
        scope: crate::artifact_admission::NativeScopePreparation,
        execution: PreparedNativeExecution,
        entry_call: Option<crate::SelectedProgramEntryCall>,
        private_entry_ram: Vec<terminal_installation::CheckedPrivateEntryRamRecipe>,
    }

    impl PreparedNativeWork<'_> {
        pub fn scalar_application_demand(&self) -> &[semantic_vocabulary::ScalarApplicationId] {
            &self.scope.scalar_demand
        }

        pub fn lower(
            self,
            scalar_applications: Option<crate::NativeScalarApplications<'_>>,
        ) -> Result<(crate::VerifiedNativeArtifactInput, PreparedNativeExecution), Vec<Diagnostic>>
        {
            let input = self
                .scope
                .lower_with_private_entry_ram(
                    self.artifact,
                    &self.private_entry_ram,
                    &crate::NativeExecutionSelection {
                        scalar_applications,
                        providers: &self.execution.local_adapters,
                        plans: &self.proposal.providers().selection,
                        callbacks: &self.proposal.occurrences().callbacks,
                        entry_call: self.entry_call.as_ref(),
                    },
                )
                .map_err(crate::native_admission::artifact_lowering_error)?;
            let input = input
                .bind_selected_provider_coordinates(self.artifact, &self.execution.selection)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            Ok((input, self.execution))
        }
    }

    pub struct PreparedRetainedNativeProviders<'input> {
        execution: &'input PreparedNativeExecution,
        proposal: &'input TerminalNativeRealizationProposal,
        bindings: PreparedSourceEvaluatedBindings<'input>,
        program_demands: BTreeSet<String>,
        callbacks: &'input [crate::NativeCallbackArgumentProposal],
        pub callback_thunks: Vec<backend_plan::NativeCallbackThunkSettlement<'input>>,
        compiler_builtins: Vec<NativeCompilerBuiltinSettlement<'input>>,
    }

    impl PreparedRetainedNativeProviders<'_> {
        pub fn program_settlements(&self) -> Vec<NativeProviderSettlement<'_>> {
            self.settlements_for(&self.program_demands)
        }

        fn settlements_for(&self, demands: &BTreeSet<String>) -> Vec<NativeProviderSettlement<'_>> {
            self.bindings
                .settlements()
                .iter()
                .filter(|settlement| {
                    demands.contains(settlement.provider_execution.requirement_identity())
                })
                .copied()
                .collect()
        }

        pub fn admission_request<'request>(
            &'request self,
            settlements: &'request [NativeProviderSettlement<'request>],
            terminal_authority_policy: &'request TerminalAuthorityPolicy,
            receiving_permissions: &'request Option<TerminalAuthorityPermissionPolicy>,
            installation_assumptions: &'request [installation_evidence::InstallationEnvironmentAssumption],
        ) -> crate::NativeAdmissionRequest<'request, 'request> {
            let proposal = self.proposal;
            crate::NativeAdmissionRequest {
                private_entry_ram: &[],
                entry: terminal_psi::TargetMachineCallable::Machine(
                    proposal.program_entry().checked_entry().terminal_entry(),
                ),
                boundary_application_coverage: Some(&proposal.boundary_applications().coverage),
                behavior_exclusions: proposal.behavior_exclusions(),
                providers: ProviderAdmissionRequest {
                    profile: proposal.target_profile(),
                    bootstrap_entry: proposal.program_entry().checked_entry().terminal_entry(),
                    program_provider_context: None,
                    installation_assumptions,
                    selected_provider_plans: &self.execution.selection,
                    checked_adapters: &self.execution.local_adapters,
                    external_binding_rows: &self.execution.external_bindings,
                    settlements,
                    compiler_builtins: &self.compiler_builtins,
                    native_callbacks: self.callbacks,
                    installed_interrupts: &proposal.occurrences().installed_interrupts,
                    installed_calling: &proposal.boundary_applications().calling,
                    callback_occurrences: &proposal.occurrences().callbacks,
                    terminal_authority_policy,
                    terminal_authority_permission_policy: receiving_permissions,
                    x86_scalar_fma_provider: proposal.providers().x86_scalar_fma,
                },
            }
        }
    }

    impl TerminalNativeRealizationProposal {
        pub fn prepare_native_execution_with_components<'input>(
            &'input self,
            artifact: &'input terminal_codec::CanonicalTerminalArtifact,
            private_entry_ram: &[terminal_installation::CheckedPrivateEntryRamRecipe],
            components: &[&super::AdmittedNativeComponentCall<'_>],
        ) -> Result<PreparedNativeWork<'input>, Vec<Diagnostic>> {
            if components.iter().any(|component| {
                component.selection().receiver()
                    != &terminal_codec::TerminalExecutionIdentity::portable(
                        artifact.view().identity(),
                    )
            }) {
                return Err(vec![Diagnostic::error(
                    "component scope selection names another receiving artifact",
                )]);
            }
            for (position, component) in components.iter().enumerate() {
                if components[..position].iter().any(|earlier| {
                    earlier.selection().receiving_boundary()
                        == component.selection().receiving_boundary()
                }) || self
                    .providers()
                    .checked_adapters
                    .iter()
                    .filter(|adapter| component.replaces_local_adapter(adapter))
                    .count()
                    != 1
                {
                    return Err(vec![Diagnostic::error(
                        "component scope selection has competing or missing exact local adapter coordinates",
                    )]);
                }
            }
            let local_adapters = self
                .providers()
                .checked_adapters
                .iter()
                .filter(|adapter| {
                    !components
                        .iter()
                        .any(|component| component.replaces_local_adapter(adapter))
                })
                .cloned()
                .collect::<Vec<_>>();
            let entry = self.program_entry().as_settlement();
            let root = entry.physical_adapter().map_or(
                terminal_psi::TargetMachineCallable::Machine(
                    self.program_entry().checked_entry().terminal_entry(),
                ),
                |adapter| terminal_psi::TargetMachineCallable::Machine(adapter.machine()),
            );
            let semantic_call =
                crate::settlement::entry_settlement::bind_selected_program_entry_call(
                    artifact,
                    self.program_entry(),
                    &self.providers().selection,
                    &self.providers().intrinsic_executions,
                )
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let scope = crate::artifact_admission::NativeScopePreparation::prepare(
                artifact,
                self.target_profile(),
                root,
                &crate::NativeExecutionSelection {
                    scalar_applications: None,
                    providers: &local_adapters,
                    plans: &self.providers().selection,
                    callbacks: &self.occurrences().callbacks,
                    entry_call: semantic_call.as_ref(),
                },
            )
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "native execution input: {error:?}"
                ))]
            })?;
            let execution =
                self.prepare_native_provider_demand(artifact, &scope, local_adapters)?;
            Ok(PreparedNativeWork {
                artifact,
                proposal: self,
                scope,
                execution,
                entry_call: semantic_call,
                private_entry_ram: private_entry_ram.to_vec(),
            })
        }

        pub fn prepare_native_callable_execution<'input>(
            &'input self,
            artifact: &'input terminal_codec::CanonicalTerminalArtifact,
            entry: terminal_psi::TargetMachineCallable,
        ) -> Result<PreparedNativeWork<'input>, Vec<Diagnostic>> {
            let scope = crate::artifact_admission::NativeScopePreparation::prepare(
                artifact,
                self.target_profile(),
                entry,
                &crate::NativeExecutionSelection {
                    scalar_applications: None,
                    providers: &self.providers().checked_adapters,
                    plans: &self.providers().selection,
                    callbacks: &self.occurrences().callbacks,
                    entry_call: None,
                },
            )
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "component callable execution input: {error:?}",
                ))]
            })?;
            let execution = self.prepare_native_provider_demand(
                artifact,
                &scope,
                self.providers().checked_adapters.clone(),
            )?;
            Ok(PreparedNativeWork {
                artifact,
                proposal: self,
                scope,
                execution,
                entry_call: None,
                private_entry_ram: Vec::new(),
            })
        }

        fn prepare_native_provider_demand(
            &self,
            artifact: &terminal_codec::CanonicalTerminalArtifact,
            scope: &crate::artifact_admission::NativeScopePreparation,
            local_adapters: Vec<crate::SelectedProviderAdapter>,
        ) -> Result<PreparedNativeExecution, Vec<Diagnostic>> {
            let module = artifact.semantic_module();
            let identity = |boundary: &terminal_psi::BoundaryMachineDeclaration| {
                module
                    .identities
                    .text(boundary.identity)
                    .filter(|text| !text.is_empty())
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "native execution boundary has no exact identity",
                        )]
                    })
            };
            let mut demands = BTreeSet::new();
            let mut direct_demands = BTreeSet::new();
            let mut family_members = Vec::new();
            for boundary in module
                .boundary_machines
                .iter()
                .filter(|boundary| scope.scope.contains_boundary(boundary.id))
            {
                let name = identity(boundary)?;
                demands.insert(name.to_owned());
                if self
                    .providers()
                    .selection
                    .selected_requirement_row(name)
                    .is_some()
                {
                    continue;
                }
                match terminal_installation::admitted_boundary_member(
                    module,
                    Some(&scope.targets),
                    boundary.id,
                )
                .map_err(|reason| vec![Diagnostic::error(reason)])?
                {
                    Some(terminal_psi::TargetMachineCallable::Boundary(member)) => {
                        let member = module.boundary_machine(member).ok_or_else(|| {
                            vec![Diagnostic::error(
                                "foreign family lost its exact target member",
                            )]
                        })?;
                        direct_demands.insert(identity(member)?.to_owned());
                        family_members.push((boundary, member));
                    }
                    _ => {
                        direct_demands.insert(name.to_owned());
                    }
                }
            }
            let mut direct = Vec::new();
            let mut external_bindings = self.providers().external_bindings.clone();
            for leaf in &self.providers().direct_boundary_leaves {
                if leaf.plan.rows.iter().any(|row| {
                    direct_demands.contains(leaf.plan.identities.text(row.requirement_identity))
                }) {
                    leaf.validate_bound_calling_custody(module)
                        .map_err(|reason| vec![Diagnostic::error(reason)])?;
                    direct.push(leaf.plan.clone());
                    if let backend_plan::DirectBoundaryExecution::Foreign(binding) = &leaf.execution
                    {
                        external_bindings.push(binding.clone()?);
                    }
                }
            }
            let mut selection = self
                .providers()
                .selection
                .clone()
                .with_direct_boundary_plans(direct)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let mut family_rows = selection.family_realization_rows().to_vec();
            let mut identities = selection.identities().clone().into_builder();
            for (head, member) in family_members {
                let Some((plan, row)) = selection.selected_requirement_row(identity(member)?)
                else {
                    continue;
                };
                family_rows.push(effects::SelectedTargetFamilyRealization {
                    terminal: artifact.manifest().semantic(),
                    target: self.target_profile().target_name().to_owned(),
                    head: head.id,
                    member: member.id,
                    head_identity: identities.intern(identity(head)?),
                    member_identity: identities.intern(identity(member)?),
                    selected_requirement_identity: identities
                        .intern(plan.identities.text(row.requirement_identity)),
                    provider_plan_report_identity: plan.report_fingerprint(),
                    provider_plan_digest: plan.identity_digest(),
                });
            }
            selection = selection
                .with_family_realization_rows(family_rows, &identities.build())
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let non_intrinsic_execution = [None];
            let non_authored_realization = [None];
            let intrinsic_provenance = selection
                .plans()
                .iter()
                .map(|plan| -> Result<_, Vec<Diagnostic>> {
                    if let Some(position) = self
                        .providers()
                        .selection
                        .plans()
                        .iter()
                        .position(|original| original == plan)
                    {
                        let row_executions = self
                            .providers()
                            .intrinsic_executions
                            .get(position)
                            .ok_or_else(|| {
                                vec![Diagnostic::error(
                                    "native execution lost original intrinsic row custody",
                                )]
                            })?;
                        let row_numeric_realizations = self
                            .providers()
                            .numeric_realizations
                            .get(position)
                            .ok_or_else(|| {
                                vec![Diagnostic::error(
                                    "native execution lost selected external declaration custody",
                                )]
                            })?;
                        return Ok(
                            crate::providers::provider_admission::CompilerIntrinsicPlanProvenance {
                                plan,
                                semantic_schema: &plan.schema,
                                row_executions,
                                row_numeric_realizations,
                            },
                        );
                    }
                    let leaf = self
                        .providers()
                        .direct_boundary_leaves
                        .iter()
                        .find(|leaf| leaf.plan == *plan)
                        .ok_or_else(|| {
                            vec![Diagnostic::error(
                                "native execution has an unowned direct plan",
                            )]
                        })?;
                    let row_executions: &[Option<effects::CompilerIntrinsicExecutionIdentity>] =
                        match &leaf.execution {
                            backend_plan::DirectBoundaryExecution::Intrinsic(identity) => {
                                std::slice::from_ref(identity)
                            }
                            backend_plan::DirectBoundaryExecution::Foreign(_) => {
                                &non_intrinsic_execution
                            }
                        };
                    Ok(
                        crate::providers::provider_admission::CompilerIntrinsicPlanProvenance {
                            plan,
                            semantic_schema: &leaf.satisfied_schema,
                            row_executions,
                            row_numeric_realizations: &non_authored_realization,
                        },
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let compiler_builtins =
            crate::providers::provider_admission::derive_selected_intrinsic_settlement_proposals(
                &selection,
                intrinsic_provenance.into_iter(),
                &demands,
                &module.boundary_machines,
                &module.external_providers,
                &module.identities,
            )?;
            let demands = demands
                .into_iter()
                .map(|identity| {
                    selection
                        .selected_requirement_identity(&identity)
                        .to_owned()
                })
                .collect();
            Ok(PreparedNativeExecution {
                selection,
                external_bindings,
                demands,
                compiler_builtins,
                local_adapters,
            })
        }

        pub fn prepare_native_providers<'input>(
            &'input self,
            _artifact: &terminal_codec::CanonicalTerminalArtifact,
            inputs: RetainedNativeProviderInputs<'input>,
        ) -> Result<PreparedRetainedNativeProviders<'input>, Vec<Diagnostic>> {
            if self.post_terminal_optimizations().selections() != inputs.optimization_selections {
                return Err(diagnostics::error_in_context(
                    "Terminal native proposal",
                    "receiving lowerer selections differ from the exact post-Terminal build proposal",
                ));
            }
            crate::providers::provider_admission::terminal_authority_permissions::validate_retained_package_terminal_authority_permissions(
            self.package_terminal_authority_permissions(),
            self.package_terminal_authority_permission_identities(),
            inputs.accepted_package_permissions,
        )?;
            let callbacks = self
                .occurrences()
                .callback_arguments
                .as_deref()
                .ok_or_else(|| {
                    diagnostics::error_in_context(
                        "Terminal callback custody",
                        "callback placement has no private registrar materialization",
                    )
                })?;
            let callback_thunks =
                backend_plan::native_callback_thunks(self.occurrences().callbacks.as_slice());

            let program_demands = inputs.execution.demands.clone();
            let bindings = prepare_source_evaluated_bindings(
                inputs.execution.selection.plans(),
                &program_demands,
                inputs.imports,
                inputs.execution.selection.identities(),
            )?;

            let compiler_builtins =
                crate::providers::provider_admission::prepare_retained_compiler_builtins(
                    inputs.execution.compiler_builtins.as_slice(),
                    inputs.execution.selection.plans(),
                )?;

            Ok(PreparedRetainedNativeProviders {
                execution: inputs.execution,
                proposal: self,
                bindings,
                program_demands,
                callbacks,
                callback_thunks,
                compiler_builtins,
            })
        }
    }
}
mod proposal {
    use super::{
        TerminalNativeBodyProposal, TerminalNativeBoundaryApplications, TerminalNativeOccurrences,
        TerminalNativeProviders, TerminalNativeRealizationProposal,
    };
    use backend_plan::SelectedFamilyRealizationSource;
    use diagnostics::Diagnostic;

    pub struct TerminalNativeProposalRequest<'input> {
        pub evaluated_binding_identities: &'input terminal_psi::TerminalIdentityTable,
        pub direct_boundary_leaves: Vec<&'input backend_plan::DirectBoundaryLeaf>,
        pub provider_declarations: &'input [backend_plan::TerminalTargetProviderPlan],
        pub target_profile: Option<target::TargetProfile>,
        pub family_realizations: &'input [SelectedFamilyRealizationSource],
        pub boundaries: crate::BoundaryApplicationSettlement<'input>,
        pub native_target: Option<target::NativeTarget>,
        pub program_entry: Option<crate::NativeProgramEntrySettlement<'input>>,
        pub application: target::NativeApplicationMetadata,
        pub optimizations: optimization_core::PostTerminalOptimizationSelectionProjection,
        pub external_bindings: &'input [calling_conventions::ExternalBindingRow],
        pub behavior_exclusions: &'input [effects::AuthoredBehaviorExclusion],
        pub package_terminal_authority_permissions:
            Vec<effects::ServiceTerminalAuthorityPermission>,
        pub package_terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable,
        pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
    }

    fn close_family_realizations(
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        selections: &terminal_installation::AdmittedTargetMachineSelections,
        selected: &effects::SelectedProviderPlanFacts,
        sources: &[SelectedFamilyRealizationSource],
    ) -> Result<effects::SelectedProviderPlanFacts, Vec<Diagnostic>> {
        use terminal_psi::TargetMachineCallable;
        let error = |message: String| vec![Diagnostic::error(message)];
        let module = artifact.semantic_module();
        let mut rows = Vec::new();
        let mut row_identities = terminal_psi::TerminalIdentityTableBuilder::default();
        for source in sources {
            let mut heads = module.boundary_machines.iter().filter(|boundary| {
                boundary.declaration.as_ref().is_some_and(|declaration| {
                    module
                        .identities
                        .text(declaration.source.semantic_declaration)
                        == Some(source.head_declaration_identity.as_str())
                        && module.identities.text(declaration.source.callable)
                            == Some(source.head_callable_identity.as_str())
                })
            });
            let Some(head) = heads.next() else {
                continue;
            };
            if heads.next().is_some() {
                return Err(error(
                    "selected provider family has multiple retained heads".to_owned(),
                ));
            }
            if source.target != selections.target() {
                return Err(error(
                    "selected provider family has a different target".to_owned(),
                ));
            }
            let selected_member = selections
                .member(TargetMachineCallable::Boundary(head.id))
                .map_err(|message| error(format!("selected provider family member: {message}")))?;
            let TargetMachineCallable::Boundary(member_id) = selected_member.callable() else {
                continue;
            };
            let member = terminal_psi::find_boundary_machine(&module.boundary_machines, member_id)
                .ok_or_else(|| {
                    error("selected provider family member declaration is absent".to_owned())
                })?;
            rows.push(effects::SelectedTargetFamilyRealization {
                terminal: selections.terminal_psi(),
                target: source.target.clone(),
                head: head.id,
                member: member.id,
                head_identity: row_identities
                    .intern(module.identities.text(head.identity).unwrap_or_default()),
                member_identity: row_identities
                    .intern(module.identities.text(member.identity).unwrap_or_default()),
                selected_requirement_identity: row_identities
                    .intern(&source.selected_requirement_identity),
                provider_plan_report_identity: source.provider_plan_report_identity,
                provider_plan_digest: source.provider_plan_digest,
            });
        }
        selected
            .clone()
            .with_family_realization_rows(rows, &row_identities.build())
            .map_err(error)
    }

    pub fn project_terminal_native_realization_proposal(
        artifact: &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
        checked_program_entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
        request: TerminalNativeProposalRequest<'_>,
    ) -> Result<TerminalNativeRealizationProposal, Vec<Diagnostic>> {
        let target_profile = request.target_profile.ok_or_else(|| {
            vec![Diagnostic::error(
                "Terminal native proposal requires one selected target profile",
            )]
        })?;
        let native_target = request.native_target.ok_or_else(|| {
            vec![Diagnostic::error(
                "Terminal native proposal requires one selected native target",
            )]
        })?;
        let program_entry = request.program_entry.ok_or_else(|| {
            vec![Diagnostic::error(
                "Terminal native proposal requires one exact selected ProgramEntry",
            )]
        })?;
        if program_entry.source().target_slot().owner != target_profile {
            return Err(vec![Diagnostic::error(
                "Terminal native proposal target, profile, and ProgramEntry disagree",
            )]);
        }
        let program_entry = crate::validate_native_program_entry_settlement(
            artifact,
            &checked_program_entry,
            program_entry.with_checked_entry(&checked_program_entry),
            native_target,
        )
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "Terminal native proposal ProgramEntry settlement failed: {error}"
            ))]
        })?;
        let body = project_terminal_native_body(artifact, target_profile, &request)?;
        Ok(TerminalNativeRealizationProposal::new(
            request.application,
            program_entry,
            request.optimizations,
            body.providers,
            body.occurrences,
            body.boundary_applications,
            request.package_terminal_authority_permissions,
            request.package_terminal_authority_permission_identities,
            body.behavior_exclusions,
        ))
    }

    fn project_terminal_native_body(
        artifact: &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
        target_profile: target::TargetProfile,
        request: &TerminalNativeProposalRequest<'_>,
    ) -> Result<TerminalNativeBodyProposal, Vec<Diagnostic>> {
        let terminal_module = artifact.semantic_module();
        let targets = terminal_installation::admit_target_machine_selections(
            artifact.view(),
            target_profile.target_name(),
        );
        let selected = backend_plan::close_checked_provider_specializations(
            request.boundaries.selected_provider_plans.clone(),
            request.provider_declarations,
            terminal_module,
        )
        .map_err(|message| vec![Diagnostic::error(message)])?;
        let selected =
            close_family_realizations(artifact, &targets, &selected, request.family_realizations)?;
        task_plans::realize_task_activations(
            &terminal_module.task_activation_demands,
            &terminal_module.identities,
            &selected,
            target_profile,
        )?;
        let checked_adapters =
            crate::providers::provider_admission::retain_selected_provider_adapters(
                &selected,
                request.provider_declarations,
            )
            .map_err(|message| vec![Diagnostic::error(message)])?;
        crate::providers::provider_admission::validate_external_provider_owners(
            &terminal_module.external_providers,
            &terminal_module.identities,
            request.provider_declarations,
        )
        .map_err(|message| vec![Diagnostic::error(message)])?;
        if selected.plans().len() != request.boundaries.selected_provider_provenance.len() {
            return Err(vec![Diagnostic::error(
                "native proposal lost selected intrinsic plan custody",
            )]);
        }
        let (intrinsic_executions, numeric_realizations): (Vec<_>, Vec<_>) = selected
        .plans()
        .iter()
        .zip(&request.boundaries.selected_provider_provenance)
        .map(|(plan, retained)| {
            if plan != retained.plan
                || plan.rows.len() != retained.row_executions.len()
                || plan.rows.len() != retained.row_numeric_realizations.len()
            {
                return Err(vec![Diagnostic::error(
                    "native proposal has mismatched intrinsic row custody",
                )]);
            }
            let mut declarations = request
                .provider_declarations
                .iter()
                .filter(|declaration| declaration.plan == *plan);
            let (Some(declaration), None) = (declarations.next(), declarations.next()) else {
                return Err(vec![Diagnostic::error(
                    "native intrinsic provenance has no unique whole-plan declaration custody",
                )]);
            };
            if declaration.row_numeric_realizations.as_slice() != retained.row_numeric_realizations
            {
                return Err(vec![Diagnostic::error(
                    "native intrinsic provenance substituted its selected external declarations",
                )]);
            }
            Ok((
                retained
                    .row_executions
                    .iter()
                    .map(|execution| execution.filter(|execution| !execution.is_numeric()))
                    .collect::<Vec<_>>(),
                retained.row_numeric_realizations.to_vec(),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .unzip();
        let callback_placements = crate::settle_native_callback_placements(
            terminal_module,
            Some(&targets),
            &selected,
            &request.boundaries.calling_applications,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let mut callback_occurrences = project_callback_occurrences(
            &callback_placements,
            artifact,
            &selected,
            &checked_adapters,
        )?;
        let callback_arguments =
            crate::providers::provider_admission::project_native_callback_arguments(
                &callback_placements,
                &callback_occurrences,
            )?;
        let boundary_application_coverage = request.boundaries.settle(artifact, &selected)?;
        let comparisons = crate::settlement::comparison_settlement::settle_comparison_occurrences(
            terminal_module,
            &selected,
            &boundary_application_coverage,
        )?;
        let external_binding_rows = crate::settle_callback_registrar_imports(
            terminal_module,
            Some(&targets),
            &selected,
            &callback_placements,
            callback_occurrences
                .iter()
                .map(|occurrence| crate::CallbackRegistrarOccurrence {
                    placement_index: occurrence.placement_index(),
                    terminal_operation: occurrence.terminal_operation(),
                }),
            &request.boundaries.calling_applications,
            request.external_bindings,
        )
        .map_err(|message| vec![Diagnostic::error(message)])?;
        let installed_placements =
            crate::settlement::callback_settlement::settle_installed_callback_placements(
                terminal_module,
                target_profile,
                Some(&targets),
                &request.boundaries.calling_applications,
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let mut installed_interrupts = Vec::with_capacity(installed_placements.len());
        for (ordinal, placement) in installed_placements.into_iter().enumerate() {
            let placement_index =
                callback_placements
                    .len()
                    .checked_add(ordinal)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "installed callback placement index overflow",
                        )]
                    })?;
            let occurrence = callback_occurrence(placement_index, &placement, artifact)?;
            installed_interrupts.push(crate::InstalledInterruptNativeEntryProposal {
                operation: occurrence.terminal_operation(),
                placement_index,
                callback_function: occurrence.callback_thunk_identity(),
                placement,
            });
            callback_occurrences.push(occurrence);
        }
        backend_plan::validate_callback_thunk_joins(artifact.view(), &callback_occurrences)
            .map_err(|message| vec![Diagnostic::error(message)])?;
        Ok(TerminalNativeBodyProposal {
            providers: TerminalNativeProviders {
                direct_boundary_leaves: bind_direct_boundary_leaves(
                    terminal_module,
                    &request.direct_boundary_leaves,
                )?,
                selection: selected,
                declarations: request.provider_declarations.to_vec(),
                checked_adapters,
                external_bindings: external_binding_rows,
                intrinsic_executions,
                numeric_realizations,
                x86_scalar_fma: request.x86_scalar_fma_provider,
            },
            occurrences: TerminalNativeOccurrences {
                callbacks: callback_occurrences,
                installed_interrupts,
                callback_arguments,
                ieee_float_comparisons: comparisons.ieee_float,
                integer_comparisons: comparisons.integer,
            },
            boundary_applications: TerminalNativeBoundaryApplications {
                calling: request.boundaries.calling_applications.clone(),
                coverage: boundary_application_coverage,
            },
            behavior_exclusions: effects::authored_behavior_exclusion_set_in(
                request.behavior_exclusions,
                terminal_module,
            ),
        })
    }

    fn bind_direct_boundary_leaves(
        module: &terminal_psi::TerminalModule,
        candidates: &[&backend_plan::DirectBoundaryLeaf],
    ) -> Result<Vec<backend_plan::DirectBoundaryLeaf>, Vec<Diagnostic>> {
        let mut bound = Vec::new();
        for candidate in candidates {
            let mut declarations = module.boundary_machines.iter().filter(|boundary| {
                boundary.declaration.as_ref().is_some_and(|declaration| {
                    module
                        .identities
                        .text(declaration.source.semantic_declaration)
                        == Some(candidate.declaration_identity.as_str())
                        && module.identities.text(declaration.source.callable)
                            == Some(candidate.callable_identity.as_str())
                })
            });
            let Some(boundary) = declarations.next() else {
                continue;
            };
            if declarations.next().is_some() {
                return Err(vec![Diagnostic::error(
                    "direct boundary leaf has multiple exact Terminal declarations",
                )]);
            }
            let identity = module
                .identities
                .text(boundary.identity)
                .filter(|text| !text.is_empty())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "direct boundary leaf has no Terminal boundary identity",
                    )]
                })?;
            let mut leaf = (*candidate).clone();
            let declaration = std::sync::Arc::make_mut(&mut leaf.plan.schema.declaration);
            let terminal_requirement = declaration.identities.intern(identity);
            declaration.trait_name = terminal_requirement;
            leaf.plan.rows[0].requirement_identity = leaf.plan.identities.intern(identity);
            declaration.methods[0].requirement_identity = terminal_requirement;
            if let backend_plan::DirectBoundaryExecution::Foreign(Ok(binding)) = &mut leaf.execution
            {
                binding.trait_name = identity.to_owned();
                binding.requirement_identity = identity.to_owned();
            }
            leaf.validate_bound_calling_custody(module)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            bound.push(leaf);
        }
        Ok(bound)
    }

    fn project_callback_occurrences(
        callback_placements: &[backend_plan::NativeCallbackPlacement],
        artifact: &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
        selected: &effects::SelectedProviderPlanFacts,
        checked_adapters: &[crate::SelectedProviderAdapter],
    ) -> Result<Vec<crate::TerminalCallbackOccurrenceProposal>, Vec<Diagnostic>> {
        let caller = artifact.view();
        let module = caller.module();
        let mut application_indices = callback_placements
            .iter()
            .map(|placement| placement.application_index)
            .collect::<Vec<_>>();
        application_indices.sort_unstable();
        let operations = crate::settlement::callback_settlement::CallbackTerminalOperations::new(
            module
                .callback_applications
                .iter()
                .map(|application| application.registrar),
            module.operations(),
        );
        for application in &module.callback_applications {
            let first = application_indices.partition_point(|application_index| {
                *application_index < application.application_index
            });
            let last = application_indices.partition_point(|application_index| {
                *application_index <= application.application_index
            });
            let operation = operations.find(application.registrar).map_err(|_| {
                vec![Diagnostic::error(
                    "callback application has no unique canonical registrar operation",
                )]
            })?;
            let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = operation.kind else {
                return Err(vec![Diagnostic::error(
                    "callback application registrar is not a boundary operation",
                )]);
            };
            let declaration = module.boundary_machine(boundary).ok_or_else(|| {
                vec![Diagnostic::error(
                    "callback registrar has no exact boundary declaration",
                )]
            })?;
            let requirement = module
                .identities
                .text(declaration.identity)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "callback registrar has no exact boundary identity",
                    )]
                })?;
            let Some((plan, row)) = selected.selected_requirement_row(requirement) else {
                if last != first {
                    return Err(vec![Diagnostic::error(
                        "unselected callback registrar retained a native placement",
                    )]);
                }
                continue;
            };
            if let effects::provider_plan::ProviderBinding::CheckedAdapter {
                machine_identity,
                ..
            } = &row.binding
            {
                let machine = selected
                    .specialized_requirement_row(requirement)
                    .map_or_else(
                        || plan.identity_text(*machine_identity),
                        |specialization| {
                            selected
                                .identities()
                                .text(specialization.machine_identity)
                                .unwrap_or_default()
                        },
                    );
                if machine.is_empty() {
                    return Err(vec![Diagnostic::error(
                        "ordinary callback forwarding has no exact selected machine identity",
                    )]);
                }
                let mut adapters = checked_adapters.iter().filter(|adapter| {
                    adapter.requirement_identity == requirement
                        && adapter.selected_requirement_identity
                            == plan.identities.text(row.requirement_identity)
                        && adapter.provider_plan_digest == plan.identity_digest()
                        && adapter.machine_identity == machine
                });
                if adapters.next().is_none() || adapters.next().is_some() || last != first {
                    return Err(vec![Diagnostic::error(
                        "ordinary callback forwarding lacks its exact selected checked body or retains a native registrar placement",
                    )]);
                }
                continue;
            }
            if last - first != 1 {
                return Err(vec![Diagnostic::error(
                    "Terminal callback application has no unique checked placement",
                )]);
            }
        }
        let occurrences = callback_placements
            .iter()
            .enumerate()
            .map(|(placement_index, placement)| {
                callback_occurrence(placement_index, placement, artifact)
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        Ok(occurrences)
    }

    fn callback_occurrence(
        placement_index: usize,
        placement: &backend_plan::NativeCallbackPlacement,
        artifact: &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    ) -> Result<crate::TerminalCallbackOccurrenceProposal, Vec<Diagnostic>> {
        let caller = artifact.view();
        let module = caller.module();
        let application = module
            .callback_application(placement.application_index)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "callback placement {placement_index} has no Terminal application",
                ))]
            })?;
        if application.static_machine_ordinal != placement.static_machine_ordinal
            || application.requirement != placement.canonical_requirement_overload
            || application.body.contract != placement.contract_commitment
        {
            return Err(vec![Diagnostic::error(
                "callback placement disagrees with its checked Terminal application",
            )]);
        }
        let callback_thunk_identity = function_identity::MachineFunctionIdentity::callback_thunk(
        caller.identity(),
        placement_index,
    )
    .ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "callback placement {placement_index} cannot derive one valid callback-thunk identity",
        ))]
    })?;
        let callback_thunk_artifact = crate::TerminalCallbackThunkArtifact::for_application(
            std::sync::Arc::clone(artifact),
            placement.application_index,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        Ok(crate::TerminalCallbackOccurrenceProposal::new(
            placement_index,
            placement.application_index,
            application.registrar,
            callback_thunk_identity,
            callback_thunk_artifact,
            placement.boundary_entry_plan.clone(),
            placement.entry_parameters.clone(),
        ))
    }
}

pub use proposal::{TerminalNativeProposalRequest, project_terminal_native_realization_proposal};

pub use backend_plan::{SelectedFamilyRealizationSource, TerminalTargetProviderProposal};
pub use backend_plan::{TerminalCallbackOccurrenceProposal, TerminalCallbackThunkArtifact};
pub use native_providers::{
    PreparedNativeExecution, PreparedNativeWork, PreparedRetainedNativeProviders,
    RetainedNativeProviderInputs,
};

pub use crate::providers::provider_admission::TerminalCompilerBuiltinProposal;

pub use boundary_applications::{
    TerminalIeeeFloatComparisonOccurrenceProposal, TerminalIntegerComparisonOccurrenceProposal,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeProviders {
    pub direct_boundary_leaves: Vec<backend_plan::DirectBoundaryLeaf>,
    pub selection: effects::SelectedProviderPlanFacts,
    pub declarations: Vec<backend_plan::TerminalTargetProviderPlan>,
    pub checked_adapters: Vec<crate::SelectedProviderAdapter>,
    pub external_bindings: Vec<calling_conventions::ExternalBindingRow>,
    pub intrinsic_executions: Vec<Vec<Option<effects::CompilerIntrinsicExecutionIdentity>>>,
    pub numeric_realizations: Vec<Vec<Option<String>>>,
    pub x86_scalar_fma: Option<target::AdmittedX86ScalarFmaProvider>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeOccurrences {
    pub callbacks: Vec<TerminalCallbackOccurrenceProposal>,
    pub installed_interrupts: Vec<crate::InstalledInterruptNativeEntryProposal>,
    pub callback_arguments: Option<Vec<crate::NativeCallbackArgumentProposal>>,
    pub ieee_float_comparisons: Vec<TerminalIeeeFloatComparisonOccurrenceProposal>,
    pub integer_comparisons: Vec<TerminalIntegerComparisonOccurrenceProposal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeBoundaryApplications {
    pub calling: Vec<calling_conventions::BoundaryCallingApplication>,
    pub coverage: boundary_applications::TerminalBoundaryApplicationCoverage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TerminalNativeBodyProposal {
    providers: TerminalNativeProviders,
    occurrences: TerminalNativeOccurrences,
    boundary_applications: TerminalNativeBoundaryApplications,
    behavior_exclusions: effects::BehaviorExclusions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeRealizationProposal {
    application: target::NativeApplicationMetadata,
    program_entry: crate::ValidatedNativeProgramEntrySettlement,
    post_terminal_optimizations: optimization_core::PostTerminalOptimizationSelectionProjection,
    providers: TerminalNativeProviders,
    occurrences: TerminalNativeOccurrences,
    boundary_applications: TerminalNativeBoundaryApplications,
    package_terminal_authority_permissions: Vec<effects::ServiceTerminalAuthorityPermission>,
    package_terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable,
    behavior_exclusions: effects::BehaviorExclusions,
    target_provider_proposals: Vec<backend_plan::TerminalTargetProviderProposal>,
}

impl TerminalNativeRealizationProposal {
    pub fn bind_physical_adapter(
        &mut self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
    ) -> Result<(), &'static str> {
        let application = self
            .program_entry
            .physical_calling_application()
            .ok_or("physical adapter selected without a physical arrival contract")?;
        let physical = crate::settlement::entry_settlement::PhysicalProgramEntry::bind(
            artifact,
            application,
            &self.providers.checked_adapters,
        )?;
        self.program_entry.physical_adapter = Some(physical);
        Ok(())
    }

    pub(crate) fn new(
        application: target::NativeApplicationMetadata,
        program_entry: crate::ValidatedNativeProgramEntrySettlement,
        post_terminal_optimizations: optimization_core::PostTerminalOptimizationSelectionProjection,
        providers: TerminalNativeProviders,
        occurrences: TerminalNativeOccurrences,
        boundary_applications: TerminalNativeBoundaryApplications,
        mut package_terminal_authority_permissions: Vec<
            effects::ServiceTerminalAuthorityPermission,
        >,
        package_terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable,
        behavior_exclusions: effects::BehaviorExclusions,
    ) -> Self {
        package_terminal_authority_permissions.sort_by(|left, right| {
            left.service_schema()
                .as_bytes()
                .cmp(right.service_schema().as_bytes())
                .then_with(|| {
                    package_terminal_authority_permission_identities
                        .text(left.requirement_identity())
                        .cmp(
                            &package_terminal_authority_permission_identities
                                .text(right.requirement_identity()),
                        )
                })
        });
        Self {
            target_provider_proposals: Vec::new(),
            application,
            program_entry,
            post_terminal_optimizations,
            providers,
            occurrences,
            boundary_applications,
            package_terminal_authority_permissions,
            package_terminal_authority_permission_identities,
            behavior_exclusions,
        }
    }

    pub const fn target_profile(&self) -> target::TargetProfile {
        self.program_entry.source().target_slot().owner
    }

    pub fn with_target_provider_proposals(
        mut self,
        proposals: Vec<backend_plan::TerminalTargetProviderProposal>,
    ) -> Self {
        self.target_provider_proposals = proposals;
        self
    }

    pub fn target_provider_proposals(&self) -> &[backend_plan::TerminalTargetProviderProposal] {
        &self.target_provider_proposals
    }

    pub const fn native_target(&self) -> target::NativeTarget {
        self.program_entry.target()
    }

    pub const fn application(&self) -> &target::NativeApplicationMetadata {
        &self.application
    }

    pub const fn program_entry(&self) -> &crate::ValidatedNativeProgramEntrySettlement {
        &self.program_entry
    }

    pub const fn post_terminal_optimizations(
        &self,
    ) -> &optimization_core::PostTerminalOptimizationSelectionProjection {
        &self.post_terminal_optimizations
    }

    pub const fn providers(&self) -> &TerminalNativeProviders {
        &self.providers
    }

    pub const fn occurrences(&self) -> &TerminalNativeOccurrences {
        &self.occurrences
    }

    pub const fn boundary_applications(&self) -> &TerminalNativeBoundaryApplications {
        &self.boundary_applications
    }

    pub fn package_terminal_authority_permissions(
        &self,
    ) -> &[effects::ServiceTerminalAuthorityPermission] {
        &self.package_terminal_authority_permissions
    }

    pub const fn package_terminal_authority_permission_identities(
        &self,
    ) -> &terminal_psi::TerminalIdentityTable {
        &self.package_terminal_authority_permission_identities
    }

    pub const fn behavior_exclusions(&self) -> &effects::BehaviorExclusions {
        &self.behavior_exclusions
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct RetainedTerminalArtifact {
    artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    native_realization_proposal: Option<TerminalNativeRealizationProposal>,
}

impl RetainedTerminalArtifact {
    pub fn new_with_native_realization_proposal(
        artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
        native_realization_proposal: TerminalNativeRealizationProposal,
    ) -> Self {
        Self {
            artifact,
            native_realization_proposal: Some(native_realization_proposal),
        }
    }

    pub fn artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.artifact
    }

    pub fn shared_artifact(&self) -> &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact> {
        &self.artifact
    }

    pub const fn native_realization_proposal(&self) -> Option<&TerminalNativeRealizationProposal> {
        self.native_realization_proposal.as_ref()
    }

    pub fn into_parts(
        self,
    ) -> (
        std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
        Option<TerminalNativeRealizationProposal>,
    ) {
        (self.artifact, self.native_realization_proposal)
    }
}
