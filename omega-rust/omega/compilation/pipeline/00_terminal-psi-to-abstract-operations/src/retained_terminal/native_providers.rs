//! Prepare a retained proposal's provider inputs before native admission.
//!
//! This operation owns package-custody joins and the callback, import and
//! builtin rosters. Derived binding evidence stays owned here until admission
//! and publication have consumed their borrowed execution records.
//! One program owns all callback bodies and their nominal identities. Imports,
//! selected plans and builtin evidence are prepared once for that program;
//! native roots select demand from the same namespace.

use super::TerminalNativeRealizationProposal;
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
    pub execution: &'input PreparedNativeExecution,
    pub optimization_selections: &'input optimization_core::PostTerminalOptimizationSelections,
    pub accepted_package_permissions: &'input TerminalAuthorityPermissionPolicy,
    pub imports: &'input [SourceEvaluatedImportSettlement<'input>],
}

/// The invocation's one executable projection and its demanded provider rows.
/// Preparation owns no execution evidence and grants no terminal authority.
pub struct PreparedNativeExecution {
    selection: effects::SelectedProviderPlanFacts,
    external_bindings: Vec<calling_conventions::ExternalBindingRow>,
    demands: BTreeSet<String>,
}

/// Invocation-owned preparation, not provider or publication authority.
/// Native admission independently checks every borrowed settlement.
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
            receiving_provider_roots: &[],
            private_entry_ram: &[],
            entry: terminal_psi::TargetMachineCallable::Machine(
                proposal.program_entry().checked_entry().terminal_entry(),
            ),
            boundary_application_coverage: Some(&proposal.boundary_applications().coverage),
            behavior_exclusions: proposal.behavior_exclusions(),
            providers: ProviderAdmissionRequest {
                profile: proposal.target_profile(),
                selected_provider_plans: &self.execution.selection,
                checked_adapters: &proposal.providers().checked_adapters,
                external_binding_rows: &self.execution.external_bindings,
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
    /// Select work before preparing foreign execution records. Complete retained
    /// declarations are not executable demand, and a foreign leaf is not an
    /// alternative implementation of the wrapper that happens to call it.
    pub fn prepare_native_execution(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        private_entry_ram: &[terminal_installation::CheckedPrivateEntryRamRecipe],
    ) -> Result<(crate::VerifiedNativeArtifactInput, PreparedNativeExecution), Vec<Diagnostic>>
    {
        let entry = self.program_entry().as_settlement();
        let root = entry.physical_adapter().map_or(
            terminal_psi::TargetMachineCallable::Machine(
                self.program_entry().checked_entry().terminal_entry(),
            ),
            |adapter| terminal_psi::TargetMachineCallable::Machine(adapter.machine()),
        );
        let roots = entry
            .receiving_provider_roots(artifact)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let input = crate::VerifiedNativeArtifactInput::prepare_with_private_entry_ram(
            artifact,
            self.target_profile(),
            root,
            private_entry_ram,
        )
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "native execution input: {error:?}"
            ))]
        })?
        .select_target_execution(
            artifact,
            self.target_profile(),
            &self.providers().checked_adapters,
            &self.providers().selection,
            &roots,
            &self.occurrences().callbacks,
        )
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
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
        for boundary in &input.plan().boundary_machines {
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
            match input
                .optimization_input()
                .target_family_member(boundary.id)
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
        for leaf in &self.providers().direct_foreign_leaves {
            if leaf.plan.rows.iter().any(|row| {
                direct_demands.contains(leaf.plan.identities.text(row.requirement_identity))
            }) {
                direct.push(leaf.plan.clone());
                external_bindings.push(leaf.external_binding.clone()?);
            }
        }
        let mut selection = self
            .providers()
            .selection
            .clone()
            .with_direct_foreign_plans(direct)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let mut family_rows = selection.family_realization_rows().to_vec();
        let mut identities = selection.identities().clone().into_builder();
        for (head, member) in family_members {
            let Some((plan, row)) = selection.selected_requirement_row(identity(member)?) else {
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
        let input = input
            .bind_selected_provider_coordinates(artifact, &selection)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let demands = demands
            .into_iter()
            .map(|identity| {
                selection
                    .selected_requirement_identity(&identity)
                    .to_owned()
            })
            .collect();
        Ok((
            input,
            PreparedNativeExecution {
                selection,
                external_bindings,
                demands,
            },
        ))
    }

    /// Rejoin retained plans to exact package acceptance and execution inputs.
    /// The caller retains the artifact and proposal throughout native realization.
    pub fn prepare_native_providers<'input>(
        &'input self,
        _artifact: &terminal_codec::CanonicalTerminalArtifact,
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

        // Execution records belong to selected provider rows. Calls keep their
        // original head identities, joined through the retained exact family
        // and specialization relations rather than renamed in Terminal Psi.
        let program_demands = inputs.execution.demands.clone();
        // Supplied evidence covers the program's complete demand union once.
        // Per-root native admission projects it without minting missing rows.
        let bindings = prepare_source_evaluated_bindings(
            inputs.execution.selection.plans(),
            &program_demands,
            inputs.imports,
            inputs.execution.selection.identities(),
        )?;

        let compiler_builtins = crate::provider_admission::prepare_retained_compiler_builtins(
            self.providers().compiler_builtins.as_slice(),
            self.providers().selection.plans(),
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
