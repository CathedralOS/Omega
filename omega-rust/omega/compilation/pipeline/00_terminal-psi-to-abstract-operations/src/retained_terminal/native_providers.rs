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
    compiler_builtins: Vec<crate::provider_admission::TerminalCompilerBuiltinProposal>,
    local_adapters: Vec<crate::SelectedProviderAdapter>,
}

impl PreparedNativeExecution {
    /// Lower another callable in this prepared execution namespace. In
    /// particular, callbacks preserve separately emitted component crossings;
    /// they cannot restore the proposal's excluded local provider bodies.
    pub fn lower_callable(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        proposal: &TerminalNativeRealizationProposal,
        entry: terminal_psi::TerminalCallable,
        scalar_applications: Option<crate::NativeScalarApplications<'_>>,
    ) -> Result<crate::VerifiedNativeArtifactInput, Vec<Diagnostic>> {
        let selection = crate::NativeExecutionSelection {
            scalar_applications,
            providers: &self.local_adapters,
            plans: &proposal.providers().selection,
            callbacks: &proposal.occurrences().callbacks,
            entry_call: None,
        };
        let scope = crate::artifact_admission::NativeScopePreparation::prepare(
            artifact,
            proposal.target_profile(),
            entry,
            &selection,
        )
        .map_err(crate::native_admission::artifact_lowering_error)?;
        let input = scope
            .lower(artifact, &[], &selection)
            .map_err(crate::native_admission::artifact_lowering_error)?;
        input
            .bind_selected_provider_coordinates(&self.selection)
            .map_err(|reason| vec![Diagnostic::error(reason)])
    }
}

/// Selected native work and its original proposal, retained across scalar
/// completion. Lowering consumes this preparation; it does not select again.
pub struct PreparedNativeWork<'input> {
    artifact: &'input terminal_codec::CanonicalTerminalArtifact,
    proposal: &'input TerminalNativeRealizationProposal,
    scope: crate::artifact_admission::NativeScopePreparation,
    execution: PreparedNativeExecution,
    entry_call: Option<crate::SelectedProgramEntryCall>,
    private_entry_ram: Vec<terminal_installation::CheckedPrivateEntryRamRecipe>,
}

