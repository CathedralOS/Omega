//! Join a chosen logical provider to its exact accepted bodies.
//! Source settlement and native admission use this same declaration query;
//! no calling convention or native instruction participates in the choice.

use service_surface::ServiceSurface;
use semantic_vocabulary::PackageKeyIdentity;
use terminal_psi::{TerminalIdentityIndex, TerminalModule};

pub struct CheckedProviderRow<'a> {
    pub requirement: &'a str,
    pub declared_requirement: &'a str,
    pub machine: &'a str,
    pub machine_package: Option<PackageKeyIdentity>,
}

pub struct CheckedProviderQuery<'a> {
    pub name: &'a str,
    pub declaration: &'a str,
    pub provider: &'a str,
    pub surface: &'a ServiceSurface,
    pub rows: &'a [CheckedProviderRow<'a>],
}

/// One position list per requested row, in the request's order. Every match
/// retains its full application, package and region coordinates.
pub fn selected_candidate_positions(
    query: CheckedProviderQuery<'_>,
    module: &TerminalModule,
) -> Result<Vec<Vec<usize>>, String> {
    let owner = (!query.declaration.is_empty())
        .then_some(query.declaration)
        .ok_or("checked provider plan has no exact selecting declaration")?;
    let mut selections = Vec::with_capacity(query.rows.len());
    for row in query.rows {
        let requirement = row.requirement;
        let matching = module
            .provider_candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| {
                module
                    .identities
                    .text(candidate.provider_declaration.identity())
                    == Some(owner)
                    && candidate.source_candidate_package_identity == row.machine_package
                    && module.identities.text(candidate.provider_identity) == Some(query.provider)
                    && module
                        .identities
                        .text(candidate.source_requirement_identity)
                        == Some(requirement)
                    && module.identities.text(candidate.source_candidate_identity)
                        == Some(row.machine)
                    && receiving_application_matches(
                        query.surface,
                        row.declared_requirement,
                        module,
                        candidate.declaration_application,
                    )
            })
            .map(|(position, _)| position)
            .collect::<Vec<_>>();
        let mut requirements = matching
            .iter()
            .map(|position| module.provider_candidates[*position].requirement_identity)
            .collect::<Vec<_>>();
        requirements.sort();
        if requirements.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(format!(
                "selected provider requirement `{requirement}` has duplicate canonical realizations: {:?}",
                matching
                    .iter()
                    .map(|position| &module.provider_candidates[*position])
                    .map(|candidate| (
                        candidate.candidate,
                        module.identities.text(candidate.candidate_identity),
                        module
                            .machine(candidate.candidate)
                            .and_then(|machine| machine.declaration.as_ref())
                            .and_then(|declaration| module
                                .identities
                                .text(declaration.semantic_declaration)),
                        module.identities.text(candidate.receiving_application),
                    ))
                    .collect::<Vec<_>>()
            ));
        }
        selections.push(matching);
    }
    for (row, positions) in query.rows.iter().zip(&selections) {
        let requirement = row.requirement;
        let method = query
            .surface
            .methods
            .iter()
            .find(|method| query.surface.identity_text(method.requirement_identity) == requirement)
            .ok_or("selected provider row lost its exact requirement declaration")?;
        for obligations in module
            .provider_instance_requirements
            .iter()
            .filter(|obligations| {
                obligations.package == method.requirement_owner_package_identity
                    && module.identities.text(obligations.template) == Some(requirement)
                    && obligations
                        .selecting_declaration
                        .as_ref()
                        .is_none_or(|declaration| {
                            module.identities.text(declaration.identity()) == Some(owner)
                                && module
                                    .identities
                                    .text(obligations.source_candidate_identity)
                                    == Some(row.machine)
                        })
            })
        {
            for required in &obligations.required_instances {
                if !receiving_application_matches(
                    query.surface,
                    row.declared_requirement,
                    module,
                    required.declaration_application,
                ) {
                    continue;
                }
                let count = positions
                    .iter()
                    .filter(|position| {
                        let candidate = &module.provider_candidates[**position];
                        candidate.requirement_identity == required.requirement_identity
                            && (required.candidate_identity.is_absent()
                                || candidate.candidate_identity == required.candidate_identity)
                            && candidate.declaration_application == required.declaration_application
                            && candidate.receiving_application == required.receiving_application
                            && candidate.receiving_region_arguments
                                == required.receiving_region_arguments
                    })
                    .count();
                if count != 1 {
                    return Err(format!(
                        "selected provider plan `{}` resolves retained closed requirement `{}` to {count} canonical provider instances; exactly one must serve it",
                        query.name,
                        module
                            .identities
                            .text(required.requirement_identity)
                            .unwrap_or("<unknown requirement>"),
                    ));
                }
            }
        }
    }
    Ok(selections)
}

fn receiving_application_matches(
    surface: &ServiceSurface,
    requirement: &str,
    module: &TerminalModule,
    receiving_application: TerminalIdentityIndex,
) -> bool {
    let receiving = if receiving_application == TerminalIdentityIndex::default() {
        None
    } else {
        let Some(receiving) = module.identities.text(receiving_application) else {
            return false;
        };
        Some(receiving)
    };
    surface.matches_receiving_application(requirement, receiving)
}
