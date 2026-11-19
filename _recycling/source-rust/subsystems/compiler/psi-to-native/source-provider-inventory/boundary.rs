use crate::CheckedBodyCandidate;
use declaration_contracts::boundary_provider_review::*;
use diagnostics::Diagnostic;
use provider_plans::{ProviderBinding, ProviderPlan, ProviderPlanRow};

pub struct BoundaryProviderAdmission<'facts> {
    pub demands: &'facts [ProviderPlan], // required schemas recorded by canonical Product demand discovery
    pub review: &'facts BoundaryProviderReview<'facts>,
    pub identities: &'facts terminal_psi::TerminalIdentityTable,
    pub external_providers: &'facts [terminal_psi::ExternalProviderConformance],
}

impl BoundaryProviderAdmission<'_> {
    pub fn admit(
        &self,
        selected: &selected_provider_plans::SelectedProviderPlanFacts,
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
        usage: &declaration_contracts::boundary_provider_review::BoundaryRequirementUse<'_>,
        candidates: &[ProviderPlan],
        selected: &selected_provider_plans::SelectedProviderPlanFacts,
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
        selected: &selected_provider_plans::SelectedProviderPlanFacts,
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
    selected: &'plans selected_provider_plans::SelectedProviderPlanFacts,
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
