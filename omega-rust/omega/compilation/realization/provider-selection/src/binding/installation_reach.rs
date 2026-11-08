// Substitute the complete selected installation closure without source queries.
// Inputs retain the selected requirement/plan association and its checked reach;
// a concrete child may become available several rounds before another child while
// its contribution stays in the parent's accumulated row. All publishers are
// counted up front so a pending competing plan never appears unambiguous, and
// plan_scoped_resolution prefers the owning plan's row before allowing a foreign
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
    rows: Vec<InstallationReachRow>,
) -> Result<(Vec<InstallationReachResolution>, TerminalIdentityTable), Vec<Diagnostic>> {
    let mut identities = TerminalIdentityTableBuilder::default();
    let mut resolutions = Vec::new();
    let mut pending = Vec::new();
    let mut publishers =
        std::collections::BTreeMap::<String, std::collections::BTreeSet<u64>>::new();
    for row in rows {
        publishers
            .entry(row.requirement_identity.clone())
            .or_default()
            .insert(row.provider_plan_report_identity);
        if row.dependencies.is_empty() {
            resolutions.push(completed_row(row, &mut identities));
        } else {
            pending.push(row);
        }
    }
    loop {
        let mut next = Vec::new();
        let mut progressed = false;
        for mut row in pending {
            row.dependencies.retain(|dependency| {
                let child = plan_scoped_resolution(
                    &resolutions,
                    &identities,
                    &publishers,
                    row.provider_plan_report_identity,
                    dependency,
                );
                match child {
                    Some(child) => {
                        row.concrete_reach
                            .extend(child.resolved_row.iter().cloned());
                        false
                    }
                    None => true,
                }
            });
            if row.dependencies.is_empty() {
                row.concrete_reach.sort();
                row.concrete_reach.dedup();
                resolutions.push(completed_row(row, &mut identities));
                progressed = true;
            } else {
                next.push(row);
            }
        }
        pending = next;
        if !progressed {
            break;
        }
    }
    if pending.is_empty() {
        Ok((resolutions, identities.build()))
    } else {
        Err(pending.into_iter().map(|row| Diagnostic::error(format!(
            "selected provider row `{}` realization `{}` of provider `{}` retains {} unresolved installation-bound requirement(s); its checked reach is a conservative bound, not a resolved row",
            row.requirement_identity, row.realization_name, row.provider_name, row.dependencies.len(),
        ))).collect())
    }
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

fn plan_scoped_resolution<'a>(
    resolutions: &'a [InstallationReachResolution],
    identities: &TerminalIdentityTableBuilder,
    publishers: &std::collections::BTreeMap<String, std::collections::BTreeSet<u64>>,
    plan: u64,
    requirement: &str,
) -> Option<&'a InstallationReachResolution> {
    if let Some(own) = resolutions.iter().find(|row| {
        identities.text(row.requirement_identity) == Some(requirement)
            && row.provider_plan_report_identity == plan
    }) {
        return Some(own);
    }
    if publishers
        .get(requirement)
        .is_some_and(|owners| owners.len() > 1)
    {
        return None;
    }
    resolutions
        .iter()
        .find(|row| identities.text(row.requirement_identity) == Some(requirement))
}
