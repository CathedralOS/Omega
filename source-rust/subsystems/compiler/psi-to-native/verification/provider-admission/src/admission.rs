/*
Provider admission for native realization.

`source_evaluated_bindings` prepares selected import and syscall executions for
direct compilation and retained Terminal re-entry; only imported user-space
callees carry a foreign same-stack reserve. `admit_native_providers` first
requires a selected provider for every boundary requirement the entry reaches,
then independently checks those inputs, settles external and builtin executions,
and admits the exact checked-provider installation and authority closure
retained by the plan.
*/

/*
We keep the public closure receipt (`TerminalAuthorityClosureReviewReceipt`)
behind `AdmittedNativeCallableAuthority`, which is minted only by actual
provider admission after complete closure review beside the original canonical
artifact and selected entry. Likewise, `project_native_callback_arguments` and
retained builtin proposals carry plan-index custody as proposals, never
replacing independent native admission.
*/

use crate::terminal_authority_policy::darwin_poll;
use crate::terminal_authority_review::reachability::AuthorityGraph;
use crate::{
    AdmissionSource, AdmittedBoundaryExecution, AdmittedBoundarySettlement,
    AdmittedProgramProviderContext, AdmittedProviderInstallation, ProviderAdmissionRequest,
    access_lifecycles, callback_entries, compiler_builtins, settlements, static_interrupts,
    terminal_authority_review,
};
use ::diagnostics::Diagnostic;
use application_identity::type_identity::callable_display_path as requirement_path;

pub fn realization_error(context: &str, error: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "native artifact {context} failed: {error}"
    ))]
}

/// Report identities supplied when admitting a source-evaluated or component provider call.
pub trait ProviderExecutionEvidence: std::fmt::Debug {
    fn requirement_identity(&self) -> &str;
    fn provider_plan_report_identity(&self) -> u64;
    fn provider_execution_report_identity(&self) -> u64;
    fn provider_execution_report_fingerprint(&self) -> u64;
    fn normalized_root_report_identity(&self) -> u64;
    fn boundary_contract_report_fingerprint(&self) -> u64;
}

#[derive(Debug, Clone)]
pub struct AdmittedNativeCallableAuthority {
    subject: terminal_psi::TerminalPsiIdentity,
    entry: semantic_vocabulary::MachineId,
    review: terminal_authority_receipts::TerminalAuthorityClosureReviewReceipt,
}

impl AdmittedNativeCallableAuthority {
    fn from_admission(
        subject: terminal_psi::TerminalPsiIdentity,
        entry: semantic_vocabulary::MachineId,
        review: terminal_authority_receipts::TerminalAuthorityClosureReviewReceipt,
    ) -> Self {
        Self {
            subject,
            entry,
            review,
        }
    }

    pub const fn subject(&self) -> terminal_psi::TerminalPsiIdentity {
        self.subject
    }

    pub const fn entry(&self) -> semantic_vocabulary::MachineId {
        self.entry
    }

    pub const fn review(
        &self,
    ) -> &terminal_authority_receipts::TerminalAuthorityClosureReviewReceipt {
        &self.review
    }
}

pub struct AdmittedNativeProviders<'execution> {
    native_bindings: native_binding_admission::RetainedNativeBindings,
    native_calling: Vec<calling_conventions::BoundaryCallingApplication>,
    pub settlements: Vec<AdmittedBoundarySettlement<'execution>>,
    pub(crate) native_callbacks: crate::AdmittedNativeCallbackCohort, // Scoped to this executable occurrence scope, never the full retained catalog.
    pub(crate) installed_interrupts: crate::AdmittedInstalledInterruptCohort,
    pub executions: Vec<&'execution dyn ProviderExecutionEvidence>,
    pub terminal_authority_policy_identity:
        terminal_authority_receipts::TerminalAuthorityPolicyIdentity,
    pub terminal_authority_permission_policy_identity:
        Option<terminal_authority_receipts::TerminalAuthorityPermissionPolicyIdentity>, // `Some` only when a receiving permission policy was explicitly supplied.
    callable_authority: AdmittedNativeCallableAuthority,
    program_provider_context: std::sync::Arc<AdmittedProgramProviderContext>,
    pub installation: Option<AdmittedProviderInstallation>,
}

