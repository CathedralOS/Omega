/*
Boundary application selection and selected declaration-policy admission.

select_boundary_applications admits exact executable boundary applications from
Terminal's retained catalog. Source declaration eligibility is a separate scope
because unused generic declarations need no portable executable body; we select
only actual demands, keeping row identity and its already-checked execution
together without revisiting checked expressions or rederiving an intrinsic's
meaning, while native realization owns physical operation coverage.

BoundaryProviderAdmission admits selected declaration policy under canonical
Product demand. Psi supplies ordered, source-free eligibility and application
evidence; we own selected-plan cardinality, binding policy, and evidence use in
three strict diagnostic phases (dormant declarations, spelled/named occurrences,
then direct calls) where each phase completes before the next begins.
*/

use crate::CheckedBodyCandidate;
use diagnostics::Diagnostic;
use effects::CompilerIntrinsicExecutionIdentity;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};
use language_semantics::boundary_provider_review::{
    BoundaryOperatorApplication, BoundaryProviderReview, BoundaryRequirementReview,
};

pub struct CompilerIntrinsicPlanProvenance<'a> {
    pub plan: &'a ProviderPlan,
    pub semantic_schema: &'a ServiceSchema, // contract classification authenticated before projecting a direct callable's selection coordinate
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
                    &module.external_providers,
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

pub struct BoundaryProviderAdmission<'facts> {
    pub demands: &'facts [ProviderPlan], // required schemas recorded by canonical Product demand discovery
    pub review: &'facts BoundaryProviderReview<'facts>,
    pub identities: &'facts terminal_psi::TerminalIdentityTable,
    pub external_providers: &'facts [terminal_psi::ExternalProviderConformance],
}

impl BoundaryProviderAdmission<'_> {
    pub fn admit(
        &self,
        selected: &effects::SelectedProviderPlanFacts,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        for plan in selected.plans() {
            let Some(declaration) = self
                .review
                .declarations
                .iter()
                .find(|declaration| requirement_binds(declaration.requirement, plan))
            else {
                continue;
            };
            if let Err(error) = self.admit_operator(
                declaration.requirement,
                OperatorEvidence::Declaration(&declaration.bodies),
                selected.plans(),
                selected,
            ) {
                diagnostics.push(error);
            }
        }
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        for usage in &self.review.operator_uses {
            if let Err(error) = self.admit_operator(
                usage.requirement,
                OperatorEvidence::Application(&usage.application),
                self.demands,
                selected,
            ) {
                diagnostics.push(error);
            }
        }
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        for usage in &self.review.requirement_uses {
            if let Err(error) = self.admit_requirement(usage, self.demands, selected) {
                diagnostics.push(error);
            }
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    fn admit_requirement(
        &self,
        usage: &language_semantics::boundary_provider_review::BoundaryRequirementUse<'_>,
        candidates: &[ProviderPlan],
        selected: &effects::SelectedProviderPlanFacts,
    ) -> Result<(), Diagnostic> {
        let Some((plan, row)) = selected_plan(
            usage.requirement,
            candidates,
            selected,
            BoundaryUse::DirectCall(usage.diagnostic_name),
        )?
        else {
            return Ok(());
        };
        match &row.binding {
            ProviderBinding::CheckedAdapter { .. }
            | ProviderBinding::Import { .. }
            | ProviderBinding::Syscall { .. } => Ok(()),
            ProviderBinding::CompilerIntrinsic { .. } => {
                self.require_numeric(usage.requirement, plan, row)
            }
            binding => Err(Diagnostic::error(format!(
                "selected boundary-requirement ProviderPlan `{}` uses unsupported binding `{binding:?}` for a direct call; a directly called requirement takes a checked adapter, compiler intrinsic, or evaluated import/syscall binding",
                plan.name,
            ))),
        }
    }

    fn admit_operator(
        &self,
        requirement: BoundaryRequirementReview<'_>,
        evidence: OperatorEvidence<'_>,
        candidates: &[ProviderPlan],
        selected: &effects::SelectedProviderPlanFacts,
    ) -> Result<(), Diagnostic> {
        let Some((plan, row)) =
            selected_plan(requirement, candidates, selected, BoundaryUse::Operator)?
        else {
            return Ok(());
        };
        match &row.binding {
            ProviderBinding::CheckedAdapter { .. } => {
                let bodies = match evidence {
                    OperatorEvidence::Declaration(bodies) => bodies,
                    OperatorEvidence::Application(BoundaryOperatorApplication::GenericTemplate) => {
                        return Ok(());
                    }
                    OperatorEvidence::Application(BoundaryOperatorApplication::Closed(bodies)) => {
                        bodies
                            .as_deref()
                            .map_err(|error| Diagnostic::error(error.to_string()))?
                    }
                };
                crate::select_checked_body(plan, bodies.iter().copied())
            }
            ProviderBinding::CompilerIntrinsic { .. } => {
                self.require_numeric(requirement, plan, row)
            }
            binding => Err(Diagnostic::error(format!(
                "selected boundary-operator ProviderPlan `{}` uses unsupported binding `{binding:?}`; boundary operators require a checked adapter or compiler intrinsic",
                plan.name,
            ))),
        }
    }

    fn require_numeric(
        &self,
        requirement: BoundaryRequirementReview<'_>,
        plan: &ProviderPlan,
        row: &ProviderPlanRow,
    ) -> Result<(), Diagnostic> {
        let join = || -> Result<(), String> {
            let nominal = requirement.numeric_requirement?;
            crate::bodies::selected_execution_for_identity(
                self.external_providers,
                self.identities,
                nominal,
                plan,
                row,
                None,
                None,
            )?
            .ok_or("numeric selection has no exact checked external satisfaction")?;
            Ok(())
        };
        join().map_err(|reason| {
            Diagnostic::error(format!("selected ProviderPlan `{}`: {reason}", plan.name))
        })
    }
}

enum OperatorEvidence<'facts> {
    Declaration(&'facts [CheckedBodyCandidate<'facts, ()>]),
    Application(&'facts BoundaryOperatorApplication<'facts>),
}

