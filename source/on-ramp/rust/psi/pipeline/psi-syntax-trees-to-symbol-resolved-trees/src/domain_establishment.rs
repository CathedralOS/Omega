use psi_diagnostics::Diagnostic;
use psi_language_semantics::DomainEstablishmentRoute;
use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbol_resolved_trees::domain::ProofFact;
use psi_symbol_resolved_trees::expression::ExpressionNode;
use psi_symbol_resolved_trees::signature::{SignatureContract, SignatureContractKind};
use psi_symbol_resolved_trees::types::TypeReference;
use psi_symbols::SymbolHandle;

use crate::signature_free_requirements::{
    SignatureFreeRequirementResolutionError, resolve_signature_free_requirement, same_semantic_name,
};

/// Normalize authored domain-introduction relationships after every
/// declaration and contract fact has a symbol.
///
/// This is the sole projection point for the currently landed route sources.
/// Checked consumers consult these identities instead of reconstructing owner
/// authority from attachment names or contract placement.
pub(crate) fn normalize_domain_establishment_routes(
    program: &mut SymbolResolvedTrees,
) -> Result<(), Diagnostic> {
    let mut additions = Vec::new();
    collect_authored_requirement_routes(program, &mut additions)?;

    program.domain_definitions.for_each_mut(|domain| {
        domain.establishment_routes.clear();
        for (domain_symbol, route) in &additions {
            if *domain_symbol == domain.symbol && !domain.establishment_routes.contains(route) {
                domain.establishment_routes.push(*route);
            }
        }
    });
    Ok(())
}

fn collect_authored_requirement_routes(
    program: &SymbolResolvedTrees,
    additions: &mut Vec<(SymbolHandle, DomainEstablishmentRoute)>,
) -> Result<(), Diagnostic> {
    for domain in &program.domain_definitions {
        if domain.alias.is_some() && !domain.authored_routes.is_empty() {
            return Err(Diagnostic::error(format!(
                "domain alias `{}` cannot author establishment routes; routes belong to its atomic declarations",
                domain.name
            )));
        }
        for path in &domain.authored_routes {
            let resolved = resolve_signature_free_requirement(program, path).map_err(|error| {
                let route = path
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("::");
                match error {
                    SignatureFreeRequirementResolutionError::InvalidPath => Diagnostic::error(
                        format!(
                            "domain `{}` establishment route must name an exact `Trait::requirement`",
                            domain.name
                        ),
                    ),
                    SignatureFreeRequirementResolutionError::TraitNotUnique => {
                        Diagnostic::error(format!(
                            "domain `{}` establishment route `{route}` does not resolve to one exact trait",
                            domain.name
                        ))
                    }
                    SignatureFreeRequirementResolutionError::RequirementNotUnique => {
                        Diagnostic::error(format!(
                            "domain `{}` establishment route `{route}` does not resolve to one exact trait requirement",
                            domain.name
                        ))
                    }
                }
            })?;
            let trait_definition = resolved.trait_definition;
            let requirement = resolved.requirement;
            if !requirement_authorizes_domain_subject(
                program,
                requirement,
                domain.symbol,
                trait_definition.is_boundary,
            ) {
                return Err(Diagnostic::error(format!(
                    "domain `{}` authorizes `{}` but that requirement does not name the domain on its exact result or an exact non-self external-root parameter",
                    domain.name,
                    path.iter()
                        .map(|member| member.as_str())
                        .collect::<Vec<_>>()
                        .join("::")
                )));
            }
            additions.push((
                domain.symbol,
                if trait_definition.is_boundary {
                    DomainEstablishmentRoute::BoundaryRequirement {
                        boundary_trait: trait_definition.symbol,
                        requirement: requirement.symbol,
                    }
                } else {
                    DomainEstablishmentRoute::CheckedRequirement {
                        trait_definition: trait_definition.symbol,
                        requirement: requirement.symbol,
                    }
                },
            ));
        }
    }
    Ok(())
}

fn requirement_authorizes_domain_subject(
    program: &SymbolResolvedTrees,
    requirement: &psi_symbol_resolved_trees::signature::StateSignature,
    domain_symbol: SymbolHandle,
    permits_external_root_parameters: bool,
) -> bool {
    ensured_result_domain_symbols(program, program.signature_contracts(requirement.contracts))
        .contains(&domain_symbol)
        || requirement.return_type.as_ref().is_some_and(|return_type| {
            type_reference_domain_symbols(program, return_type).contains(&domain_symbol)
        })
        || permits_external_root_parameters
            && program
                .state_parameters(requirement.parameters)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .any(|parameter| {
                    type_reference_domain_symbols(program, &parameter.type_reference)
                        .contains(&domain_symbol)
                })
}

