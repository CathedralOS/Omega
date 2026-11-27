/*
  Preparing native code must not drop selected providers, callbacks or the
  program entry. Publish this proposal only after their canonical joins pass;
  scope preparation then borrows the complete immutable result.
*/
use diagnostics::Diagnostic;
use provider_correspondence::callbacks::TerminalCallbackOccurrenceProposal;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeProviders {
    pub direct_boundary_leaves: Vec<direct_boundary_bindings::DirectBoundaryLeaf>,
    pub native_bindings: native_binding_admission::RetainedNativeBindings,
    pub selection: std::sync::Arc<selected_provider_plans::SelectedProviderPlanFacts>,
    pub checked_adapters: target_bound_program::SelectedProviderAdapters,
    pub external_bindings: Vec<calling_conventions::ExternalBindingRow>,
    pub intrinsic_executions:
        Vec<Vec<Option<native_intrinsics::CompilerIntrinsicExecutionIdentity>>>,
    pub x86_scalar_fma: Option<target::AdmittedX86ScalarFmaProvider>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeOccurrences {
    pub callbacks: std::sync::Arc<[TerminalCallbackOccurrenceProposal]>,
    pub installed_interrupts: Vec<provider_admission::InstalledInterruptNativeEntryProposal>,
    pub callback_arguments: Option<Vec<provider_admission::NativeCallbackArgumentProposal>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeBoundaryApplications {
    pub calling: Vec<calling_conventions::BoundaryCallingApplication>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TerminalNativeBodyProposal {
    providers: TerminalNativeProviders,
    occurrences: TerminalNativeOccurrences,
    boundary_applications: TerminalNativeBoundaryApplications,
    behavior_exclusions: native_behavior_exclusions::BehaviorExclusions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeRealizationProposal {
    imports: Vec<crate::ComponentImport>,
    application: target::NativeApplicationMetadata,
    scope_source: std::sync::Arc<target_bound_program::NativeScopeSource>,
    post_terminal_optimizations:
        optimization_selections::PostTerminalOptimizationSelectionProjection,
    providers: TerminalNativeProviders,
    occurrences: TerminalNativeOccurrences,
    boundary_applications: TerminalNativeBoundaryApplications,
    package_terminal_authority_permissions:
        Vec<provider_acceptance::ServiceTerminalAuthorityPermission>,
    package_terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable,
    behavior_exclusions: native_behavior_exclusions::BehaviorExclusions,
    target_provider: target_provider_admission::TerminalTargetProviderProposal,
}

impl TerminalNativeRealizationProposal {
    pub fn component_imports(&self) -> &[crate::ComponentImport] {
        &self.imports
    }

    pub fn target_profile(&self) -> target::TargetProfile {
        self.scope_source.root().target()
    }

    pub fn target_provider_proposals(
        &self,
    ) -> &[target_provider_admission::TerminalTargetProviderProposal] {
        std::slice::from_ref(&self.target_provider)
    }

    pub fn x86_scalar_fma(&self) -> Option<target::AdmittedX86ScalarFmaProvider> {
        self.providers.x86_scalar_fma
    }

    pub fn native_target(&self) -> target::NativeTarget {
        self.target_profile().native_target()
    }

    pub const fn application(&self) -> &target::NativeApplicationMetadata {
        &self.application
    }

    pub fn program_entry(&self) -> Option<&crate::ValidatedNativeProgramEntrySettlement> {
        self.scope_source.root().program()
    }

    /// Target callable named by the checked program entry and its optional physical adapter.
    pub fn root_callable(&self) -> terminal_psi::TargetMachineCallable {
        self.scope_source.root().callable()
    }

    pub fn scope_source(&self) -> &std::sync::Arc<target_bound_program::NativeScopeSource> {
        &self.scope_source
    }

    pub fn settle_scoped_boundary_applications(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        scope: &crate::NativeExecutionScope,
        selection: &selected_provider_plans::SelectedProviderPlanFacts,
        provenance: &[provider_correspondence::CompilerIntrinsicPlanProvenance<'_>],
    ) -> Result<boundary_applications::TerminalBoundaryApplicationCoverage, Vec<Diagnostic>> {
        let coverage = crate::settlement::boundary_application_settlement::settle(
            artifact,
            selection,
            provenance,
            &self.boundary_applications().calling,
            &self.providers().native_bindings,
            Some(self.target_profile()),
            scope,
        )?;
        crate::settlement::comparison_settlement::settle_comparison_occurrences(
            artifact.semantic_module(),
            selection,
            &coverage,
            scope.reach().machines(),
        )?;
        Ok(coverage)
    }

    pub const fn post_terminal_optimizations(
        &self,
    ) -> &optimization_selections::PostTerminalOptimizationSelectionProjection {
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
    ) -> &[provider_acceptance::ServiceTerminalAuthorityPermission] {
        &self.package_terminal_authority_permissions
    }

    pub const fn package_terminal_authority_permission_identities(
        &self,
    ) -> &terminal_psi::TerminalIdentityTable {
        &self.package_terminal_authority_permission_identities
    }

    pub const fn behavior_exclusions(&self) -> &native_behavior_exclusions::BehaviorExclusions {
        &self.behavior_exclusions
    }
}

mod proposal {
    use super::{
        TerminalNativeBodyProposal, TerminalNativeBoundaryApplications, TerminalNativeOccurrences,
        TerminalNativeProviders, TerminalNativeRealizationProposal,
    };
    use diagnostics::Diagnostic;

    pub enum NativeRootNomination<'input> {
        Program {
            receipt: &'input psi_publication_records::CheckedProgramEntryTerminalReceipt,
            entry: crate::NativeProgramEntrySettlement<'input>,
            bind_physical_adapter: bool,
        },
        Component(&'input psi_provider_selections::AdmittedComponentProvider),
    }

    pub struct TerminalNativeProposalRequest<'input> {
        pub evaluated_binding_identities: &'input terminal_psi::TerminalIdentityTable,
        pub native_bindings: &'input [native_binding_admission::NativeBindingInput<'input>],
        pub direct_boundary_leaves: Vec<&'input direct_boundary_bindings::DirectBoundaryLeaf>,
        pub target_profile: target::TargetProfile,
        pub boundaries: crate::BoundaryApplicationSettlement<'input>,
        pub target_provider: target_provider_admission::TerminalTargetProviderProposal,
        pub root: NativeRootNomination<'input>,
        pub imports: Vec<crate::ComponentImport>,
        pub application: target::NativeApplicationMetadata,
        pub optimizations: optimization_selections::PostTerminalOptimizationSelectionProjection,
        pub external_bindings: &'input [calling_conventions::ExternalBindingRow],
        pub behavior_exclusions:
            &'input [build_model::behavior_exclusions::AuthoredBehaviorExclusion],
        pub package_terminal_authority_permissions:
            Vec<provider_acceptance::ServiceTerminalAuthorityPermission>,
        pub package_terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable,
        pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
    }

    impl super::RetainedTerminalArtifact {
        /// Bind the entry, target and providers to the authorized program they describe.
        pub fn project(
            authorization: psi_authorization::AuthorizedTerminalArtifact,
            request: TerminalNativeProposalRequest<'_>,
        ) -> Result<Self, Vec<Diagnostic>> {
            let artifact = authorization.shared_artifact();
            let target_profile = request.target_profile;
            let body = project_terminal_native_body(artifact, target_profile, &request)?;
            let root = match request.root {
                NativeRootNomination::Program {
                    receipt,
                    entry,
                    bind_physical_adapter,
                } => {
                    if entry.source().target_slot().owner != target_profile {
                        return Err(vec![Diagnostic::error(
                            "Terminal native proposal target, profile, and ProgramEntry disagree",
                        )]);
                    }
                    let mut entry = crate::validate_native_program_entry_settlement(
                        artifact,
                        receipt,
                        entry.with_checked_entry(receipt),
                        target_profile.native_target(),
                    )
                    .map_err(|error| {
                        vec![Diagnostic::error(format!(
                            "Terminal native proposal ProgramEntry settlement failed: {error}"
                        ))]
                    })?;
                    if bind_physical_adapter {
                        entry
                            .bind_physical_adapter(
                                artifact,
                                &body.providers.selection,
                                &body.providers.checked_adapters,
                            )
                            .map_err(|error| vec![Diagnostic::error(error)])?;
                    }
                    target_bound_program::NativeRoot::Program(entry)
                }
                NativeRootNomination::Component(export) => {
                    target_bound_program::NativeRoot::Component {
                        target: target_profile,
                        export: export.clone(),
                    }
                }
            };
            let scope_source = target_bound_program::NativeScopeSource::admit(
                &authorization,
                root,
                body.providers.selection.clone(),
                body.providers.checked_adapters.clone(),
                body.occurrences.callbacks.clone(),
                body.providers.native_bindings.identity_digest(),
                &body.providers.intrinsic_executions,
            )?;
            let mut permissions = request.package_terminal_authority_permissions;
            let identities = request.package_terminal_authority_permission_identities;
            permissions.sort_by(|left, right| {
                left.service_schema()
                    .as_bytes()
                    .cmp(right.service_schema().as_bytes())
                    .then_with(|| {
                        identities
                            .text(left.requirement_identity())
                            .cmp(&identities.text(right.requirement_identity()))
                    })
            });
            Ok(Self {
                artifact: authorization,
                native_realization_proposal: TerminalNativeRealizationProposal {
                    imports: request.imports,
                    target_provider: request.target_provider,
                    application: request.application,
                    scope_source: std::sync::Arc::new(scope_source),
                    post_terminal_optimizations: request.optimizations,
                    providers: body.providers,
                    occurrences: body.occurrences,
                    boundary_applications: body.boundary_applications,
                    package_terminal_authority_permissions: permissions,
                    package_terminal_authority_permission_identities: identities,
                    behavior_exclusions: body.behavior_exclusions,
                },
            })
        }
    }

    fn project_terminal_native_body(
        artifact: &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
        target_profile: target::TargetProfile,
        request: &TerminalNativeProposalRequest<'_>,
    ) -> Result<TerminalNativeBodyProposal, Vec<Diagnostic>> {
        let terminal_module = artifact.semantic_module();
        let selected = target_provider_admission::close_checked_provider_specializations(
            request.boundaries.selected_provider_plans.clone(),
            terminal_module,
        )
        .map_err(|message| vec![Diagnostic::error(message)])?;
        provider_admission::validate_task_activations(
            &terminal_module.execution.task_activation_demands,
            &terminal_module.identities,
            &selected,
            target_profile,
        )?;
        let checked_adapters = crate::component_inputs::local_adapters(
            artifact.view(),
            target_profile,
            &selected,
            &request.imports,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        provider_admission::validate_external_provider_owners(terminal_module, &selected)
            .map_err(|message| vec![Diagnostic::error(message)])?;
        if selected.plans().len() != request.boundaries.selected_provider_provenance.len() {
            return Err(vec![Diagnostic::error(
                "native proposal lost selected intrinsic plan custody",
            )]);
        }
        let intrinsic_executions = selected
            .plans()
            .iter()
            .zip(&request.boundaries.selected_provider_provenance)
            .map(|(plan, retained)| {
                if plan != retained.plan
                    || plan.rows.len() != retained.row_executions.len()
                    || plan.rows.len() != plan.row_numeric_realizations.len()
                {
                    return Err(vec![Diagnostic::error(
                        "native proposal has mismatched intrinsic row custody",
                    )]);
                }
                // The plan owns external declaration custody; only reviewed
                // execution meanings are additional provenance.
                Ok(retained.row_executions.to_vec())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let callback_placements = crate::settle_native_callback_placements(
            terminal_module,
            &selected,
            &request.boundaries.calling_applications,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let mut callback_occurrences = project_callback_occurrences(
            &callback_placements,
            artifact,
            &selected,
            checked_adapters.rows(),
        )?;
        let callback_arguments = provider_admission::project_native_callback_arguments(
            terminal_module,
            &callback_placements,
            &callback_occurrences,
        )?;
        let registrar_occurrences = callback_occurrences
            .iter()
            .map(|occurrence| {
                let callback = terminal_module
                    .callback_application(occurrence.application_index())
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "callback occurrence lost its canonical application",
                        )]
                    })?;
                Ok(crate::CallbackRegistrarOccurrence {
                    placement_index: occurrence.placement_index(),
                    registrar_machine: callback.registrar_machine,
                    terminal_operation: occurrence.terminal_operation(),
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let external_binding_rows = crate::settle_callback_registrar_imports(
            terminal_module,
            &selected,
            &callback_placements,
            registrar_occurrences.into_iter(),
            &request.boundaries.calling_applications,
            request.external_bindings,
        )
        .map_err(|message| vec![Diagnostic::error(message)])?;
        let installed_placements =
            crate::settlement::callback_settlement::settle_installed_callback_placements(
                terminal_module,
                target_profile,
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
            let owner = terminal_module
                .callback_application(occurrence.application_index())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "installed entry lost its exact callback occurrence",
                    )]
                })?
                .registrar_machine;
            installed_interrupts.push(crate::InstalledInterruptNativeEntryProposal {
                machine: owner,
                operation: occurrence.terminal_operation(),
                placement_index,
                callback_function: occurrence.callback_thunk_identity(),
                placement,
            });
            callback_occurrences.push(occurrence);
        }
        provider_correspondence::callbacks::validate_callback_thunk_joins(
            artifact.view(),
            &callback_occurrences,
        )
        .map_err(|message| vec![Diagnostic::error(message)])?;
        Ok(TerminalNativeBodyProposal {
            providers: TerminalNativeProviders {
                native_bindings: native_binding_admission::RetainedNativeBindings::retain(
                    request.native_bindings,
                )
                .map_err(|reason| vec![Diagnostic::error(reason)])?,
                direct_boundary_leaves: bind_direct_boundary_leaves(
                    terminal_module,
                    &request.direct_boundary_leaves,
                )?,
                selection: std::sync::Arc::new(selected),
                checked_adapters,
                external_bindings: external_binding_rows,
                intrinsic_executions,
                x86_scalar_fma: request.x86_scalar_fma_provider,
            },
            occurrences: TerminalNativeOccurrences {
                callbacks: callback_occurrences.into(),
                installed_interrupts,
                callback_arguments,
            },
            boundary_applications: TerminalNativeBoundaryApplications {
                calling: request.boundaries.calling_applications.clone(),
            },
            behavior_exclusions: native_behavior_exclusions::authored_behavior_exclusion_set_in(
                request.behavior_exclusions,
                terminal_module,
            )?,
        })
    }

    fn bind_direct_boundary_leaves(
        module: &terminal_psi::TerminalModule,
        candidates: &[&direct_boundary_bindings::DirectBoundaryLeaf],
    ) -> Result<Vec<direct_boundary_bindings::DirectBoundaryLeaf>, Vec<Diagnostic>> {
        let mut bound = Vec::new();
        for candidate in candidates {
            let mut leaf = (*candidate).clone();
            if !leaf
                .bind_to_terminal(module)
                .map_err(|reason| vec![Diagnostic::error(reason)])?
            {
                continue;
            }
            bound.push(leaf);
        }
        Ok(bound)
    }

    fn project_callback_occurrences(
        callback_placements: &[provider_correspondence::callbacks::NativeCallbackPlacement],
        artifact: &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
        selected: &selected_provider_plans::SelectedProviderPlanFacts,
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
                .interfaces
                .callback_applications
                .iter()
                .map(|application| (application.registrar_machine, application.registrar)),
            module.machines.iter().flat_map(|machine| {
                machine
                    .operations()
                    .map(move |operation| (machine.id, operation))
            }),
        );
        for application in &module.interfaces.callback_applications {
            let first = application_indices.partition_point(|application_index| {
                *application_index < application.application_index
            });
            let last = application_indices.partition_point(|application_index| {
                *application_index <= application.application_index
            });
            let operation = operations
                .find((application.registrar_machine, application.registrar))
                .map_err(|_| {
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
            if let provider_plans::ProviderBinding::CheckedAdapter {
                machine_identity, ..
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
        placement: &provider_correspondence::callbacks::NativeCallbackPlacement,
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

pub use proposal::{NativeRootNomination, TerminalNativeProposalRequest};

#[derive(Debug, PartialEq, Eq)]
pub struct RetainedTerminalArtifact {
    artifact: psi_authorization::AuthorizedTerminalArtifact,
    native_realization_proposal: TerminalNativeRealizationProposal,
}

impl RetainedTerminalArtifact {
    pub fn artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        self.artifact.artifact()
    }

    pub fn shared_artifact(&self) -> &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact> {
        self.artifact.shared_artifact()
    }

    pub const fn authorization(&self) -> &psi_authorization::AuthorizedTerminalArtifact {
        &self.artifact
    }

    pub const fn native_realization_proposal(&self) -> &TerminalNativeRealizationProposal {
        &self.native_realization_proposal
    }
}