enum BoundaryUse<'facts> {
    Operator,
    DirectCall(&'facts str),
}

fn selected_plan<'plans>(
    requirement: BoundaryRequirementReview<'_>,
    candidates: &[ProviderPlan],
    selected: &'plans effects::SelectedProviderPlanFacts,
    usage: BoundaryUse<'_>,
) -> Result<Option<(&'plans ProviderPlan, &'plans ProviderPlanRow)>, Diagnostic> {
    if !candidates
        .iter()
        .any(|plan| requirement_binds(requirement, plan))
    {
        return Ok(None);
    }
    let (subject, category, name) = match usage {
        BoundaryUse::Operator => ("operator", "boundary-operator", requirement.schema_identity),
        BoundaryUse::DirectCall(name) => ("requirement", "boundary-requirement", name),
    };
    let mut matching = selected
        .plans()
        .iter()
        .filter(|plan| requirement_binds(requirement, plan));
    let first = matching.next();
    let count = usize::from(first.is_some()) + matching.count();
    let plan = match (first, count) {
        (Some(plan), 1) => plan,
        _ => {
            // An unchosen canonical float operation is one the target's
            // realization table declares unsupported; report that constraint.
            let unsupported = (count == 0)
                .then(|| {
                    candidates
                        .iter()
                        .filter(|plan| requirement_binds(requirement, plan))
                        .find_map(|plan| {
                            target::TargetProfile::from_canonical_target_name(&plan.target).ok()
                        })
                })
                .flatten()
                .and_then(|profile| match profile.canonical_float_realization(name)? {
                    target::CanonicalFloatRealization::Unsupported { constraint } => {
                        Some((profile, constraint))
                    }
                    target::CanonicalFloatRealization::CatalogIntrinsic { .. } => None,
                });
            if let Some((profile, constraint)) = unsupported {
                return Err(Diagnostic::error(format!(
                    "canonical float operation `{name}` has no realization on target `{}`: {constraint}",
                    profile.target_name(),
                )));
            }
            return Err(Diagnostic::error(format!(
                "boundary {subject} `{name}` in package {:?} resolves to {count} selected ProviderPlans; expected exactly one",
                requirement.package_identity,
            )));
        }
    };
    let [row] = plan.rows.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected {category} ProviderPlan `{}` must contain exactly one realization row",
            plan.name
        )));
    };
    Ok(Some((plan, row)))
}

fn requirement_binds(requirement: BoundaryRequirementReview<'_>, plan: &ProviderPlan) -> bool {
    plan.schema
        .identity_text(plan.schema.declaration.trait_name)
        == requirement.schema_identity
        && plan.schema.declaration.trait_package_identity == requirement.package_identity
}
