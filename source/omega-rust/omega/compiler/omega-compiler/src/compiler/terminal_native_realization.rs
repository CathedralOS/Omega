//! Native re-entry for a retained Terminal product with source-evaluated imports.
//!
//! This coordinator owns the exact-plan join. Callers supply only independently
//! admitted execution and same-stack evidence; they cannot select a provider
//! plan, locator, target, compiler builtin, or checked-scope receipt.

use omega_installation_evidence::ProviderExecutionEvidence;
use omega_task_plans::AdmittedSameStackContribution;
use psi_diagnostics::Diagnostic;
use std::collections::{BTreeMap, BTreeSet};

/// Externally admitted custody for one demanded source-evaluated import.
///
/// Construction does not admit either input. Native realization independently
/// rejoins both values to the exact retained provider plan and import row.
#[derive(Debug, Clone, Copy)]
pub struct SourceEvaluatedImportSettlement<'evidence> {
    provider_execution: &'evidence dyn ProviderExecutionEvidence,
    same_stack: &'evidence AdmittedSameStackContribution,
}

impl<'evidence> SourceEvaluatedImportSettlement<'evidence> {
    pub const fn new(
        provider_execution: &'evidence dyn ProviderExecutionEvidence,
        same_stack: &'evidence AdmittedSameStackContribution,
    ) -> Self {
        Self {
            provider_execution,
            same_stack,
        }
    }

    pub const fn provider_execution(self) -> &'evidence dyn ProviderExecutionEvidence {
        self.provider_execution
    }

    pub const fn same_stack(self) -> &'evidence AdmittedSameStackContribution {
        self.same_stack
    }
}

