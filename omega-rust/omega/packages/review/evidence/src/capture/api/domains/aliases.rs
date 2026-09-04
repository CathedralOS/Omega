use crate::capture::semantics::declarations::{nominal_identity, trait_requirement_identity};
use crate::record::{
    PackageReviewDomainAliasAtom, PackageReviewDomainEstablishmentKind,
    PackageReviewDomainEstablishmentRoute,
};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn project_domain_alias_expansion(
    compilation: &CheckedCompilation,
    domain_symbol: SymbolHandle,
) -> Result<Vec<PackageReviewDomainAliasAtom>, Vec<Diagnostic>> {
    fn expand(
        compilation: &CheckedCompilation,
        domain_symbol: SymbolHandle,
        stack: &mut Vec<SymbolHandle>,
        atoms: &mut Vec<PackageReviewDomainAliasAtom>,
    ) -> Result<(), Vec<Diagnostic>> {
        if stack.contains(&domain_symbol) {
            return Err(vec![Diagnostic::error(
                "package review encountered a cycle in checked domain alias expansion",
            )]);
        }
        let definitions = compilation
            .domain_definitions()
            .iter()
            .filter(|candidate| candidate.symbol == domain_symbol)
            .collect::<Vec<_>>();
        let [definition] = definitions.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "package review domain alias resolves to {} declarations; expected exactly one",
                definitions.len()
            ))]);
        };
        let Some(alias) = definition.alias.as_ref() else {
            atoms.push(PackageReviewDomainAliasAtom::Declared(nominal_identity(
                compilation,
                definition.symbol,
            )?));
            return Ok(());
        };
        stack.push(domain_symbol);
        for constituent in &alias.constituents {
            let label = compilation
                .domain_path_members(constituent.domain)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            if !constituent.domain_symbol.is_valid() && label == "Carry::Portable" {
                atoms.extend(
                    psi_language_semantics::CarryPermission::ALL
                        .map(PackageReviewDomainAliasAtom::Carry),
                );
            } else if !constituent.domain_symbol.is_valid()
                && let Some(permission) = psi_language_semantics::CarryPermission::from_name(&label)
            {
                atoms.push(PackageReviewDomainAliasAtom::Carry(permission));
            } else {
                if !constituent.domain_symbol.is_valid() {
                    return Err(vec![Diagnostic::error(format!(
                        "package review domain alias has unresolved constituent `{label}`"
                    ))]);
                }
                expand(compilation, constituent.domain_symbol, stack, atoms)?;
            }
        }
        stack.pop();
        Ok(())
    }

    let mut atoms = Vec::new();
    expand(compilation, domain_symbol, &mut Vec::new(), &mut atoms)?;
    atoms.sort();
    atoms.dedup();
    if atoms.is_empty() {
        return Err(vec![Diagnostic::error(
            "package review domain alias has an empty canonical expansion",
        )]);
    }
    Ok(atoms)
}

pub(crate) fn project_domain_establishment_route(
    compilation: &CheckedCompilation,
    route: psi_language_semantics::DomainEstablishmentRoute,
) -> Result<PackageReviewDomainEstablishmentRoute, Vec<Diagnostic>> {
    let (kind, trait_symbol, requirement_symbol, expects_boundary) = match route {
        psi_language_semantics::DomainEstablishmentRoute::CheckedRequirement {
            trait_definition,
            requirement,
        } => (
            PackageReviewDomainEstablishmentKind::CheckedRequirement,
            trait_definition,
            requirement,
            false,
        ),
        psi_language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
            boundary_trait,
            requirement,
        } => (
            PackageReviewDomainEstablishmentKind::BoundaryRequirement,
            boundary_trait,
            requirement,
            true,
        ),
    };
    let owners = compilation
        .traits()
        .iter()
        .filter(|candidate| candidate.symbol == trait_symbol)
        .collect::<Vec<_>>();
    let [owner] = owners.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "package review domain establishment route resolves to {} trait declarations; expected exactly one",
            owners.len()
        ))]);
    };
    if owner.is_boundary != expects_boundary {
        return Err(vec![Diagnostic::error(
            "package review domain establishment route kind disagrees with its exact trait declaration",
        )]);
    }
    let requirements = compilation
        .trait_machine_signatures(owner)
        .iter()
        .filter(|candidate| candidate.symbol == requirement_symbol)
        .collect::<Vec<_>>();
    let [requirement] = requirements.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "package review domain establishment route resolves to {} requirements under its exact trait; expected exactly one",
            requirements.len()
        ))]);
    };
    Ok(PackageReviewDomainEstablishmentRoute {
        kind,
        trait_identity: nominal_identity(compilation, owner.symbol)?,
        requirement_identity: trait_requirement_identity(compilation, owner, requirement)?,
    })
}
