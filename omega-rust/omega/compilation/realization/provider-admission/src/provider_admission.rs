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

mod access_lifecycles;
mod adapters;
mod callback_entries;
mod compiler_builtins;
mod model;
mod settlements;
mod source_evaluated_bindings;
mod static_interrupts;
mod terminal_authority_permission_policy;
mod terminal_authority_policy;
mod terminal_authority_review;

pub use crate::AdmittedInstalledInterruptCohort;
pub use adapters::{
    AdmittedProgramProviderContext, retain_selected_provider_adapters,
    selected_synchronous_call_graph,
};
pub use callback_entries::project_native_callback_arguments;
pub use compiler_builtins::{
    AdmittedTerminalIntrinsics, CompilerIntrinsicPlanProvenance, SettledCompilerBuiltin,
    TerminalCompilerBuiltinProposal, TerminalCompilerBuiltinSettlement, admit_terminal_intrinsics,
    derive_selected_intrinsic_settlement_proposals, prepare_retained_compiler_builtins,
    validate_external_provider_owners,
};
pub use model::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedCallbackEntry,
    AdmittedFatalInterruptEntry, AdmittedInstalledInterruptEntry, AdmittedInstalledInterruptHelper,
    AdmittedInstalledInterruptScope, AdmittedInterruptContextAnchor,
    AdmittedNativeCallbackArgument, InstalledInterruptNativeEntryProposal,
    NativeBoundaryRealization, NativeCallbackArgumentProposal, NativeCompilerBuiltinSettlement,
    NativeProviderSettlement, ProviderAdmissionRequest,
};
pub use source_evaluated_bindings::{
    PreparedSourceEvaluatedBindings, SourceEvaluatedImportSettlement,
    prepare_source_evaluated_bindings,
};
pub use terminal_authority_permission_policy::{
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPermissionPolicyRow,
    terminal_authority_permission_policy_with_rows,
};
pub use terminal_authority_policy::{
    TerminalAuthorityPolicy, current_terminal_authority_policy, filesystem_host_permission_rows,
    terminal_authority_policy_with_rows, time_host_permission_rows,
};
pub(crate) use terminal_authority_policy::{
    normalized_foreign_terminal_mechanism,
    normalized_foreign_terminal_mechanism_with_callback_materializations,
};

pub mod terminal_authority_permissions {
    pub use super::terminal_authority_permission_policy::{
        validate_package_terminal_authority_permission_custody,
        validate_retained_package_terminal_authority_permissions,
    };
}

use crate::AdmissionSource as NativeRealizationInput;
use crate::AdmittedProviderInstallation;
use ::diagnostics::Diagnostic;
use installation_evidence::ProviderExecutionEvidence;
use language_semantics::type_identity::callable_display_path as requirement_path;
use terminal_authority_review::reachability::AuthorityGraph;

pub fn realization_error(context: &str, error: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "native artifact {context} failed: {error}"
    ))]
}

#[derive(Debug, Clone)]
pub struct AdmittedNativeCallableAuthority {
    subject: terminal_psi::TerminalPsiIdentity,
    entry: semantic_vocabulary::MachineId,
    review: effects::TerminalAuthorityClosureReviewReceipt,
}

impl AdmittedNativeCallableAuthority {
    fn from_admission(
        subject: terminal_psi::TerminalPsiIdentity,
        entry: semantic_vocabulary::MachineId,
        review: effects::TerminalAuthorityClosureReviewReceipt,
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

    pub const fn review(&self) -> &effects::TerminalAuthorityClosureReviewReceipt {
        &self.review
    }
}

pub struct AdmittedNativeProviders<'execution> {
    pub settlements: Vec<AdmittedBoundarySettlement<'execution>>,
    pub(crate) native_callbacks: crate::AdmittedNativeCallbackCohort, // Scoped to this executable occurrence scope, never the full retained catalog.
    pub(crate) installed_interrupts: crate::AdmittedInstalledInterruptCohort,
    pub executions: Vec<&'execution dyn ProviderExecutionEvidence>,
    pub terminal_authority_policy_identity: effects::TerminalAuthorityPolicyIdentity,
    pub terminal_authority_permission_policy_identity:
        Option<effects::TerminalAuthorityPermissionPolicyIdentity>, // `Some` only when a receiving permission policy was explicitly supplied.
    callable_authority: AdmittedNativeCallableAuthority,
    program_provider_context: std::sync::Arc<AdmittedProgramProviderContext>,
    pub installation: Option<AdmittedProviderInstallation>,
}

