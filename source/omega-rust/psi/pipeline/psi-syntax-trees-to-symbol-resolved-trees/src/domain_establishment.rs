use psi_diagnostics::Diagnostic;
use psi_language_semantics::DomainEstablishmentRoute;
use psi_language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionExposure as Exposure,
    AuthoredDeclarationSelectionKind as SelectionKind, AuthoredDeclarationSelectionRecordError,
};
use psi_source::{SourceSpan, Span};
use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbol_resolved_trees::domain::ProofFact;
use psi_symbol_resolved_trees::expression::ExpressionNode;
use psi_symbol_resolved_trees::name::DiagnosticName;
use psi_symbol_resolved_trees::signature::{SignatureContract, SignatureContractKind};
use psi_symbol_resolved_trees::types::TypeReference;
use psi_symbols::SymbolHandle;

use crate::signature_free_requirements::{
    SignatureFreeRequirementResolutionError, resolve_signature_free_requirement, same_semantic_name,
};

#[derive(Debug, Clone, Copy)]
struct AuthoredRouteResolution {
    domain_symbol: SymbolHandle,
    route: DomainEstablishmentRoute,
    trait_source_span: SourceSpan,
    requirement_source_span: SourceSpan,
    exposure: Exposure,
}

/// Normalize authored domain-introduction relationships after every
/// declaration and contract fact has a symbol.
///
/// This is the sole projection point for the currently landed route sources.
/// Checked consumers consult these identities instead of reconstructing owner
/// authority from attachment names or contract placement.
pub(crate) fn normalize_domain_establishment_routes(
    program: &mut SymbolResolvedTrees,
) -> Result<(), Diagnostic> {
    let mut resolutions = Vec::new();
    collect_authored_requirement_routes(program, &mut resolutions)?;

    program.domain_definitions.for_each_mut(|domain| {
        domain.establishment_routes.clear();
        for resolution in &resolutions {
            if resolution.domain_symbol == domain.symbol
                && !domain.establishment_routes.contains(&resolution.route)
            {
                domain.establishment_routes.push(resolution.route);
            }
        }
    });

    for resolution in resolutions {
        program
            .record_resolved_authored_declaration_selection(
                resolution.trait_source_span,
                resolution.exposure,
                SelectionKind::TypeReference,
                resolution.route.source_symbol(),
            )
            .map_err(selection_diagnostic)?;
        program
            .record_resolved_authored_declaration_selection(
                resolution.requirement_source_span,
                resolution.exposure,
                SelectionKind::StaticPathSegment,
                resolution.route.requirement_symbol(),
            )
            .map_err(selection_diagnostic)?;
    }
    Ok(())
}

fn collect_authored_requirement_routes(
    program: &SymbolResolvedTrees,
    resolutions: &mut Vec<AuthoredRouteResolution>,
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
            let route = if trait_definition.is_boundary {
                DomainEstablishmentRoute::BoundaryRequirement {
                    boundary_trait: trait_definition.symbol,
                    requirement: requirement.symbol,
                }
            } else {
                DomainEstablishmentRoute::CheckedRequirement {
                    trait_definition: trait_definition.symbol,
                    requirement: requirement.symbol,
                }
            };
            let [trait_path @ .., requirement_name] = path.as_slice() else {
                unreachable!("resolved establishment route has a trait and requirement")
            };
            resolutions.push(AuthoredRouteResolution {
                domain_symbol: domain.symbol,
                route,
                trait_source_span: path_source_span(trait_path),
                requirement_source_span: requirement_name.source_span(),
                exposure: if domain.is_public {
                    Exposure::PublicInterface
                } else {
                    Exposure::PrivateImplementation
                },
            });
        }
    }
    Ok(())
}

fn path_source_span(path: &[DiagnosticName]) -> SourceSpan {
    let first = path
        .first()
        .expect("resolved establishment route has a trait path")
        .source_span();
    let last = path
        .last()
        .expect("resolved establishment route has a trait path")
        .source_span();
    if first.source_id == last.source_id {
        SourceSpan::new(first.source_id, Span::new(first.span.start, last.span.end))
    } else {
        first
    }
}

fn selection_diagnostic(error: AuthoredDeclarationSelectionRecordError) -> Diagnostic {
    Diagnostic::error(format!(
        "failed to retain domain establishment-route declaration selection: {error:?}"
    ))
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
