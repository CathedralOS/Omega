//! Admit and project checked product custody for Terminal production and handoff.
//!
//! These projections supply existing stage inputs; they do not lower code,
//! produce callback bodies, execute native proposal production, or assemble a
//! report — `CompileReportSettlement` owns report construction.
//! Omega stage 00 joins the recorded occurrences to provider selection.

use super::CheckedCompilation;
use crate::TerminalCallbackThunkArtifact;
use diagnostics::Diagnostic;
use terminal_psi_to_abstract_operations::{
    BoundaryApplicationSettlement, TerminalNativeProposalRequest,
};

impl CheckedCompilation {
    /// Admit the checked product for Terminal production and borrow its exact
    /// entry. This is product eligibility, not proof or native admission of the
    /// artifact that Psi has yet to produce.
    pub fn admit_terminal_production(
        &self,
    ) -> Result<&provider_planning::program_entry::SelectedCompilerProgramEntry, Vec<Diagnostic>>
    {
        // An Independent edge has no product carrier yet. Reject it before
        // checking entry custody or allowing a fused artifact to be produced.
        let execution = self.execution();
        provider_planning::verify_selected_compositions_are_realized(
            execution.selected_provider_provenance.as_slice(),
        )?;
        // Preserve the build operand's lexical package choice, including when
        // another package contains a machine with the same display name.
        let entry = execution.selected_program_entry.as_ref().ok_or_else(|| {
            vec![Diagnostic::error(
                "terminal-artifact production requires one exact selected program entry",
            )]
        })?;
        Ok(entry)
    }

    /// Borrow the exact checked selections for a produced Terminal occurrence
    /// roster. Supplied callbacks remain candidates, not admitted native code;
    /// stage 00 must check their cardinality, identities and source associations.
    pub fn terminal_native_proposal_request<'input>(
        &'input self,
        callback_placements: &[provider_planning::calling_policy_plans::SelectedCallbackPlacement],
        callback_thunks: Vec<TerminalCallbackThunkArtifact>,
        selections: &optimization_core::OptimizationSelections,
    ) -> Result<TerminalNativeProposalRequest<'input>, Vec<Diagnostic>> {
        let execution = self.execution();
        let providers = self.providers();
        let (
            package_terminal_authority_permissions,
            package_terminal_authority_permission_identities,
        ) = execution.terminal_authority_permissions();
        Ok(TerminalNativeProposalRequest {
            provider_declarations: execution.selected_provider_provenance.iter().map(|retained| {
                let typed = &execution.program.typed;
                let provider_declaration = retained.provider.provider_type.map(|symbol| {
                    typed.exact_owner_nominal_identity(symbol, self.sources().exact_toolchain_sources())
                        .and_then(|identity| typed.type_identity_text(identity))
                        .ok_or_else(|| vec![Diagnostic::error("native provider selection lost its exact declaration owner")])
                }).transpose()?;
                Ok(terminal_psi_to_abstract_operations::provider_admission::NativeProviderPlanDeclaration {
                    plan: &retained.plan,
                    provider_declaration,
                })
            }).collect::<Result<Vec<_>, Vec<Diagnostic>>>()?,
            family_realizations: self.selected_family_realizations()?,
            target_profile: execution.selected_target_profile,
            boundaries: BoundaryApplicationSettlement {
                selected_provider_plans: &execution.selected_provider_plan_facts,
                selected_provider_provenance: execution.selected_provider_provenance.iter().map(|retained| {
                    terminal_psi_to_abstract_operations::provider_admission::CompilerIntrinsicPlanProvenance {
                        plan: &retained.plan, row_executions: &retained.row_compiler_intrinsic_executions,
                    }
                }).collect(),
                calling_applications: providers.boundary_calling_plan_realizations.iter()
                    .map(|realization| realization.native_application()).collect(),
            },
            native_target: execution.selected_native_target(),
            program_entry: execution.selected_program_entry.as_ref().map(|entry| {
                terminal_psi_to_abstract_operations::NativeProgramEntrySettlement::new(
                    entry.source_signature(),
                    entry.calling_plans().map(|plans| (
                        &plans.semantic_calling_application,
                        &plans.physical_calling_application,
                        &plans.storage_entry,
                    )),
                    entry.fused_service_establishments(),
                )
            }),
            application: self.build_admission().native_application_metadata(
                &self.build().config.settings,
                execution.selected_target_profile,
            ),
            optimizations: selections.project_post_terminal(),
            external_bindings: &providers.external_binding_rows,
            behavior_exclusions: self.build().config.behavior_exclusions.as_slice(),
            package_terminal_authority_permissions,
            package_terminal_authority_permission_identities,
            callback_placements: callback_placements.iter().map(|placement| placement.native.clone()).collect(),
            callback_thunks,
            x86_scalar_fma_provider: execution.x86_scalar_fma_provider,
        })
    }
}

