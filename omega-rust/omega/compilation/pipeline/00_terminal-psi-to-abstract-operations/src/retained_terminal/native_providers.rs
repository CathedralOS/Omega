//! Prepare a retained proposal's provider inputs before native admission.
//!
//! This operation owns package-custody joins and the callback, import and
//! builtin rosters. Derived binding evidence stays owned here until admission
//! and publication have consumed their borrowed execution records.
//! Imports are prepared once for the application/callback demand union, then
//! borrowed by canonical selected requirement. Each body's proposal retains
//! its own family, boundary and operation identities. Passing the application's
//! numeric rows to a callback would silently select unrelated local declarations;
//! preparing imports separately could bypass a complete supplied evidence roster.

use super::{TerminalNativeCallbackBody, TerminalNativeRealizationProposal};
use crate::provider_admission::{
    NativeCompilerBuiltinSettlement, NativeProviderSettlement, PreparedSourceEvaluatedBindings,
    ProviderAdmissionRequest, SourceEvaluatedImportSettlement, TerminalAuthorityPermissionPolicy,
    TerminalAuthorityPolicy, prepare_source_evaluated_bindings,
};
use diagnostics::Diagnostic;
use std::collections::BTreeSet;

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
    bindings: PreparedSourceEvaluatedBindings<'input>,
    program_demands: BTreeSet<String>,
    callbacks: &'input [crate::AdmittedNativeCallbackArgument],
    pub callback_thunks: Vec<backend_plan::NativeCallbackThunkSettlement<'input>>,
    compiler_builtins: Vec<NativeCompilerBuiltinSettlement<'input>>,
    callback_bodies: Vec<PreparedCallbackBody<'input>>,
}

struct PreparedCallbackBody<'input> {
    body: &'input TerminalNativeCallbackBody,
    demands: BTreeSet<String>,
    compiler_builtins: Vec<NativeCompilerBuiltinSettlement<'input>>,
}

