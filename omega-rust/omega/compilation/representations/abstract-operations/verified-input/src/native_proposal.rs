/*
  Preparing native code must not drop selected providers, callbacks or the
  program entry. Publish this proposal only after their canonical joins pass;
  scope preparation then borrows the complete immutable result.
*/
use backend_plan::TerminalCallbackOccurrenceProposal;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeProviders {
    pub direct_boundary_leaves: Vec<backend_plan::DirectBoundaryLeaf>,
    pub selection: effects::SelectedProviderPlanFacts,
    pub declarations: Vec<backend_plan::TerminalTargetProviderPlan>,
    pub checked_adapters: Vec<provider_admission::SelectedProviderAdapter>,
    pub external_bindings: Vec<calling_conventions::ExternalBindingRow>,
    pub intrinsic_executions: Vec<Vec<Option<effects::CompilerIntrinsicExecutionIdentity>>>,
    pub numeric_realizations: Vec<Vec<Option<String>>>,
    pub x86_scalar_fma: Option<target::AdmittedX86ScalarFmaProvider>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeOccurrences {
    pub callbacks: Vec<TerminalCallbackOccurrenceProposal>,
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
        self.program_entry
            .bind_physical_adapter(artifact, &self.providers.checked_adapters)
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
            provider_admission::provider_admission::retain_selected_provider_adapters(
                &selected,
                request.provider_declarations,
            )
            .map_err(|message| vec![Diagnostic::error(message)])?;
        provider_admission::provider_admission::validate_external_provider_owners(
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
                // Supplied meanings must survive until selected execution
                // checks them against the canonical external declaration.
                retained.row_executions.to_vec(),
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
            provider_admission::provider_admission::project_native_callback_arguments(
                &callback_placements,
                &callback_occurrences,
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
            },
            boundary_applications: TerminalNativeBoundaryApplications {
                calling: request.boundaries.calling_applications.clone(),
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
            leaf.plan.rows[0].declared_requirement_identity =
                leaf.plan.rows[0].requirement_identity;
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

#[derive(Debug, PartialEq, Eq)]
pub struct RetainedTerminalArtifact {
    artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    native_realization_proposal: Option<TerminalNativeRealizationProposal>,
    /// The evidence seals produced beside this exact in-memory artifact by
    /// the checker on the lowered form. Terminal Psi's bytes carry no
    /// evidence, so a product rebuilt from bytes has none.
    evidence: Option<std::sync::Arc<terminal_codec::EvidenceSeals>>,
}

impl RetainedTerminalArtifact {
    pub fn new_with_native_realization_proposal(
        artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
        native_realization_proposal: TerminalNativeRealizationProposal,
    ) -> Self {
        Self {
            artifact,
            native_realization_proposal: Some(native_realization_proposal),
            evidence: None,
        }
    }

    /// Keeps the seals that were produced beside this artifact, so native
    /// production hands them to Omega 00 with the module they name.
    pub fn with_evidence(
        mut self,
        evidence: Option<std::sync::Arc<terminal_codec::EvidenceSeals>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        if let Some(evidence) = &evidence {
            validate_operation_evidence(&self.artifact, evidence)?;
        }
        self.evidence = evidence;
        Ok(self)
    }

    pub const fn evidence(&self) -> Option<&std::sync::Arc<terminal_codec::EvidenceSeals>> {
        self.evidence.as_ref()
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

/// Validate the complete in-memory bundle before selecting executable machines.
/// A foreign or damaged dormant seal cannot disappear through scope filtering.
pub(crate) fn validate_operation_evidence(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    evidence: &terminal_codec::EvidenceSeals,
) -> Result<(), Vec<Diagnostic>> {
    if evidence.module != artifact.view().identity() {
        return Err(vec![Diagnostic::error(
            "operation evidence belongs to another canonical artifact",
        )]);
    }
    terminal_codec::EvidenceSealValidator::new(artifact.semantic_module(), &evidence.seals)
        .validate_all()
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "canonical operation evidence: {error}"
            ))]
        })
}