impl CheckedCompilation {
    /// Project the same exact source member that supplied each checked provider
    /// row. Artifact IDs are joined later from its structural family roster.
    fn selected_family_realizations(
        &self,
    ) -> Result<
        Vec<terminal_psi_to_abstract_operations::SelectedFamilyRealizationSource>,
        Vec<Diagnostic>,
    > {
        let execution = self.execution();
        let typed = &execution.program.typed;
        let mut rows = Vec::new();
        for retained in &execution.selected_provider_provenance {
            let provider = &retained.provider;
            let plan = &retained.plan;
            if provider.row_realizations.len() != plan.rows.len()
                || provider.row_requirements.len() != plan.rows.len()
                || provider.row_target_machine_origins.len() != plan.rows.len()
                || provider.row_placed_accessors.len() != plan.rows.len()
            {
                return Err(vec![Diagnostic::error(
                    "selected family realization has incomplete provider row custody",
                )]);
            }
            for (index, row) in plan.rows.iter().enumerate() {
                let realization = provider.row_target_machine_origins[index]
                    .as_ref()
                    .map_or(provider.row_realizations[index], |origin| origin.machine);
                for family in &typed.target_machine_families {
                    if !family.members.iter().any(|member| {
                        member.machine == realization && member.target.as_str() == plan.target
                    }) {
                        continue;
                    }
                    if provider.row_target_machine_origins[index]
                        .as_ref()
                        .is_some_and(|origin| origin.target != plan.target)
                    {
                        return Err(vec![Diagnostic::error(
                            "selected family realization has substituted target custody",
                        )]);
                    }
                    // Join the exact checked declaration retained by Psi. Export
                    // spellings and normalized requirement callable identities
                    // occupy different namespaces and cannot identify this head.
                    let head_declaration_identity = typed
                        .normalized_hermetic_symbol_identity(family.canonical)
                        .map_err(|message| {
                            vec![Diagnostic::error(format!(
                                "selected family realization head: {message}"
                            ))]
                        })?;
                    let head = typed.machine_by_symbol(family.canonical).ok_or_else(|| {
                        vec![Diagnostic::error(
                            "selected family realization has no typed head",
                        )]
                    })?;
                    let head_callable_identity =
                        typed.machine_callable_identity_text(head).ok_or_else(|| {
                            vec![Diagnostic::error(
                                "selected family realization has no head callable identity",
                            )]
                        })?;
                    rows.push(
                        terminal_psi_to_abstract_operations::SelectedFamilyRealizationSource {
                            head_declaration_identity,
                            head_callable_identity,
                            target: plan.target.clone(),
                            selected_requirement_identity: plan
                                .identities
                                .text(row.requirement_identity)
                                .to_owned(),
                            provider_plan_report_identity: plan.report_fingerprint(),
                            provider_plan_digest: plan.identity_digest(),
                        },
                    );
                }
            }
        }
        Ok(rows)
    }
}