impl PreparedRetainedNativeProviders<'_> {
    /// Project the shared execution evidence to this body's canonical selected
    /// requirements. Numeric Terminal IDs never cross artifact namespaces.
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

    /// Admit a private canonical body through the ordinary provider route.
    /// Imports were prepared once for the full demand closure; local rosters
    /// borrow that evidence without deriving replacements for missing supplies.
    /// No application entry is manufactured for the callback.
    pub fn admit_callback_body<'execution>(
        &'execution self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        terminal_authority_policy: &TerminalAuthorityPolicy,
        receiving_permissions: &Option<TerminalAuthorityPermissionPolicy>,
    ) -> Result<crate::AdmittedNativeRealization<'execution>, Vec<Diagnostic>> {
        let prepared = self
            .callback_bodies
            .iter()
            .find(|prepared| {
                prepared.body.artifact.artifact().manifest().identity()
                    == artifact.manifest().identity()
            })
            .ok_or_else(|| {
                diagnostics::error_in_context(
                    "Terminal callback custody",
                    "callback artifact has no retained native provider proposal",
                )
            })?;
        let proposal = &prepared.body.proposal;
        let settlements = self.settlements_for(&prepared.demands);
        let callbacks = proposal
            .occurrences
            .callback_arguments
            .as_deref()
            .ok_or_else(|| {
                diagnostics::error_in_context(
                    "Terminal callback custody",
                    "callback body placement has no private registrar materialization",
                )
            })?;
        crate::admit_native_realization(
            artifact,
            &crate::NativeAdmissionRequest {
                boundary_application_coverage: Some(&proposal.boundary_applications.coverage),
                behavior_exclusions: &proposal.behavior_exclusions,
                providers: ProviderAdmissionRequest {
                    profile: self.proposal.target_profile(),
                    selected_provider_plans: &proposal.providers.selection,
                    checked_adapters: &proposal.providers.checked_adapters,
                    external_binding_rows: &proposal.providers.external_bindings,
                    settlements: &settlements,
                    compiler_builtins: &prepared.compiler_builtins,
                    native_callbacks: callbacks,
                    terminal_authority_policy,
                    terminal_authority_permission_policy: receiving_permissions,
                    x86_scalar_fma_provider: proposal.providers.x86_scalar_fma,
                },
            },
        )
    }

    /// Assemble admission from the exact proposal this preparation consumed.
    /// Independent policies remain caller inputs; prepared rows grant no
    /// authority until `admit_native_realization` checks the complete request.
    /// The binding projection borrows this preparation's owned execution records
    /// and must remain alive through admission and realization.
    pub fn admission_request<'request>(
        &'request self,
        settlements: &'request [NativeProviderSettlement<'request>],
        terminal_authority_policy: &'request TerminalAuthorityPolicy,
        receiving_permissions: &'request Option<TerminalAuthorityPermissionPolicy>,
    ) -> crate::NativeAdmissionRequest<'request, 'request> {
        let proposal = self.proposal;
        crate::NativeAdmissionRequest {
            boundary_application_coverage: Some(&proposal.boundary_applications().coverage),
            behavior_exclusions: proposal.behavior_exclusions(),
            providers: ProviderAdmissionRequest {
                profile: proposal.target_profile(),
                selected_provider_plans: &proposal.providers().selection,
                checked_adapters: &proposal.providers().checked_adapters,
                external_binding_rows: proposal.providers().external_bindings.as_slice(),
                settlements,
                compiler_builtins: &self.compiler_builtins,
                native_callbacks: self.callbacks,
                terminal_authority_policy,
                terminal_authority_permission_policy: receiving_permissions,
                x86_scalar_fma_provider: proposal.providers().x86_scalar_fma,
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
        inputs: RetainedNativeProviderInputs<'input>,
    ) -> Result<PreparedRetainedNativeProviders<'input>, Vec<Diagnostic>> {
        // Proposal validation rejoins each IEEE comparison to its exact selected
        // intrinsic and checked arm/expression occurrence. The ordinary graph
        // lowering preserves that operation identity into the D29 physical child;
        // bitwise IEEE ordering needs no independent foreign-provider settlement.
        if self.post_terminal_optimizations().selections() != inputs.optimization_selections {
            return Err(diagnostics::error_in_context(
                "Terminal native proposal",
                "receiving lowerer selections differ from the exact post-Terminal build proposal",
            ));
        }
        crate::provider_admission::terminal_authority_permissions::validate_retained_package_terminal_authority_permissions(
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

        let module = artifact.semantic_module();
        // Execution records belong to selected provider rows. Calls keep their
        // original head identities, joined through the retained exact family
        // and specialization relations rather than renamed in Terminal Psi.
        let program_demands = selected_demands(module, &self.providers().selection)?;
        let mut demanded = program_demands.clone();
        let mut callback_bodies = Vec::with_capacity(self.callback_bodies.len());
        for body in &self.callback_bodies {
            let demands = selected_demands(
                body.artifact.artifact().semantic_module(),
                &body.proposal.providers.selection,
            )?;
            demanded.extend(demands.iter().cloned());
            callback_bodies.push(PreparedCallbackBody {
                body,
                demands,
                compiler_builtins: crate::provider_admission::prepare_retained_compiler_builtins(
                    &body.proposal.providers.compiler_builtins,
                    body.proposal.providers.selection.plans(),
                )?,
            });
        }
        // Supplied evidence must cover the union exactly once. Preparing each
        // body independently would treat an empty projection as permission to
        // mint evidence, bypassing the caller's complete supplied roster.
        let bindings = prepare_source_evaluated_bindings(
            self.providers().selection.plans(),
            &demanded,
            inputs.imports,
            self.providers().selection.identities(),
        )?;

        let compiler_builtins = crate::provider_admission::prepare_retained_compiler_builtins(
            self.providers().compiler_builtins.as_slice(),
            self.providers().selection.plans(),
        )?;

        Ok(PreparedRetainedNativeProviders {
            proposal: self,
            bindings,
            program_demands,
            callbacks,
            callback_thunks,
            compiler_builtins,
            callback_bodies,
        })
    }
}

fn selected_demands(
    module: &terminal_psi::TerminalModule,
    selection: &effects::SelectedProviderPlanFacts,
) -> Result<BTreeSet<String>, Vec<Diagnostic>> {
    Ok(
        crate::provider_admission::demanded_boundary_identities(module)?
            .iter()
            .map(|identity| selection.selected_requirement_identity(identity).to_owned())
            .collect(),
    )
}