impl AdmittedNativeProviders<'_> {
    pub fn native_bindings(&self) -> &native_binding_admission::RetainedNativeBindings {
        &self.native_bindings
    }
    pub fn native_calling(&self) -> &[calling_conventions::BoundaryCallingApplication] {
        &self.native_calling
    }
    pub const fn program_provider_context(
        &self,
    ) -> &std::sync::Arc<AdmittedProgramProviderContext> {
        &self.program_provider_context
    }

    pub fn take_native_callbacks(&mut self) -> crate::AdmittedNativeCallbackCohort {
        let psi = self.native_callbacks.psi;
        std::mem::replace(
            &mut self.native_callbacks,
            crate::AdmittedNativeCallbackCohort::empty(psi),
        )
    }

    pub fn take_installed_interrupts(&mut self) -> crate::AdmittedInstalledInterruptCohort {
        let psi = self.installed_interrupts.psi();
        std::mem::replace(
            &mut self.installed_interrupts,
            crate::AdmittedInstalledInterruptCohort::empty(psi),
        )
    }

    pub const fn callable_authority(&self) -> &AdmittedNativeCallableAuthority {
        &self.callable_authority
    }

    pub const fn terminal_authority_closure_review(
        &self,
    ) -> &terminal_authority_receipts::TerminalAuthorityClosureReviewReceipt {
        self.callable_authority.review()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AdmittedTerminalMechanism {
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub mechanism: terminal_authority_receipts::TerminalMechanismIdentity,
}

pub fn admit_native_providers<'request>(
    input: &AdmissionSource,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    request: &ProviderAdmissionRequest<'_, 'request>,
) -> Result<AdmittedNativeProviders<'request>, Vec<Diagnostic>> {
    if let Some(context) = request.program_provider_context {
        context
            .require(
                artifact.view().identity(),
                request.profile,
                request.selected_provider_plans,
            )
            .map_err(|reason| realization_error("program provider context", reason))?;
    }
    let terminal_artifact_identity = *artifact.manifest().identity().as_bytes();
    let entry_custody = callback_entries::PreparedEntries::new(input, request)
        .map_err(|reason| realization_error("callback entry admission", reason))?;
    let installation = admit_checked_provider_installation(input, artifact, request)?;
    let helper_associations = static_interrupts::require_helper_associations(
        input,
        request,
        artifact,
        &entry_custody,
        installation.as_ref().map(|row| row.terminal_selection()),
    )?;
    let installed_interrupts = static_interrupts::admit(
        input,
        artifact,
        request,
        installation.as_ref().map(|row| row.terminal_selection()),
        helper_associations,
        &entry_custody,
    )?;
    let (interrupt_settlements, interrupt_mechanisms) = static_interrupts::settle(
        input,
        &installed_interrupts,
        request.terminal_authority_policy,
    )?;
    let compiler_builtins = compiler_builtins::reached_builtins(input, artifact, request)?;
    let (native_settlements, native_mechanisms) =
        crate::native_binding_settlements::settle(input, request, &compiler_builtins)?;
    require_reached_providers(
        input,
        request.profile,
        request.selected_provider_plans,
        &installed_interrupts,
        &interrupt_mechanisms,
        &native_settlements,
    )?;
    let (lifecycle_settlements, lifecycle_mechanisms) =
        access_lifecycles::settle_native_lifecycles(
            input,
            installation.as_ref(),
            request.terminal_authority_policy,
        )?;
    let (settlements, executions, mut mechanisms, mut cohort_rows) =
        settlements::settle_provider_executions(input, request, artifact.view().identity())?;
    let mut settlements = settlements;
    settlements.extend(interrupt_settlements);
    mechanisms.extend(interrupt_mechanisms);
    let (builtin_settlements, builtin_mechanisms) =
        compiler_builtins::settle_compiler_builtins(input, request, &compiler_builtins)?;
    settlements.extend(builtin_settlements);
    mechanisms.extend(builtin_mechanisms);
    settlements.extend(native_settlements);
    mechanisms.extend(native_mechanisms);
    for settlement in &settlements {
        let checked_component = matches!(
            &settlement.realization,
            target_operations::BoundarySettlementRealization::NormalizedForeignCall(binding)
                if matches!(&binding.mechanism,
                    target_operations::NormalizedForeignCallMechanism::Component { .. })
        );
        let module = input.module();
        let opaque_foreign = matches!(
            &settlement.realization,
            target_operations::BoundarySettlementRealization::NormalizedForeignCall(_)
        ) && !checked_component;
        if let Some(boundary) = module.boundary_machine(settlement.boundary)
            && opaque_foreign
            && boundary.requires_checked_non_observation(|id| module.structural_type(id))
        {
            return Err(vec![Diagnostic::error(format!(
                "opaque native provider for `{}` requires an admitted write-only non-observation judgment bound to its exact selected implementation and receipt",
                module
                    .identities
                    .text(boundary.identity)
                    .unwrap_or_default(),
            ))]);
        }
        if let Some(boundary) = module.boundary_machine(settlement.boundary)
            && boundary.requires_checked_result(
                &module.types.structural_domains,
                &module.types.scalar_qualifications.domains,
            )
            && !checked_component
        {
            return Err(vec![Diagnostic::error(format!(
                "native provider for `{}` cannot establish result provenance owned by another issuer; a checked provider must return the qualified value",
                module
                    .identities
                    .text(boundary.identity)
                    .unwrap_or_default(),
            ))]);
        }
    }
    settlements.extend(lifecycle_settlements);
    mechanisms.extend(lifecycle_mechanisms);
    let (run_settlements, run_mechanisms) =
        access_lifecycles::settle_empty_releases(input, request.terminal_authority_policy)?;
    settlements.extend(run_settlements);
    mechanisms.extend(run_mechanisms);
    if request.profile == target::TargetProfile::MacosArm64 {
        darwin_poll::classify_checked_waits(
            artifact,
            input.plan(),
            &settlements,
            &mut mechanisms,
            request.terminal_authority_policy,
            &mut cohort_rows,
        );
    }
    let effective_policy = request
        .terminal_authority_policy
        .with_reviewed_operations(cohort_rows, mechanisms.iter().map(|leaf| leaf.mechanism))
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "could not construct the receiving policy from the reviewed native operations ({error:?})"
            ))]
        })?;
    let effective_policy = effective_policy
        .as_ref()
        .unwrap_or(request.terminal_authority_policy);
    let terminal_authority_closure_review =
        terminal_authority_review::review_terminal_authority_closure(
            terminal_artifact_identity,
            request.profile,
            input,
            request.selected_provider_plans,
            effective_policy,
            request.terminal_authority_permission_policy.as_ref(),
            &mechanisms,
            &settlements,
            installation
                .as_ref()
                .map(AdmittedProviderInstallation::installed_candidates)
                .unwrap_or_default(),
            request.settlements,
            &installed_interrupts,
        )
        .map_err(|error| realization_error("terminal-authority closure review", error))?;
    let program_provider_context = request
        .program_provider_context
        .cloned()
        .unwrap_or_else(|| {
            std::sync::Arc::new(AdmittedProgramProviderContext::new(
                artifact.view().identity(),
                request.profile,
                request.selected_provider_plans,
                installation.as_ref().map(|row| row.terminal_selection()),
                input
                    .selected_program_entry_call()
                    .map(|entry| (entry.boundary(), entry.entry())),
            ))
        });
    let mut native_calling = Vec::new();
    for settlement in &settlements {
        if !matches!(
            settlement.execution,
            AdmittedBoundaryExecution::NativeBinding { .. }
        ) {
            continue;
        }
        let abi = artifact
            .semantic_module()
            .calling_abi_identity(settlement.boundary)
            .map_err(|reason| realization_error("native binding calling custody", reason))?;
        let application = request
            .installed_calling
            .iter()
            .find(|application| application.abi == abi)
            .ok_or_else(|| {
                realization_error(
                    "native binding calling custody",
                    "admitted application disappeared",
                )
            })?;
        native_calling.push(application.clone());
    }
    Ok(AdmittedNativeProviders {
        native_bindings: request.native_bindings.clone(),
        native_calling,
        installed_interrupts,
        settlements,
        native_callbacks: crate::AdmittedNativeCallbackCohort {
            psi: input.plan().psi,
            callbacks: callback_entries::admit_entries(input, request, &entry_custody)
                .map_err(|reason| realization_error("callback entry admission", reason))?
                .into(),
        },
        executions,
        terminal_authority_policy_identity: terminal_authority_closure_review.physical_policy(),
        terminal_authority_permission_policy_identity: request
            .terminal_authority_permission_policy
            .as_ref()
            .map(|policy| policy.identity()),
        callable_authority: AdmittedNativeCallableAuthority::from_admission(
            artifact.view().identity(),
            input.plan().entry,
            terminal_authority_closure_review,
        ),
        installation,
        program_provider_context,
    })
}