/// Consume one retained Terminal product and realize its demanded evaluated
/// imports through externally admitted execution and same-stack custody.
///
/// The first lane deliberately rejects callback custody. Callback placement
/// needs a combined checked-scope realization owner and cannot be inferred from
/// an opaque retained sidecar.
pub fn realize_retained_terminal_artifact_with_source_evaluated_imports(
    retained: omega_compilation_report::RetainedTerminalArtifact,
    profile: &psi_proof_admission::AdmissionProfile,
    optimization_selections: &omega_optimization_core::OptimizationSelections,
    imports: &[SourceEvaluatedImportSettlement<'_>],
) -> Result<omega_compilation_report::RetainedNativeArtifact, Vec<Diagnostic>> {
    retained
        .validate()
        .map_err(|message| diagnostic("retained Terminal product", message))?;
    if !retained.callback_placements().is_empty() {
        return Err(diagnostic(
            "retained Terminal product",
            "source-evaluated import realization does not yet consume callback custody",
        ));
    }
    let (artifact, callback_placements, proposal) = retained.into_parts();
    debug_assert!(callback_placements.is_empty());
    let proposal = proposal.ok_or_else(|| {
        diagnostic(
            "retained Terminal product",
            "source-evaluated import realization requires one native proposal",
        )
    })?;
    proposal
        .validate_for_artifact(&artifact)
        .map_err(|message| diagnostic("Terminal native proposal", message))?;
    if !proposal.callback_occurrences().is_empty() {
        return Err(diagnostic(
            "Terminal native proposal",
            "source-evaluated import realization does not yet consume callback occurrences",
        ));
    }

    let module = psi_terminal_codec::decode_module(artifact.semantic_bytes()).map_err(|error| {
        diagnostic(
            "Terminal native proposal",
            format!("canonical semantics could not be decoded: {error}"),
        )
    })?;
    let demanded = super::intrinsic_settlements::demanded_boundary_identities(&module)?;
    let exact_import_plans = exact_demanded_import_plans(&proposal, &demanded)?;
    let native_settlements = rejoin_external_import_settlements(&exact_import_plans, imports)?;

    let selected_plans = proposal.selected_provider_plans().plans();
    let compiler_builtins = proposal
        .compiler_builtins()
        .iter()
        .map(|builtin| {
            let provider_plan = selected_plans
                .get(builtin.provider_plan_index())
                .ok_or_else(|| {
                    diagnostic(
                        "Terminal native proposal",
                        format!(
                            "compiler builtin `{}` names an absent selected provider plan",
                            builtin.requirement_identity()
                        ),
                    )
                })?;
            Ok(
                omega_terminal_psi_to_native_artifact::NativeCompilerBuiltinSettlement {
                    requirement_identity: builtin.requirement_identity(),
                    provider_plan,
                    execution: builtin.execution(),
                },
            )
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let ieee_float_fma = proposal
        .ieee_float_fma_occurrences()
        .iter()
        .map(|occurrence| {
            let provider_plan = proposal
                .selected_provider_plans()
                .plans()
                .get(occurrence.provider_plan_index())
                .ok_or_else(|| {
                    diagnostic(
                        "Terminal nearest-FMA proposal",
                        "occurrence names an absent exact selected provider plan",
                    )
                })?;
            let admission = occurrence.x86_admission().ok_or_else(|| {
                diagnostic(
                    "Terminal nearest-FMA proposal",
                    "ordinary native lowering currently requires admitted x86 FMA custody",
                )
            })?;
            Ok(
                omega_terminal_psi_to_native_artifact::AdmittedIeeeFloatFmaSettlement {
                    terminal_operation: occurrence.terminal_operation(),
                    provider_plan,
                    format: occurrence.format(),
                    slot: admission.slot(),
                    provider: admission.provider(),
                },
            )
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let calling_plans = proposal
        .program_entry()
        .calling_plans()
        .map(|plans| (&plans.semantic_boundary_entry_plan, &plans.storage_entry));
    let program_entry = omega_terminal_psi_to_native_artifact::NativeProgramEntrySettlement::new(
        proposal.program_entry().source_signature(),
        calling_plans,
    );
    omega_terminal_psi_to_native_artifact::realize_native_artifact_with_checked_boundary_operator_scope(
        artifact,
        proposal.checked_boundary_operator_scope(),
        omega_terminal_psi_to_native_artifact::NativeRealizationRequest {
            target: proposal.native_target(),
            subsystem: proposal.subsystem(),
            profile,
            terminal_authority_policy:
                omega_terminal_psi_to_native_artifact::current_compiler_intrinsic_terminal_authority_policy(),
            program_entry,
            optimization_selections,
            selected_provider_plans: proposal.selected_provider_plans(),
            external_binding_rows: proposal.external_binding_rows(),
            settlements: &native_settlements,
            compiler_builtins: &compiler_builtins,
            ieee_float_fma: &ieee_float_fma,
        },
    )
}

fn exact_demanded_import_plans<'proposal>(
    proposal: &'proposal omega_compilation_report::TerminalNativeRealizationProposal,
    demanded: &BTreeSet<String>,
) -> Result<BTreeMap<String, &'proposal omega_effects::provider_plan::ProviderPlan>, Vec<Diagnostic>>
{
    let mut exact = BTreeMap::new();
    for requirement in demanded {
        let matches = proposal
            .selected_provider_plans()
            .plans()
            .iter()
            .flat_map(|plan| {
                plan.rows
                    .iter()
                    .filter(move |row| {
                        row.requirement_identity == *requirement
                            && matches!(
                                row.binding,
                                omega_effects::provider_plan::ProviderBinding::Import { .. }
                            )
                    })
                    .map(move |_| plan)
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => {}
            [plan] => {
                exact.insert(requirement.clone(), *plan);
            }
            _ => {
                return Err(diagnostic(
                    "Terminal native proposal",
                    format!(
                        "demanded source-evaluated import `{requirement}` resolves to {} selected import plans",
                        matches.len()
                    ),
                ));
            }
        }
    }
    Ok(exact)
}

fn rejoin_external_import_settlements<'proposal, 'evidence>(
    exact_plans: &BTreeMap<String, &'proposal omega_effects::provider_plan::ProviderPlan>,
    imports: &[SourceEvaluatedImportSettlement<'evidence>],
) -> Result<
    Vec<omega_terminal_psi_to_native_artifact::NativeProviderSettlement<'proposal>>,
    Vec<Diagnostic>,
>
where
    'evidence: 'proposal,
{
    let mut seen = BTreeSet::new();
    let mut settlements = Vec::with_capacity(imports.len());
    for import in imports {
        let requirement = import.provider_execution.requirement_identity();
        if !seen.insert(requirement) {
            return Err(diagnostic(
                "source-evaluated import settlement",
                format!("requirement `{requirement}` was supplied more than once"),
            ));
        }
        let provider_plan = exact_plans.get(requirement).copied().ok_or_else(|| {
            diagnostic(
                "source-evaluated import settlement",
                format!("requirement `{requirement}` is not a demanded selected import"),
            )
        })?;
        if provider_plan.report_fingerprint()
            != import.provider_execution.provider_plan_report_identity()
        {
            return Err(diagnostic(
                "source-evaluated import settlement",
                format!(
                    "execution evidence for `{requirement}` names a different provider-plan report coordinate"
                ),
            ));
        }
        settlements.push(
            omega_terminal_psi_to_native_artifact::NativeProviderSettlement {
                provider_execution: import.provider_execution,
                provider_plan,
                realization:
                    omega_terminal_psi_to_native_artifact::NativeBoundaryRealization::NormalizedForeignCall(
                        import.same_stack,
                    ),
            },
        );
    }
    if let Some(missing) = exact_plans
        .keys()
        .find(|requirement| !seen.contains(requirement.as_str()))
    {
        return Err(diagnostic(
            "source-evaluated import settlement",
            format!("demanded import `{missing}` has no supplied execution and stack custody"),
        ));
    }
    Ok(settlements)
}

fn diagnostic(context: &str, message: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!("{context}: {message}"))]
}
