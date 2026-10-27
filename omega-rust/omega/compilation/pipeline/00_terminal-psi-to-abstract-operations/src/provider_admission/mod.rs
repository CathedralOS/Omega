//! Provider admission for native realization.
//!
//! `source_evaluated_bindings` prepares selected import and syscall executions
//! for direct compilation and retained Terminal re-entry. Only imported
//! user-space callees carry a foreign same-stack reserve.
//! `admit_native_providers` first requires a selected provider for every
//! boundary requirement the entry reaches (`provider_supply`), then
//! independently checks those inputs, settles external
//! executions, and admits the exact checked-provider installation and authority
//! closure retained by the plan.
//! `project_native_callback_arguments` projects exact retained callback
//! destinations for the lowerer's independent target/application replay.
//! Retained builtin proposals keep their plan-index custody here as well;
//! preparation never replaces independent native admission.

mod adapters;
mod callable_authority;
pub(crate) use adapters::retain_selected_provider_adapters;
pub use callable_authority::AdmittedNativeCallableAuthority;
mod callback_arguments;
mod compiler_builtins;
mod diagnostics;
mod installation;
mod placement_lifecycles;
mod provider_supply;
mod request;
mod settlements;
mod source_evaluated_bindings;
mod terminal_authority_permission_policy;
pub mod terminal_authority_permissions;
mod terminal_authority_policy;
mod terminal_authority_review;

pub(crate) use callback_arguments::project_native_callback_arguments;
pub use compiler_builtins::{
    CompilerIntrinsicPlanProvenance, TerminalCompilerBuiltinProposal,
    TerminalCompilerBuiltinSettlement,
};
pub(crate) use compiler_builtins::{
    derive_selected_intrinsic_settlement_proposals, prepare_retained_compiler_builtins,
};
pub use diagnostics::realization_error;

pub(crate) use source_evaluated_bindings::PreparedSourceEvaluatedBindings;
pub use source_evaluated_bindings::SourceEvaluatedImportSettlement;
pub(crate) use source_evaluated_bindings::prepare_source_evaluated_bindings;

pub use request::{NativeBoundaryRealization, NativeCompilerBuiltinSettlement};
pub use request::{NativeProviderSettlement, ProviderAdmissionRequest};
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
mod model;
use crate::AdmittedProviderInstallation;
use crate::VerifiedNativeArtifactInput as NativeRealizationInput;
use ::diagnostics::Diagnostic;
use installation_evidence::ProviderExecutionEvidence;
pub use model::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedNativeCallbackArgument,
    NativeCallbackArgumentProposal,
};

pub struct AdmittedNativeProviders<'execution> {
    pub settlements: Vec<AdmittedBoundarySettlement<'execution>>,
    /// Callback arguments for the same executable occurrence scope as the
    /// settlements and checked installation, never the full retained catalog.
    pub(crate) native_callbacks: Vec<AdmittedNativeCallbackArgument>,
    pub executions: Vec<&'execution dyn ProviderExecutionEvidence>,
    pub terminal_authority_policy_identity: effects::TerminalAuthorityPolicyIdentity,
    /// `Some` only when a receiving permission policy was explicitly supplied;
    /// `None` means the artifact makes no receiver-admission claim.
    pub terminal_authority_permission_policy_identity:
        Option<effects::TerminalAuthorityPermissionPolicyIdentity>,
    callable_authority: AdmittedNativeCallableAuthority,
    pub installation: Option<AdmittedProviderInstallation>,
}

impl AdmittedNativeProviders<'_> {
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

pub(crate) fn admit_native_providers<'request>(
    input: &NativeRealizationInput,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    request: &ProviderAdmissionRequest<'_, 'request>,
) -> Result<AdmittedNativeProviders<'request>, Vec<Diagnostic>> {
    let terminal_artifact_identity = *artifact.manifest().identity().as_bytes();
    provider_supply::require_reached_providers(
        input.optimization_input(),
        request.profile,
        request.selected_provider_plans,
    )?;
    let installation = installation::admit_checked_provider_installation(input, artifact, request)?;
    let (lifecycle_settlements, lifecycle_mechanisms) =
        placement_lifecycles::settle_native_lifecycles(
            input,
            installation.as_ref(),
            request.terminal_authority_policy,
        )?;
    let (settlements, executions, mut mechanisms, cohort_rows) =
        settlements::settle_provider_executions(input, request, artifact.view().identity())?;
    let mut settlements = settlements;
    let (builtin_settlements, builtin_mechanisms) =
        compiler_builtins::settle_compiler_builtins(input, request)?;
    settlements.extend(builtin_settlements);
    mechanisms.extend(builtin_mechanisms);
    for settlement in &settlements {
        // A sealed component call executes the selected checked export, which
        // can forward an existing qualification. Its installation already joined
        // the result theories; raw native providers still need their own issuer.
        let checked_component = matches!(
            &settlement.realization,
            target_operations::BoundarySettlementRealization::NormalizedForeignCall(binding)
                if matches!(&binding.mechanism,
                    target_operations::NormalizedForeignCallMechanism::Component { .. })
        );
        let module = input.context().module();
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
    // Opaque provider supply above cannot manufacture another issuer's
    // qualified result. Private lifecycle operations instead implement the
    // authenticated core result contract: rejection returns the actual inputs,
    // initialization establishes content only after writing it, and extraction
    // transfers the original resident and grant. Their operation-owned
    // settlement is not a host-supplied claim about equal-shaped result bytes.
    settlements.extend(lifecycle_settlements);
    mechanisms.extend(lifecycle_mechanisms);
    // The toolchain-settled `FilesystemHost` cohort rows emitted for demanded
    // leaves join the supplied receiving rows into the effective policy the
    // closure review and the artifact identity bind. The supplied policy is
    // the receiving authority this same process constructed, so its rows
    // classify first: a cohort row joins only on a mechanism the supplied
    // rows do not already name.
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
            terminal_authority_policy::terminal_authority_policy_with_rows(
                rows,
            )
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "settled filesystem cohort mechanism rows do not construct the effective receiving terminal-authority policy: {error:?}"
                ))]
            })?,
        )
    };
    let effective_policy = effective_policy
        .as_ref()
        .unwrap_or(request.terminal_authority_policy);
    let terminal_authority_closure_review =
        terminal_authority_review::review_terminal_authority_closure(
            terminal_artifact_identity,
            request.profile,
            input.optimization_input(),
            request.selected_provider_plans,
            effective_policy,
            request.terminal_authority_permission_policy.as_ref(),
            &mechanisms,
            installation
                .as_ref()
                .map(AdmittedProviderInstallation::installed_candidates)
                .unwrap_or_default(),
            request.settlements,
        )
        .map_err(|error| {
            diagnostics::realization_error("terminal-authority closure review", error)
        })?;
    Ok(AdmittedNativeProviders {
        settlements,
        native_callbacks: request
            .native_callbacks
            .iter()
            .cloned()
            .map(AdmittedNativeCallbackArgument::admit)
            .collect(),
        executions,
        terminal_authority_policy_identity: effective_policy.identity(),
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
    })
}