fn admit_checked_provider_installation(
    input: &AdmissionSource,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    request: &ProviderAdmissionRequest<'_, '_>,
) -> Result<Option<AdmittedProviderInstallation>, Vec<Diagnostic>> {
    let plan = input.plan();
    let view = artifact.view();
    let dependencies = target_provider_admission::matching_folded_constant_dependencies(
        view,
        request.profile.target_name(),
        request.selected_provider_plans.plans().iter(),
    )
    .map_err(|reason| realization_error("folded constant realization closure", reason))?;
    if plan.provider_candidates.is_empty() || request.checked_adapters.is_empty() {
        if view.module().scalars.folded_constants.target.is_none() {
            return Ok(None);
        }
        let selection = psi_provider_selections::admit_provider_installation(view, &[])
            .map_err(|error| {
                realization_error("historical-only catalog admission", format!("{error:?}"))
            })?
            .admit_folded_constant_closure(view, request.profile.target_name(), &dependencies)
            .map_err(|reason| realization_error("folded constant realization closure", reason))?;
        return Ok(Some(AdmittedProviderInstallation {
            psi: selection.terminal_psi(),
            terminal_selection: selection,
            installed_candidates: Vec::new(),
            installed_calls: Vec::new(),
        }));
    }
    let mut installation =
        crate::provider_installation::admit_provider_installation_with_projection(
            plan,
            view,
            request.checked_adapters,
        )
        .map_err(|error| {
            realization_error("checked-provider installation", format!("{error:?}"))
        })?;
    if view.module().scalars.folded_constants.target.is_some() {
        installation.terminal_selection = installation
            .terminal_selection
            .admit_folded_constant_closure(view, request.profile.target_name(), &dependencies)
            .map_err(|reason| realization_error("folded constant realization closure", reason))?;
    }
    Ok(Some(installation))
}