impl AdmittedNativeProviders<'_> {
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
    ) -> &effects::TerminalAuthorityClosureReviewReceipt {
        self.callable_authority.review()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AdmittedTerminalMechanism {
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub mechanism: effects::TerminalMechanismIdentity,
}

pub fn admit_native_providers<'request>(
    input: &NativeRealizationInput,
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
    require_reached_providers(
        input,
        request.profile,
        request.selected_provider_plans,
        &installed_interrupts,
        &interrupt_mechanisms,
    )?;
    let (lifecycle_settlements, lifecycle_mechanisms) =
        access_lifecycles::settle_native_lifecycles(
            input,
            installation.as_ref(),
            request.terminal_authority_policy,
        )?;
    let (settlements, executions, mut mechanisms, cohort_rows) =
        settlements::settle_provider_executions(input, request, artifact.view().identity())?;
    let mut settlements = settlements;
    settlements.extend(interrupt_settlements);
    mechanisms.extend(interrupt_mechanisms);
    let (builtin_settlements, builtin_mechanisms) =
        compiler_builtins::settle_compiler_builtins(input, request)?;
    settlements.extend(builtin_settlements);
    mechanisms.extend(builtin_mechanisms);
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
            && boundary.requires_checked_result(&module.structural_domains)
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
    let effective_policy = if cohort_rows.is_empty() {
        None
    } else {
        let mut rows = request.terminal_authority_policy.explicit_rows().to_vec();
        for row in cohort_rows {
            if !rows
                .iter()
                .any(|supplied| supplied.mechanism() == row.mechanism())
            {
                rows.push(row);
            }
        }
        Some(
            terminal_authority_policy::terminal_authority_policy_with_rows(rows).map_err(
                |error| {
                    vec![Diagnostic::error(format!(
                        "settled filesystem cohort mechanism rows do not construct the effective receiving terminal-authority policy: {error:?}"
                    ))]
                },
            )?,
        )
    };
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
    Ok(AdmittedNativeProviders {
        installed_interrupts,
        settlements,
        native_callbacks: crate::AdmittedNativeCallbackCohort {
            psi: input.plan().psi,
            callbacks: callback_entries::admit_entries(input, request, &entry_custody)
                .map_err(|reason| realization_error("callback entry admission", reason))?,
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
    input: &NativeRealizationInput,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    request: &ProviderAdmissionRequest<'_, '_>,
) -> Result<Option<AdmittedProviderInstallation>, Vec<Diagnostic>> {
    let plan = input.plan();
    let view = artifact.view();
    let dependencies = backend_plan::matching_folded_constant_dependencies(
        view,
        request.profile.target_name(),
        request.selected_provider_plans.plans().iter(),
    )
    .map_err(|reason| realization_error("folded constant realization closure", reason))?;
    if plan.provider_candidates.is_empty() || request.checked_adapters.is_empty() {
        if view.module().folded_constants.target.is_none() {
            return Ok(None);
        }
        let selection = terminal_installation::admit_provider_installation(view, &[], &[])
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
            &request
                .selected_provider_plans
                .assumed_binding_coordinates(view.module())
                .map_err(|reason| realization_error("assumed binding installation", reason))?
                .into_iter()
                .map(|(required, supplied_requirement)| {
                    terminal_installation::AssumedBindingSelection {
                        required,
                        supplied_requirement,
                    }
                })
                .collect::<Vec<_>>(),
        )
        .map_err(|error| {
            realization_error("checked-provider installation", format!("{error:?}"))
        })?;
    if view.module().folded_constants.target.is_some() {
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
    selected: &effects::SelectedProviderPlanFacts,
    installed_interrupts: &crate::AdmittedInstalledInterruptCohort,
    interrupt_mechanisms: &[AdmittedTerminalMechanism],
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
                let expected = effects::TerminalMechanismIdentity::CompilerIntrinsic(
                    effects::CompilerIntrinsicExecutionIdentity::StaticInterrupt(kind),
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
            let requirement = plan.identities.text(declaration.identity)?;
            (selected.selected_requirement_row(requirement).is_none()
                && !selected
                    .assumed_bindings()
                    .iter()
                    .any(|row| row.required == requirement))
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