fn type_reference_domain_symbols(
    program: &SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> Vec<SymbolHandle> {
    let constrained = match type_reference {
        TypeReference::Reference(reference) => {
            return type_reference_domain_symbols(
                program,
                program.child_type_reference(reference.referee),
            );
        }
        TypeReference::Constrained(constrained) => constrained,
        TypeReference::FixedArray(_)
        | TypeReference::Slice(_)
        | TypeReference::Generic(_)
        | TypeReference::ConstExpression(_)
        | TypeReference::DynamicTrait { .. }
        | TypeReference::Named { .. }
        | TypeReference::SelfType { .. }
        | TypeReference::Unit => return Vec::new(),
    };
    let mut domains = Vec::new();
    for constraint in program
        .tables
        .types
        .constraints
        .span_or_empty(constrained.constraints)
    {
        let psi_symbol_resolved_trees::types::TypeConstraint::Domain(name) = constraint else {
            continue;
        };
        for matching in program
            .domain_definitions
            .iter()
            .filter(|domain| same_semantic_name(domain.name.as_str(), name.name.as_str()))
        {
            for atom in atomic_domain_symbols(program, matching.symbol) {
                if !domains.contains(&atom) {
                    domains.push(atom);
                }
            }
        }
    }
    for inherited in
        type_reference_domain_symbols(program, program.child_type_reference(constrained.base_type))
    {
        if !domains.contains(&inherited) {
            domains.push(inherited);
        }
    }
    domains
}

fn ensured_result_domain_symbols(
    program: &SymbolResolvedTrees,
    contracts: &[SignatureContract],
) -> Vec<SymbolHandle> {
    let mut domains = Vec::new();
    for contract in contracts
        .iter()
        .filter(|contract| contract.kind == SignatureContractKind::Ensures)
    {
        for fact in program.proof_facts(contract.facts) {
            let ProofFact::Membership(membership) = fact else {
                continue;
            };
            if !expression_is_bare_result(program, membership.value) {
                continue;
            }
            for domain_symbol in atomic_domain_symbols(program, membership.domain_symbol) {
                if domain_symbol.is_valid() && !domains.contains(&domain_symbol) {
                    domains.push(domain_symbol);
                }
            }
        }
    }
    domains
}

fn atomic_domain_symbols(
    program: &SymbolResolvedTrees,
    domain_symbol: SymbolHandle,
) -> Vec<SymbolHandle> {
    fn expand(
        program: &SymbolResolvedTrees,
        domain_symbol: SymbolHandle,
        stack: &mut Vec<SymbolHandle>,
        output: &mut Vec<SymbolHandle>,
    ) {
        if !domain_symbol.is_valid() || stack.contains(&domain_symbol) {
            return;
        }
        let Some(domain) = domain_definition(program, domain_symbol) else {
            return;
        };
        let Some(alias) = domain.alias.as_ref() else {
            if !output.contains(&domain_symbol) {
                output.push(domain_symbol);
            }
            return;
        };
        stack.push(domain_symbol);
        for constituent in &alias.constituents {
            expand(program, constituent.domain_symbol, stack, output);
        }
        stack.pop();
    }

    let mut output = Vec::new();
    expand(program, domain_symbol, &mut Vec::new(), &mut output);
    output
}

fn domain_definition(
    program: &SymbolResolvedTrees,
    symbol: SymbolHandle,
) -> Option<&psi_symbol_resolved_trees::domain::DomainDefinition> {
    program
        .domain_definitions
        .iter()
        .find(|domain| domain.symbol == symbol)
}

fn expression_is_bare_result(
    program: &SymbolResolvedTrees,
    expression: psi_symbol_resolved_trees::expression::ExpressionHandle,
) -> bool {
    let ExpressionNode::Name(path) = program.tables.bodies.expressions.expression(expression)
    else {
        return false;
    };
    let [name] = program
        .tables
        .bodies
        .expressions
        .name_path_members(path.members)
    else {
        return false;
    };
    name.as_str() == "result"
}
