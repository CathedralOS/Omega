/* Native admission orchestrates the shared verified input and provider evidence. */
pub(crate) mod native_admission {
    use crate::providers::provider_admission::{
        AdmittedNativeProviders, ProviderAdmissionRequest, admit_native_providers,
        realization_error,
    };
    use crate::{ArtifactLoweringError, VerifiedNativeArtifactInput};
    use boundary_applications::TerminalBoundaryApplicationCoverage;
    use diagnostics::Diagnostic;
    use semantic_vocabulary::BoundaryMachineId;
    use std::collections::BTreeSet;
    use terminal_codec::CanonicalTerminalArtifact;

    #[derive(Clone, Copy)]
    pub struct NativeAdmissionRequest<'request, 'execution> {
        pub entry: terminal_psi::TargetMachineCallable,
        pub private_entry_ram: &'request [terminal_installation::CheckedPrivateEntryRamRecipe],
        pub providers: ProviderAdmissionRequest<'request, 'execution>,
        pub boundary_application_coverage: Option<&'request TerminalBoundaryApplicationCoverage>,
        pub behavior_exclusions: &'request effects::BehaviorExclusions,
    }

    pub struct AdmittedNativeRealization<'execution> {
        pub input: VerifiedNativeArtifactInput,
        pub providers: AdmittedNativeProviders<'execution>,
        pub boundary_application_coverage: Option<TerminalBoundaryApplicationCoverage>,
    }

    pub fn admit_native_realization<'execution>(
        artifact: &CanonicalTerminalArtifact,
        request: &NativeAdmissionRequest<'_, 'execution>,
    ) -> Result<AdmittedNativeRealization<'execution>, Vec<Diagnostic>> {
        let input = VerifiedNativeArtifactInput::prepare_with_private_entry_ram(
            artifact,
            request.providers.profile,
            request.entry,
            request.private_entry_ram,
            &crate::NativeExecutionSelection {
                scope: None,
                scalar_applications: None,
                providers: request.providers.checked_adapters,
                plans: request.providers.selected_provider_plans,
                callbacks: &[],
                entry_call: None,
            },
        )
        .map_err(artifact_lowering_error)?;
        let input = input
            .bind_selected_provider_coordinates(artifact, request.providers.selected_provider_plans)
            .map_err(|error| realization_error("selected provider coordinates", error))?;
        admit_prepared_native_realization(input, artifact, request)
    }

    pub fn admit_prepared_native_realization<'execution>(
        input: VerifiedNativeArtifactInput,
        artifact: &CanonicalTerminalArtifact,
        request: &NativeAdmissionRequest<'_, 'execution>,
    ) -> Result<AdmittedNativeRealization<'execution>, Vec<Diagnostic>> {
        let executed_boundaries: BTreeSet<BoundaryMachineId> = input
            .plan()
            .boundary_machines
            .iter()
            .map(|boundary| boundary.id)
            .collect();
        let module_boundaries: BTreeSet<BoundaryMachineId> = input
            .context()
            .module()
            .boundary_machines
            .iter()
            .map(|boundary| boundary.id)
            .collect();
        let settlements = request
            .providers
            .settlements
            .iter()
            .copied()
            .filter(|settlement| {
                let mut matching = input
                    .context()
                    .module()
                    .boundary_machines
                    .iter()
                    .filter(|boundary| {
                        request
                            .providers
                            .selected_provider_plans
                            .selected_row_matches_requirement(
                                input
                                    .context()
                                    .module()
                                    .identities
                                    .text(boundary.identity)
                                    .unwrap_or_default(),
                                settlement.provider_execution.requirement_identity(),
                                settlement
                                    .provider_execution
                                    .provider_plan_report_identity(),
                            )
                    })
                    .peekable();
                matching.peek().is_none()
                    || matching.any(|boundary| executed_boundaries.contains(&boundary.id))
            })
            .collect::<Vec<_>>();
        let compiler_builtins = request
            .providers
            .compiler_builtins
            .iter()
            .copied()
            .filter(|proposal| {
                executed_boundaries.contains(&proposal.boundary)
                    || !module_boundaries.contains(&proposal.boundary)
            })
            .collect::<Vec<_>>();
        let (native_callbacks, installed_interrupts) = if request
            .providers
            .native_callbacks
            .is_empty()
            && request.providers.installed_interrupts.is_empty()
        {
            (Vec::new(), Vec::new())
        } else {
            let plan_boundary_call_ops: BTreeSet<semantic_vocabulary::OperationId> = input
                .plan()
                .operations()
                .filter_map(|operation| match operation {
                    abstract_operations::AbstractOperation::BoundaryCall {
                        psi_operation, ..
                    } => Some(*psi_operation),
                    _ => None,
                })
                .collect();
            let module_boundary_call_ops: BTreeSet<semantic_vocabulary::OperationId> = input
                .context()
                .module()
                .machines
                .iter()
                .flat_map(|machine| machine.operations())
                .filter_map(|operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::BoundaryCall { .. }
                    )
                    .then_some(operation.id)
                })
                .collect();
            let in_scope = |operation| {
                plan_boundary_call_ops.contains(&operation)
                    || !module_boundary_call_ops.contains(&operation)
            };
            let native_callbacks = request
                .providers
                .native_callbacks
                .iter()
                .filter(|callback| in_scope(callback.terminal_operation))
                .cloned()
                .collect::<Vec<_>>();
            let mut callback_occurrences = vec![None; request.providers.callback_occurrences.len()];
            for occurrence in request.providers.callback_occurrences {
                let Some(slot) = callback_occurrences.get_mut(occurrence.placement_index()) else {
                    return Err(realization_error(
                        "installed interrupt callback scope",
                        "callback placement falls outside the original complete roster",
                    ));
                };
                if slot.replace(occurrence).is_some() {
                    return Err(realization_error(
                        "installed interrupt callback scope",
                        "original callback placement identity is duplicated",
                    ));
                }
            }
            let installed_interrupts = request
                .providers
                .installed_interrupts
                .iter()
                .filter(|entry| {
                    in_scope(entry.operation)
                        || callback_occurrences
                            .get(entry.placement_index)
                            .copied()
                            .flatten()
                            .is_some_and(|occurrence| {
                                occurrence.callback_thunk_artifact().entry() == input.plan().entry
                                    && occurrence.terminal_operation() == entry.operation
                                    && occurrence.application_index()
                                        == entry.placement.application_index
                                    && occurrence.callback_thunk_identity()
                                        == entry.callback_function
                            })
                })
                .cloned()
                .collect::<Vec<_>>();
            (native_callbacks, installed_interrupts)
        };
        let scoped_providers = ProviderAdmissionRequest {
            profile: request.providers.profile,
            bootstrap_entry: request.providers.bootstrap_entry,
            program_provider_context: request.providers.program_provider_context,
            installation_assumptions: request.providers.installation_assumptions,
            selected_provider_plans: request.providers.selected_provider_plans,
            checked_adapters: request.providers.checked_adapters,
            external_binding_rows: request.providers.external_binding_rows,
            settlements: &settlements,
            compiler_builtins: &compiler_builtins,
            native_callbacks: &native_callbacks,
            installed_interrupts: &installed_interrupts,
            installed_calling: request.providers.installed_calling,
            callback_occurrences: request.providers.callback_occurrences,
            terminal_authority_policy: request.providers.terminal_authority_policy,
            terminal_authority_permission_policy: request
                .providers
                .terminal_authority_permission_policy,
            x86_scalar_fma_provider: request.providers.x86_scalar_fma_provider,
        };
        let mut providers =
            admit_native_providers(&input.admission_source(), artifact, &scoped_providers)?;
        request
            .behavior_exclusions
            .validate_physical_authority_closure(providers.terminal_authority_closure_review())?;
        let mut input = input;
        input.retain_admitted_assumed_bindings(
            providers
                .installation
                .as_ref()
                .map(|installation| {
                    installation
                        .terminal_selection()
                        .assumed_bindings()
                        .iter()
                        .map(
                            |row| installation_evidence::InstalledAssumedBindingEvidence {
                                required: row.required,
                                supplied_requirement: row.supplied_requirement,
                                implementation: row.implementation,
                                selected_requirement: row.selected_requirement,
                                supply_path: row.supply_path.clone(),
                            },
                        )
                        .collect()
                })
                .unwrap_or_default(),
        );
        input.retain_admitted_native_callbacks(providers.take_native_callbacks());
        let installed_interrupts = providers.take_installed_interrupts();
        if installed_interrupts.psi() != input.plan().psi {
            return Err(vec![Diagnostic::error(
                "installed interrupt entries belong to another Terminal program",
            )]);
        }
        input.retain_admitted_installed_interrupts(installed_interrupts);
        Ok(AdmittedNativeRealization {
            input,
            providers,
            boundary_application_coverage: request.boundary_application_coverage.cloned(),
        })
    }

    pub fn artifact_lowering_error(error: ArtifactLoweringError) -> Vec<Diagnostic> {
        match error {
            ArtifactLoweringError::SemanticDecode(error) => {
                realization_error("semantic section", error)
            }
            error => realization_error("native artifact lowering", error),
        }
    }
}
