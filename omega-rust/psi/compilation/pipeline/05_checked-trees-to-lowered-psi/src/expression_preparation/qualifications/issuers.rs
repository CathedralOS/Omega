//! Checked domain issuer declarations shared by scalar and structural publication.

use super::{
    CheckedTrees, DomainEstablishmentRoute, LoweringError, ScalarDomainEstablishmentRoute,
};
use crate::machine_lowering::declarations::semantic_identity;
use crate::proofs::evidence_lowering::{
    checked_evidence_machine_identity, checked_evidence_requirement_identity,
};

pub(crate) fn establishment_route(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    route: &DomainEstablishmentRoute,
) -> Result<ScalarDomainEstablishmentRoute, LoweringError> {
    let declaration = terminal_psi::NominalDeclarationIdentity::new(
        semantic_identity(identities, checked, route.established_declaration()).map_err(|_| {
            LoweringError::Unsupported("domain issuer has no exact declaration owner")
        })?,
    );
    Ok(match *route {
        DomainEstablishmentRoute::CheckedRequirement {
            trait_definition,
            requirement,
        } => ScalarDomainEstablishmentRoute::CheckedRequirement {
            declaration,
            requirement_identity: identities.intern(&checked_evidence_requirement_identity(
                checked,
                trait_definition,
                requirement,
            )?),
        },
        DomainEstablishmentRoute::BoundaryRequirement {
            boundary_trait,
            requirement,
        } => ScalarDomainEstablishmentRoute::BoundaryRequirement {
            declaration,
            requirement_identity: identities.intern(&checked_evidence_requirement_identity(
                checked,
                boundary_trait,
                requirement,
            )?),
        },
        DomainEstablishmentRoute::ExactMachine { machine } => {
            ScalarDomainEstablishmentRoute::ExactMachine {
                declaration,
                machine_identity: identities
                    .intern(&checked_evidence_machine_identity(checked, machine)?),
            }
        }
    })
}

/// Normalize one authored establishment catalog to its source-free form.
/// The emitted rows are strictly ordered; each identity is the declaration's
/// canonical normalized overload identity, so private issuers retain their
/// exact names without gaining consumer authority. A declaration authorizes its
/// retained executable applications, not a provider chosen by this pass. Expand
/// that same roster used by boundary publication; each application's contract
/// still determines which exact domain instance it can return.
pub(crate) fn establishment_routes(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    routes: &[language_semantics::DomainEstablishmentRoute],
) -> Result<Vec<terminal_psi::StructuralEstablishmentRoute>, LoweringError> {
    let mut lowered = Vec::new();
    for route in routes {
        let (owner, requirement) = match route {
            language_semantics::DomainEstablishmentRoute::CheckedRequirement {
                trait_definition,
                requirement,
            } => (*trait_definition, *requirement),
            language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
                boundary_trait,
                requirement,
            } => (*boundary_trait, *requirement),
            language_semantics::DomainEstablishmentRoute::ExactMachine { machine } => {
                lowered.extend(exact_machine_routes(identities, checked, *machine)?);
                continue;
            }
        };
        let definition =
            checked
                .trait_definition_by_symbol(owner)
                .ok_or(LoweringError::Unsupported(
                    "domain establishment route names an absent trait",
                ))?;
        let declared = checked
            .trait_machine_signature_by_symbol(definition, requirement)
            .ok_or(LoweringError::Unsupported(
                "domain establishment route names an absent requirement",
            ))?;
        for signature in checked.executable_requirement_machine_instances(declared) {
            let mut application = *route;
            match &mut application {
                language_semantics::DomainEstablishmentRoute::CheckedRequirement {
                    requirement,
                    ..
                }
                | language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
                    requirement,
                    ..
                } => *requirement = signature.symbol,
                language_semantics::DomainEstablishmentRoute::ExactMachine { .. } => {}
            }
            lowered.push(establishment_route(identities, checked, &application)?);
        }
    }
    Ok(lowered)
}

/// An authored exact issuer and the checked bodies invoking that declaration.
/// Target-family membership and application lineage are explicit checked
/// relations; equal signatures or names cannot authorize another body.
pub(super) fn exact_machine_routes(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    issuer: symbols::SymbolHandle,
) -> Result<Vec<ScalarDomainEstablishmentRoute>, LoweringError> {
    let mut routes = vec![establishment_route(
        identities,
        checked,
        &DomainEstablishmentRoute::ExactMachine { machine: issuer },
    )?];
    for candidate in checked.machines() {
        if candidate.symbol != issuer
            && checked
                .typed
                .machine_invokes_declaration(candidate.symbol, issuer)
                .map_err(LoweringError::Unsupported)?
        {
            routes.push(establishment_route(
                identities,
                checked,
                &DomainEstablishmentRoute::ExactMachine {
                    machine: candidate.symbol,
                },
            )?);
        }
    }
    Ok(routes)
}

/// Bind a checked return decision to the exact body invoking its issuer.
pub(super) fn return_route(
    checked: &CheckedTrees,
    returning: symbols::SymbolHandle,
    route: &DomainEstablishmentRoute,
) -> Result<DomainEstablishmentRoute, LoweringError> {
    let DomainEstablishmentRoute::ExactMachine { machine: issuer } = route else {
        return Ok(*route);
    };
    if !checked
        .typed
        .machine_invokes_declaration(returning, *issuer)
        .map_err(LoweringError::Unsupported)?
    {
        return Err(LoweringError::Unsupported(
            "checked scalar return has no declaration correspondence to its issuer",
        ));
    }
    Ok(DomainEstablishmentRoute::ExactMachine { machine: returning })
}
