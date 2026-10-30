//! Admit exact executable boundary applications from Terminal's retained catalog.
//! Source declaration eligibility is a separate scope: unused generic declarations
//! need no portable executable body. This operation selects only actual demands;
//! native realization owns their physical operation coverage.

use diagnostics::Diagnostic;
use effects::CompilerIntrinsicExecutionIdentity;
use effects::provider_plan::{ProviderPlan, ServiceSchema};

/// The source-free part of one retained provider review. The complete plan is
/// compared before its row-aligned execution identities can supply a proposal.
pub struct CompilerIntrinsicPlanProvenance<'a> {
    pub plan: &'a ProviderPlan,
    /// The contract classification authenticated before projecting a direct
    /// callable's selection coordinate. Ordinary plans use their own schema.
    pub semantic_schema: &'a ServiceSchema,
    pub row_executions: &'a [Option<CompilerIntrinsicExecutionIdentity>],
    pub row_numeric_realizations: &'a [Option<String>],
}

pub fn select_boundary_applications(
    provenance: &[CompilerIntrinsicPlanProvenance<'_>],
    selected: &effects::SelectedProviderPlanFacts,
    module: &terminal_psi::TerminalModule,
    demands: &boundary_applications::TerminalBoundaryApplicationDemands,
) -> Result<boundary_applications::TerminalBoundaryApplicationRealizations, Vec<Diagnostic>> {
    use boundary_applications::BoundaryApplicationRealization as Realization;
    use effects::provider_plan::ProviderBinding;
    let error = |message| vec![Diagnostic::error(message)];
    let plans = selected.plans();
    if plans.len() != provenance.len() {
        return Err(error(
            "boundary applications have misaligned selected-plan provenance",
        ));
    }
    // Keep row identity and its already-checked execution together. Selection
    // never revisits checked expressions or rederives an intrinsic's meaning.
    let mut selected_rows = Vec::new();
    for (plan, retained) in plans.iter().zip(provenance) {
        if plan != retained.plan
            || plan.rows.len() != retained.row_executions.len()
            || plan.rows.len() != retained.row_numeric_realizations.len()
        {
            return Err(error(
                "boundary applications have substituted or incomplete selected rows",
            ));
        }
        for ((row, execution), realization) in plan
            .rows
            .iter()
            .zip(retained.row_executions)
            .zip(retained.row_numeric_realizations)
        {
            selected_rows.push((plan, row, *execution, realization.as_deref()));
        }
    }
    let mut rows = Vec::with_capacity(demands.rows().len());
    for demand in demands.rows() {
        let requirement = demand.requirement();
        let mut matching = selected_rows.iter().filter(|(plan, row, _, _)| {
            plan.schema.identity_text(plan.schema.trait_name) == requirement.overload()
                && plan.schema.trait_package_identity == requirement.package()
                && plan.identities.text(row.requirement_identity) == requirement.overload()
        });
        let (Some((plan, row, execution, external_realization)), None) =
            (matching.next(), matching.next())
        else {
            return Err(error(
                "Terminal boundary application has no unique exact selected row",
            ));
        };
        let realization = match &row.binding {
            ProviderBinding::CompilerIntrinsic { .. } => {
                let execution = crate::selected_numeric_execution(
                    &module.numeric_realizations,
                    &module.identities,
                    requirement.declaration(),
                    plan,
                    row,
                    *execution,
                    *external_realization,
                )
                .map_err(|message| vec![Diagnostic::error(message)])?;
                Realization::ExactCompilerIntrinsic {
                    execution: execution.ok_or_else(|| {
                        error("selected intrinsic has no canonical or admitted execution identity")
                    })?,
                }
            }
            ProviderBinding::CheckedAdapter { .. } => {
                let body =
                    crate::checked_bodies::select_terminal_checked_body(module, demand, plan, row)
                        .map_err(|diagnostic| vec![diagnostic])?;
                match &body.specialization {
                    Some(specialization) => Realization::SpecializedCheckedBody {
                        realization_template: specialization.template.clone(),
                        realization_machine: body.machine.clone(),
                        realization_state: body.state.clone(),
                        specialization_commitment: specialization.commitment,
                        realization_contract_commitment: body.contract,
                    },
                    None => Realization::NongenericCheckedBody {
                        realization_machine: body.machine.clone(),
                        realization_state: body.state.clone(),
                        realization_contract_commitment: body.contract,
                    },
                }
            }
            _ => {
                return Err(error(
                    "Terminal boundary application has no checked-body or compiler-intrinsic realization",
                ));
            }
        };
        rows.push(
            boundary_applications::BoundaryApplicationRealizationCompanion::new(
                demand.terminal_operation(),
                *plan.identity_digest().as_bytes(),
                realization,
            )
            .map_err(error)?,
        );
    }
    boundary_applications::TerminalBoundaryApplicationRealizations::new(demands, rows)
        .map_err(error)
}
