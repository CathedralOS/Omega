/*
Boundary application selection and selected declaration-policy admission.

select_boundary_applications admits exact executable boundary applications from
Terminal's retained catalog. Source declaration eligibility is a separate scope
because unused generic declarations need no portable executable body; we select
only actual demands, keeping row identity and its already-checked execution
together without revisiting checked expressions or rederiving an intrinsic's
meaning, while native realization owns physical operation coverage.

*/

use diagnostics::Diagnostic;
use native_intrinsics::CompilerIntrinsicExecutionIdentity;
use provider_plans::{ProviderBinding, ProviderPlan, ServiceSchema};

pub struct CompilerIntrinsicPlanProvenance<'a> {
    pub plan: &'a ProviderPlan,
    pub semantic_schema: &'a ServiceSchema, // contract classification authenticated before projecting a direct callable's selection coordinate
    pub row_executions: &'a [Option<CompilerIntrinsicExecutionIdentity>],
}

pub fn select_boundary_applications(
    provenance: &[CompilerIntrinsicPlanProvenance<'_>],
    selected: &selected_provider_plans::SelectedProviderPlanFacts,
    module: &terminal_psi::TerminalModule,
    demands: &terminal_psi::TerminalBoundaryApplicationDemands,
) -> Result<boundary_applications::TerminalBoundaryApplicationRealizations, Vec<Diagnostic>> {
    use boundary_applications::BoundaryApplicationRealization as Realization;
    let error = |message| vec![Diagnostic::error(message)];
    let plans = selected.plans();
    if plans.len() != provenance.len() {
        return Err(error(
            "boundary applications have misaligned selected-plan provenance",
        ));
    }
    let mut selected_rows = Vec::new();
    for (plan, retained) in plans.iter().zip(provenance) {
        if plan != retained.plan
            || plan.rows.len() != retained.row_executions.len()
            || plan.rows.len() != plan.row_numeric_realizations.len()
        {
            return Err(error(
                "boundary applications have substituted or incomplete selected rows",
            ));
        }
        for ((row, execution), realization) in plan
            .rows
            .iter()
            .zip(retained.row_executions)
            .zip(&plan.row_numeric_realizations)
        {
            selected_rows.push((plan, row, *execution, realization.as_deref()));
        }
    }
    let mut rows = Vec::with_capacity(demands.rows().len());
    for demand in demands.rows() {
        let requirement = demand.requirement();
        let mut matching = selected_rows.iter().filter(|(plan, row, _, _)| {
            plan.schema
                .identity_text(plan.schema.declaration.trait_name)
                == requirement.overload()
                && plan.schema.declaration.trait_package_identity == requirement.package()
                && plan.identities.text(row.requirement_identity) == requirement.overload()
        });
        let first = matching.next();
        let second = matching.next();
        let (Some((plan, row, execution, external_realization)), None) = (first, second) else {
            return Err(error(&format!(
                "Terminal boundary application has no unique exact selected row: requirement `{}` in package {:?} at operation {:?}, {} selected matches; selected coordinates: {:?}",
                requirement.overload(),
                requirement.package(),
                demand.terminal_operation(),
                if first.is_none() { "no" } else { "multiple" },
                selected_rows
                    .iter()
                    .map(|(plan, row, _, _)| (
                        plan.schema
                            .identity_text(plan.schema.declaration.trait_name),
                        plan.schema.declaration.trait_package_identity,
                        plan.identities.text(row.requirement_identity),
                    ))
                    .collect::<Vec<_>>(),
            )));
        };
        let realization = match &row.binding {
            ProviderBinding::CompilerIntrinsic { .. } => {
                let execution = crate::selected_external_execution(
                    &module.interfaces.external_providers,
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
                let body = crate::bodies::select_terminal_checked_body(module, demand, plan, row)
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
