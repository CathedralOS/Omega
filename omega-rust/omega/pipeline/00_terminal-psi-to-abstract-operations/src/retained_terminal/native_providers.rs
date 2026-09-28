//! Prepare a retained proposal's provider inputs before native admission.
//!
//! This operation owns receiving-policy joins and the callback, import and
//! builtin rosters. Derived import evidence stays owned here until admission
//! and publication have consumed their borrowed execution records.

use super::TerminalNativeRealizationProposal;
use crate::provider_admission::{
    NativeCompilerBuiltinSettlement, PreparedSourceEvaluatedImports,
    SourceEvaluatedImportSettlement, TerminalAuthorityPermissionPolicy,
    prepare_source_evaluated_imports,
};
use diagnostics::Diagnostic;

/// Independent receiving inputs; none can select or replace retained plans.
/// No receiving policy means no receiver-admission claim, not an empty policy.
pub struct RetainedNativeProviderInputs<'input> {
    pub optimization_selections: &'input optimization_core::PostTerminalOptimizationSelections,
    pub accepted_package_permissions: &'input TerminalAuthorityPermissionPolicy,
    pub receiving_permissions: Option<&'input TerminalAuthorityPermissionPolicy>,
    pub imports: &'input [SourceEvaluatedImportSettlement<'input>],
}

/// Invocation-owned preparation, not provider or publication authority.
/// Native admission independently checks every borrowed settlement.
pub struct PreparedRetainedNativeProviders<'input> {
    pub imports: PreparedSourceEvaluatedImports<'input>,
    pub callbacks: Vec<crate::AdmittedNativeCallbackArgument>,
    pub callback_thunks: Vec<backend_plan::NativeCallbackThunkSettlement<'input>>,
    pub compiler_builtins: Vec<NativeCompilerBuiltinSettlement<'input>>,
}

impl TerminalNativeRealizationProposal {
    /// Rejoin retained plans to the receiver's exact policies and execution inputs.
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
        // The receiver-admission join runs only under an explicitly supplied
        // receiving policy. `None` leaves the accepted package rows as package
        // evidence only; they are never silently projected into receiver
        // permissions.
        if let Some(receiving_policy) = inputs.receiving_permissions {
            crate::provider_admission::terminal_authority_permissions::validate_package_terminal_authority_permissions(
                inputs.accepted_package_permissions
                    .rows()
                    .iter(),
                receiving_policy,
            )?;
        }
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
        let prepared_imports = prepare_source_evaluated_imports(
            self.providers().selection.plans(),
            &demanded,
            inputs.imports,
        )?;

        let compiler_builtins = crate::provider_admission::prepare_retained_compiler_builtins(
            self.providers().compiler_builtins.as_slice(),
            self.providers().selection.plans(),
        )?;

        Ok(PreparedRetainedNativeProviders {
            imports: prepared_imports,
            callbacks: native_callbacks,
            callback_thunks,
            compiler_builtins,
        })
    }
}

fn diagnostic(context: &str, message: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!("{context}: {message}"))]
}
