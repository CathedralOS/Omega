// Substitute the complete selected installation closure without source queries.
// Inputs retain the selected requirement/plan association and its checked reach;
// a concrete child may complete before another child while its contribution
// stays in the parent's accumulated row. All publishers are
// counted up front so a pending competing plan never appears unambiguous, and
// the dependency index prefers the owning plan's row before allowing a foreign
// row only when the entire selected closure has a single publisher.

use diagnostics::Diagnostic;
use effects::InstallationReachResolution;
use terminal_psi::{TerminalIdentityTable, TerminalIdentityTableBuilder};

pub struct InstallationReachRow {
    pub requirement_identity: String,
    pub provider_plan_report_identity: u64,
    pub provider_name: String,
    pub realization_name: String,
    pub upper_bound: Vec<String>,
    pub concrete_reach: Vec<String>,
    pub dependencies: Vec<String>,
}

pub fn resolve_installation_reach(
    mut rows: Vec<InstallationReachRow>,
) -> Result<(Vec<InstallationReachResolution>, TerminalIdentityTable), Vec<Diagnostic>> {
    use flow_effects::installation_reach::{ReachResolutionError, ReachRow, resolve};
    // Name every publisher before resolving: a not-yet-ready competing plan
    // cannot make a foreign requirement look unambiguous.
    let dependencies = {
        let mut own = std::collections::BTreeMap::new();
        let mut publishers = std::collections::BTreeMap::new();
        for (position, row) in rows.iter().enumerate() {
            own.entry((
                row.requirement_identity.as_str(),
                row.provider_plan_report_identity,
            ))
            .or_insert(position);
            let publisher = publishers
                .entry(row.requirement_identity.as_str())
                .or_insert(Some((row.provider_plan_report_identity, position)));
            if publisher.is_some_and(|(plan, _)| plan != row.provider_plan_report_identity) {
                *publisher = None;
            }
        }
        rows.iter()
            .map(|row| {
                row.dependencies
                    .iter()
                    .map(|dependency| {
                        own.get(&(dependency.as_str(), row.provider_plan_report_identity))
                            .copied()
                            .or_else(|| {
                                publishers
                                    .get(dependency.as_str())
                                    .copied()
                                    .flatten()
                                    .map(|(_, position)| position)
                            })
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    let inputs = rows
        .iter_mut()
        .zip(dependencies)
        .map(|(row, dependencies)| ReachRow {
            fixed: std::mem::take(&mut row.concrete_reach),
            dependencies,
        })
        .collect();
    let resolved = resolve(inputs).map_err(|error| match error {
        ReachResolutionError::Unresolved(pending) => pending.into_iter().map(|(position, count)| {
            let row = &rows[position];
            Diagnostic::error(format!(
                "selected provider row `{}` realization `{}` of provider `{}` retains {} unresolved installation-bound requirement(s); its checked reach is a conservative bound, not a resolved row",
                row.requirement_identity, row.realization_name, row.provider_name, count,
            ))
        }).collect(),
        ReachResolutionError::InvalidDependency { row, dependency } => vec![Diagnostic::error(format!(
            "installation reach row {row} has invalid dependency {dependency}",
        ))],
    })?;
    let mut identities = TerminalIdentityTableBuilder::default();
    let resolutions = rows
        .into_iter()
        .zip(resolved)
        .map(|(mut row, resolved)| {
            row.concrete_reach = resolved;
            completed_row(row, &mut identities)
        })
        .collect();
    Ok((resolutions, identities.build()))
}

fn completed_row(
    row: InstallationReachRow,
    identities: &mut TerminalIdentityTableBuilder,
) -> InstallationReachResolution {
    InstallationReachResolution {
        requirement_identity: identities.intern(&row.requirement_identity),
        provider_plan_report_identity: row.provider_plan_report_identity,
        upper_bound: row.upper_bound,
        resolved_row: row.concrete_reach,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(
        requirement: &str,
        plan: u64,
        fixed: &[&str],
        dependencies: &[&str],
    ) -> InstallationReachRow {
        InstallationReachRow {
            requirement_identity: requirement.into(),
            provider_plan_report_identity: plan,
            provider_name: format!("provider-{plan}"),
            realization_name: "checked".into(),
            upper_bound: vec!["X".into(), "Y".into()],
            concrete_reach: fixed.iter().map(|name| (*name).into()).collect(),
            dependencies: dependencies.iter().map(|name| (*name).into()).collect(),
        }
    }

    #[test]
    fn the_owning_plan_wins_over_other_publishers() {
        let (resolved, _) = resolve_installation_reach(vec![
            row("A", 1, &[], &["B"]),
            row("B", 1, &["X"], &[]),
            row("B", 2, &["Y"], &[]),
        ])
        .unwrap();
        assert_eq!(resolved[0].resolved_row, ["X"]);
    }

    #[test]
    fn foreign_children_require_one_publisher_across_the_whole_closure() {
        let (resolved, _) =
            resolve_installation_reach(vec![row("A", 1, &[], &["B"]), row("B", 2, &["X"], &[])])
                .unwrap();
        assert_eq!(resolved[0].resolved_row, ["X"]);
        // The competing plan is still waiting: readiness cannot grant
        // authority to the first publisher that happened to finish.
        assert!(
            resolve_installation_reach(vec![
                row("A", 1, &[], &["B"]),
                row("B", 2, &["X"], &[]),
                row("B", 3, &[], &["missing"]),
            ])
            .is_err()
        );
    }
}