fn require_reached_providers(
    input: &crate::AdmissionSource<'_>,
    target_profile: target::TargetProfile,
    selected: &selected_provider_plans::SelectedProviderPlanFacts,
    installed_interrupts: &crate::AdmittedInstalledInterruptCohort,
    interrupt_mechanisms: &[AdmittedTerminalMechanism],
    native_settlements: &[AdmittedBoundarySettlement<'_>],
) -> Result<(), Vec<Diagnostic>> {
    let plan = input.plan();
    let reached = AuthorityGraph::new(input, target_profile)
        .and_then(|graph| graph.with_installed_interrupts(installed_interrupts))
        .and_then(|graph| graph.reachable_authority_edges(plan.entry))
        .map_err(|error| realization_error("provider supply", error))?;
    let diagnostics = reached
        .boundaries
        .iter()
        .filter_map(|boundary| {
            let declaration = plan
                .boundary_machines
                .iter()
                .find(|declaration| declaration.id == *boundary)?;
            if declaration.declaration.as_ref().is_some_and(|row| {
                row.execution == terminal_psi::CheckedBoundaryExecution::EmptyRunRelease
            }) {
                return None;
            }
            if let Some(kind) = declaration
                .declaration
                .as_ref()
                .and_then(|callable| callable.execution.static_interrupt_operation())
            {
                let expected = terminal_authority_receipts::TerminalMechanismIdentity::CompilerIntrinsic(
                    native_intrinsics::CompilerIntrinsicExecutionIdentity::StaticInterrupt(kind),
                );
                let admitted = interrupt_mechanisms
                    .binary_search_by_key(boundary, |row| row.boundary)
                    .ok()
                    .map(|position| &interrupt_mechanisms[position]);
                return (admitted.is_none_or(|row| row.mechanism != expected)).then(|| {
                    Diagnostic::error(
                        "static interrupt operation requires its exact admitted installation or arrival execution",
                    )
                });
            }
            if declaration
                .declaration
                .as_ref()
                .and_then(|declaration| declaration.execution.placement_operation())
                .is_some()
            {
                return None;
            }
            if native_settlements.iter().any(|settlement| settlement.boundary == *boundary) { return None; }
            let requirement = plan.identities.text(declaration.identity)?;
            selected.selected_requirement_row(requirement).is_none()
            .then(|| {
                // A canonical float operation is never Build-selected; when the
                // target's realization table declares it unsupported, say so.
                if let Some(target::CanonicalFloatRealization::Unsupported { constraint }) =
                    target_profile.canonical_float_realization(requirement)
                {
                    return Diagnostic::error(format!(
                        "canonical float operation `{}` has no realization on target `{}`: {constraint}",
                        requirement_path(requirement),
                        target_profile.target_name(),
                    ));
                }
                Diagnostic::error(format!(
                    "no provider is selected for boundary requirement `{}`, which the program entry reaches on target `{}`",
                    requirement_path(requirement),
                    target_profile.target_name(),
                ))
            })
        })
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
