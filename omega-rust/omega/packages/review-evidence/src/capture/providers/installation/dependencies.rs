//! Ground selected installation rows in checked dependencies, not mutual claims.

use crate::capture::{PackageReviewInput, behavior::project_installation_reaches};
use diagnostics::Diagnostic;

pub(in crate::capture::providers) fn validate_installation_dependency_order(
    compilation: &PackageReviewInput<'_>,
    selected: &effects::SelectedProviderPlanFacts,
) -> Result<(), Vec<Diagnostic>> {
    let execution = compilation.custody.execution();
    let mut rows = Vec::new();
    for resolution in selected.installation_reach_resolutions() {
        let requirement = selected
            .identities()
            .text(resolution.requirement_identity)
            .ok_or_else(|| rejected("installation dependency lost its requirement identity"))?;
        let mut owners = execution
            .selected_provider_provenance
            .iter()
            .filter(|owner| {
                owner.plan.report_fingerprint() == resolution.provider_plan_report_identity
            });
        let owner = owners
            .next()
            .ok_or_else(|| rejected("installation dependency lost its selected plan"))?;
        if owners.next().is_some() {
            return Err(rejected(
                "installation dependency has competing selected plans",
            ));
        }
        let position = owner
            .plan
            .rows
            .iter()
            .position(|row| owner.plan.identity_text(row.requirement_identity) == requirement)
            .ok_or_else(|| rejected("installation dependency lost its selected row"))?;
        let realization = owner
            .provider
            .row_realizations
            .get(position)
            .and_then(|origin| origin.authored())
            .ok_or_else(|| rejected("installation dependency has no checked realization"))?;
        let fact = compilation
            .facts
            .service_reaches
            .for_machine(realization)
            .ok_or_else(|| rejected("installation dependency has no checked reach facts"))?;
        let dependencies =
            project_installation_reaches(compilation, &fact.unresolved_installation_reaches)?
                .into_iter()
                .map(|dependency| {
                    let child = selected
                        .resolve_installation_requirement(
                            dependency.requirement().path(),
                            resolution.provider_plan_report_identity,
                        )
                        .map_err(|reason| rejected(&reason))?;
                    selected
                        .installation_reach_resolutions()
                        .iter()
                        .position(|row| {
                            row.provider_plan_report_identity == child.provider_plan_report_identity
                                && row.requirement_identity == child.requirement_identity
                        })
                        .ok_or_else(|| {
                            rejected("installation dependency lost its resolved coordinate")
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
        rows.push(dependencies);
    }
    validate_grounded_rows(&rows).map_err(rejected)
}

fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(reason)]
}

fn validate_grounded_rows(rows: &[Vec<usize>]) -> Result<(), &'static str> {
    // Explicit work stack avoids consuming the host stack on a long installed
    // chain. Entries retain their next child; shared completed nodes are reused.
    let mut visiting = vec![false; rows.len()];
    let mut complete = vec![false; rows.len()];
    for root in 0..rows.len() {
        if complete[root] {
            continue;
        }
        let mut stack = vec![(root, 0)];
        visiting[root] = true;
        while let Some((current, next)) = stack.last_mut() {
            let Some(&child) = rows[*current].get(*next) else {
                visiting[*current] = false;
                complete[*current] = true;
                stack.pop();
                continue;
            };
            *next += 1;
            if child >= rows.len() {
                return Err("installation dependency has no selected resolution");
            }
            if visiting[child] {
                return Err(
                    "installation reach has a cyclic selected dependency without a grounded resolution",
                );
            }
            if !complete[child] {
                visiting[child] = true;
                stack.push((child, 0));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grounded_dependencies_reject_circular_justification() {
        assert!(validate_grounded_rows(&[vec![1, 2], vec![3], vec![3], vec![]]).is_ok());
        assert!(
            validate_grounded_rows(&[vec![1], vec![0]])
                .unwrap_err()
                .contains("cyclic")
        );
        assert!(
            validate_grounded_rows(&[vec![0]])
                .unwrap_err()
                .contains("cyclic")
        );
        assert!(validate_grounded_rows(&[vec![1]]).is_err());
    }
}
