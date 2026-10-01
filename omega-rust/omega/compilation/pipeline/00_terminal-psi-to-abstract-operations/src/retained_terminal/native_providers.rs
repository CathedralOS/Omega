//! Prepare a retained proposal's provider inputs before native admission.
//!
//! This operation owns package-custody joins and the callback, import and
//! builtin rosters. Derived binding evidence stays owned here until admission
//! and publication have consumed their borrowed execution records.

use super::TerminalNativeRealizationProposal;
use crate::provider_admission::{
    NativeCompilerBuiltinSettlement, NativeProviderSettlement, PreparedSourceEvaluatedBindings,
    ProviderAdmissionRequest, SourceEvaluatedImportSettlement, TerminalAuthorityPermissionPolicy,
    TerminalAuthorityPolicy, prepare_source_evaluated_bindings,
};
use diagnostics::Diagnostic;

/// Independent package and realization inputs; none can replace retained plans.
/// Receiver policy is supplied later to admission of the demanded closure.
pub struct RetainedNativeProviderInputs<'input> {
    pub optimization_selections: &'input optimization_core::PostTerminalOptimizationSelections,
    pub accepted_package_permissions: &'input TerminalAuthorityPermissionPolicy,
    pub imports: &'input [SourceEvaluatedImportSettlement<'input>],
}

/// Invocation-owned preparation, not provider or publication authority.
/// Native admission independently checks every borrowed settlement.
pub struct PreparedRetainedNativeProviders<'input> {
    proposal: &'input TerminalNativeRealizationProposal,
    pub bindings: PreparedSourceEvaluatedBindings<'input>,
    callbacks: Vec<crate::AdmittedNativeCallbackArgument>,
    pub callback_thunks: Vec<backend_plan::NativeCallbackThunkSettlement<'input>>,
    compiler_builtins: Vec<NativeCompilerBuiltinSettlement<'input>>,
}

impl PreparedRetainedNativeProviders<'_> {
    /// Assemble admission from the exact proposal this preparation consumed.
    /// Independent policies remain caller inputs; prepared rows grant no
    /// authority until `admit_native_realization` checks the complete request.
    /// The binding projection borrows this preparation's owned execution records
    /// and must remain alive through admission and realization.
    pub fn admission_request<'request>(
        &'request self,
        profile: &'request proof_admission::AdmissionProfile,
        settlements: &'request [NativeProviderSettlement<'request>],
        terminal_authority_policy: &'request TerminalAuthorityPolicy,
        receiving_permissions: &'request Option<TerminalAuthorityPermissionPolicy>,
    ) -> crate::NativeAdmissionRequest<'request> {
        let proposal = self.proposal;
        crate::NativeAdmissionRequest {
            checked_scope: Some(&proposal.boundary_applications().scope),
            prepared_input: None,
            boundary_application_coverage: Some(&proposal.boundary_applications().coverage),
            behavior_exclusions: proposal.behavior_exclusions(),
            providers: ProviderAdmissionRequest {
                target: proposal.native_target(),
                profile,
                program_entry: proposal.program_entry().as_settlement(),
                selected_provider_plans: &proposal.providers().selection,
                external_binding_rows: proposal.providers().external_bindings.as_slice(),
                settlements,
                compiler_builtins: &self.compiler_builtins,
                native_callbacks: &self.callbacks,
                terminal_authority_policy,
                terminal_authority_permission_policy: receiving_permissions,
            },
        }
    }
}

impl TerminalNativeRealizationProposal {
    /// Rejoin retained plans to exact package acceptance and execution inputs.
    /// The caller retains the artifact and proposal throughout native realization.
    pub fn prepare_native_providers<'input>(
        &'input self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        callback_placements: &'input [backend_plan::BoundNominalCallbackPlacement],
        inputs: RetainedNativeProviderInputs<'input>,
    ) -> Result<PreparedRetainedNativeProviders<'input>, Vec<Diagnostic>> {
        // Proposal validation rejoins each IEEE comparison to its exact selected
        // intrinsic and checked arm/expression occurrence. The ordinary graph
        // lowering preserves that operation identity into the D29 physical child;
        // bitwise IEEE ordering needs no independent foreign-provider settlement.
        if self.post_terminal_optimizations().selections() != inputs.optimization_selections {
            return Err(diagnostic(
                "Terminal native proposal",
                "receiving lowerer selections differ from the exact post-Terminal build proposal",
            ));
        }
        crate::provider_admission::terminal_authority_permissions::validate_retained_package_terminal_authority_permissions(
            self.package_terminal_authority_permissions(),
            inputs.accepted_package_permissions,
        )?;
        let native_callbacks = crate::provider_admission::admit_native_callback_arguments(
            callback_placements,
            self.occurrences().callbacks.as_slice(),
        )?;
        let callback_thunks = backend_plan::admitted_native_callback_thunks(
            callback_placements,
            self.occurrences().callbacks.as_slice(),
        )
        .map_err(|error| diagnostic("Terminal callback thunk custody", error))?;
        debug_assert_eq!(callback_placements.len(), native_callbacks.len());
        debug_assert_eq!(callback_placements.len(), callback_thunks.len());

        let module = terminal_codec::decode_module(artifact.semantic_bytes()).map_err(|error| {
            diagnostic(
                "Terminal native proposal",
                format!("canonical semantics could not be decoded: {error}"),
            )
        })?;
        let demanded = crate::provider_admission::demanded_boundary_identities(&module)?;
        let bindings = prepare_source_evaluated_bindings(
            self.providers().selection.plans(),
            &demanded,
            inputs.imports,
        )?;

        let compiler_builtins = crate::provider_admission::prepare_retained_compiler_builtins(
            self.providers().compiler_builtins.as_slice(),
            self.providers().selection.plans(),
        )?;

        Ok(PreparedRetainedNativeProviders {
            proposal: self,
            bindings,
            callbacks: native_callbacks,
            callback_thunks,
            compiler_builtins,
        })
    }
}

fn diagnostic(context: &str, message: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!("{context}: {message}"))]
}