impl PreparedNativeWork<'_> {
    /// Scalar reads, entry obligations and physical storage extents of this
    /// exact native work domain. Unselected catalog types impose no demand.
    pub fn scalar_application_demand(&self) -> &[semantic_vocabulary::ScalarApplicationId] {
        &self.scope.scalar_demand
    }

    pub fn lower(
        self,
        scalar_applications: Option<crate::NativeScalarApplications<'_>>,
    ) -> Result<(crate::VerifiedNativeArtifactInput, PreparedNativeExecution), Vec<Diagnostic>>
    {
        let input = self
            .scope
            .lower_with_private_entry_ram(
                self.artifact,
                &self.private_entry_ram,
                &crate::NativeExecutionSelection {
                    scalar_applications,
                    providers: &self.execution.local_adapters,
                    // Scalar completion consumed the original whole companion,
                    // not the native-only direct-leaf augmentation below.
                    plans: &self.proposal.providers().selection,
                    callbacks: &self.proposal.occurrences().callbacks,
                    entry_call: self.entry_call.as_ref(),
                },
            )
            .map_err(crate::native_admission::artifact_lowering_error)?;
        let input = input
            .bind_selected_provider_coordinates(&self.execution.selection)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        Ok((input, self.execution))
    }
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
            private_entry_ram: &[],
            entry: terminal_psi::TerminalCallable::Machine(
                proposal.program_entry().checked_entry().terminal_entry(),
            ),
            boundary_application_coverage: Some(&proposal.boundary_applications().coverage),
            behavior_exclusions: proposal.behavior_exclusions(),
            providers: ProviderAdmissionRequest {
                profile: proposal.target_profile(),
                selected_provider_plans: &self.execution.selection,
                checked_adapters: &self.execution.local_adapters,
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
    /// A checked component crossing is an executable scope boundary. Its
    /// producer closure is emitted separately, never added to the receiver's
    /// local provider roots. Admission still owes the selected call settlement.
    pub fn prepare_native_execution_with_components<'input>(
        &'input self,
        artifact: &'input terminal_codec::CanonicalTerminalArtifact,
        private_entry_ram: &[terminal_installation::CheckedPrivateEntryRamRecipe],
        components: &[&super::AdmittedNativeComponentCall<'_>],
    ) -> Result<PreparedNativeWork<'input>, Vec<Diagnostic>> {
        if components.iter().any(|component| {
            component.selection().receiver()
                != &terminal_codec::TerminalExecutionIdentity::portable(artifact.view().identity())
        }) {
            return Err(vec![Diagnostic::error(
                "component scope selection names another receiving artifact",
            )]);
        }
        for (position, component) in components.iter().enumerate() {
            if components[..position].iter().any(|earlier| {
                earlier.selection().receiving_boundary()
                    == component.selection().receiving_boundary()
            }) || self
                .providers()
                .checked_adapters
                .iter()
                .filter(|adapter| component.replaces_local_adapter(adapter))
                .count()
                != 1
            {
                return Err(vec![Diagnostic::error(
                    "component scope selection has competing or missing exact local adapter coordinates",
                )]);
            }
        }
        let local_adapters = self
            .providers()
            .checked_adapters
            .iter()
            .filter(|adapter| {
                !components
                    .iter()
                    .any(|component| component.replaces_local_adapter(adapter))
            })
            .cloned()
            .collect::<Vec<_>>();
        let entry = self.program_entry().as_settlement();
        let root = entry.physical_adapter().map_or(
            terminal_psi::TerminalCallable::Machine(
                self.program_entry().checked_entry().terminal_entry(),
            ),
            |adapter| terminal_psi::TerminalCallable::Machine(adapter.machine()),
        );
        let semantic_call = crate::SelectedProgramEntryCall::bind(
            artifact,
            self.program_entry(),
            &self.providers().selection,
            &self.providers().intrinsic_executions,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let scope = crate::artifact_admission::NativeScopePreparation::prepare(
            artifact,
            self.target_profile(),
            root,
            &crate::NativeExecutionSelection {
                scalar_applications: None,
                providers: &local_adapters,
                plans: &self.providers().selection,
                callbacks: &self.occurrences().callbacks,
                entry_call: semantic_call.as_ref(),
            },
        )
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "native execution input: {error:?}"
            ))]
        })?;
        let execution = self.prepare_native_provider_demand(artifact, &scope, local_adapters)?;
        Ok(PreparedNativeWork {
            artifact,
            proposal: self,
            scope,
            execution,
            entry_call: semantic_call,
            private_entry_ram: private_entry_ram.to_vec(),
        })
    }

    /// Prepare an exported callable with ordinary parameter/result transport,
    /// independently of the component's application ProgramEntry. This chooses
    /// demand only; it grants no externally callable address or resource supply.
    pub fn prepare_native_callable_execution<'input>(
        &'input self,
        artifact: &'input terminal_codec::CanonicalTerminalArtifact,
        entry: terminal_psi::TerminalCallable,
    ) -> Result<PreparedNativeWork<'input>, Vec<Diagnostic>> {
        let scope = crate::artifact_admission::NativeScopePreparation::prepare(
            artifact,
            self.target_profile(),
            entry,
            &crate::NativeExecutionSelection {
                scalar_applications: None,
                providers: &self.providers().checked_adapters,
                plans: &self.providers().selection,
                callbacks: &self.occurrences().callbacks,
                entry_call: None,
            },
        )
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "component callable execution input: {error:?}",
            ))]
        })?;
        let execution = self.prepare_native_provider_demand(
            artifact,
            &scope,
            self.providers().checked_adapters.clone(),
        )?;
        Ok(PreparedNativeWork {
            artifact,
            proposal: self,
            scope,
            execution,
            entry_call: None,
            private_entry_ram: Vec::new(),
        })
    }

    /// Retain the selected body's exact boundary demand and its original whole
    /// plans. Program and callable entries share this operation; neither route
    /// substitutes all catalog declarations for the executable demand closure.
    fn prepare_native_provider_demand(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        scope: &crate::artifact_admission::NativeScopePreparation,
        local_adapters: Vec<crate::SelectedProviderAdapter>,
    ) -> Result<PreparedNativeExecution, Vec<Diagnostic>> {
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
        for boundary in module
            .boundary_machines
            .iter()
            .filter(|boundary| scope.scope.contains_boundary(boundary.id))
        {
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
            direct_demands.insert(name.to_owned());
        }
        let mut direct = Vec::new();
        let mut external_bindings = self.providers().external_bindings.clone();
        for leaf in &self.providers().direct_boundary_leaves {
            if leaf.plan.rows.iter().any(|row| {
                direct_demands.contains(leaf.plan.identities.text(row.requirement_identity))
            }) {
                direct.push(leaf.plan.clone());
                if let backend_plan::DirectBoundaryExecution::Foreign(binding) = &leaf.execution {
                    external_bindings.push(binding.clone()?);
                }
            }
        }
        let selection = self
            .providers()
            .selection
            .clone()
            .with_direct_boundary_plans(direct)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        // Selection may reorder plans when executable direct dependencies
        // join. Rejoin complete plans, then derive every builtin in that one
        // namespace; no saved plan ordinal crosses this selection boundary.
        let non_intrinsic_execution = [None];
        let non_authored_realization = [None];
        let intrinsic_provenance = selection
            .plans()
            .iter()
            .map(|plan| -> Result<_, Vec<Diagnostic>> {
                if let Some(position) = self
                    .providers()
                    .selection
                    .plans()
                    .iter()
                    .position(|original| original == plan)
                {
                    let row_executions = self
                        .providers()
                        .intrinsic_executions
                        .get(position)
                        .ok_or_else(|| {
                            vec![Diagnostic::error(
                                "native execution lost original intrinsic row custody",
                            )]
                        })?;
                    let row_numeric_realizations = self
                        .providers()
                        .numeric_realizations
                        .get(position)
                        .ok_or_else(|| {
                            vec![Diagnostic::error(
                                "native execution lost selected external declaration custody",
                            )]
                        })?;
                    return Ok(crate::provider_admission::CompilerIntrinsicPlanProvenance {
                        plan,
                        semantic_schema: &plan.schema,
                        row_executions,
                        row_numeric_realizations,
                    });
                }
                let leaf = self
                    .providers()
                    .direct_boundary_leaves
                    .iter()
                    .find(|leaf| leaf.plan == *plan)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "native execution has an unowned direct plan",
                        )]
                    })?;
                let row_executions: &[Option<effects::CompilerIntrinsicExecutionIdentity>] =
                    match &leaf.execution {
                        backend_plan::DirectBoundaryExecution::Intrinsic(identity) => {
                            std::slice::from_ref(identity)
                        }
                        backend_plan::DirectBoundaryExecution::Foreign(_) => {
                            &non_intrinsic_execution
                        }
                    };
                Ok(crate::provider_admission::CompilerIntrinsicPlanProvenance {
                    plan,
                    semantic_schema: &leaf.semantic_plan.schema,
                    row_executions,
                    row_numeric_realizations: &non_authored_realization,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let compiler_builtins =
            crate::provider_admission::derive_selected_intrinsic_settlement_proposals(
                &selection,
                intrinsic_provenance.into_iter(),
                &demands,
                &module.boundary_machines,
                &module.numeric_realizations,
                &module.identities,
            )?;
        // Builtin occurrences bind the actual boundary IDs, including family
        // heads. Only provider settlement demand uses normalized row identity.
        let demands = demands
            .into_iter()
            .map(|identity| {
                selection
                    .selected_requirement_identity(&identity)
                    .to_owned()
            })
            .collect();
        Ok(PreparedNativeExecution {
            selection,
            external_bindings,
            demands,
            compiler_builtins,
            local_adapters,
        })
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
            inputs.execution.compiler_builtins.as_slice(),
            inputs.execution.selection.plans(),
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
