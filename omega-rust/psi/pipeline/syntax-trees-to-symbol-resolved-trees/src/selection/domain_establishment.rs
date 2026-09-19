//! Domain establishment routes: which requirement introduces a domain.
//!
//! This is the sole projection point for route sources. Checked consumers
//! read these identities instead of reconstructing owner authority from
//! attachment names or contract placement.

use diagnostics::Diagnostic;
use language_semantics::DomainEstablishmentRoute;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionExposure as Exposure,
    AuthoredDeclarationSelectionKind as SelectionKind, AuthoredDeclarationSelectionRecordError,
};
use source::{SourceSpan, Span};
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::domain::ProofFact;
use symbol_resolved_trees::expression::ExpressionNode;
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::signature::{SignatureContract, SignatureContractKind};
use symbol_resolved_trees::types::TypeReference;
use symbols::SymbolHandle;

use crate::selection::signature_free_requirements::{
    SignatureFreeMachineResolutionError, SignatureFreeRequirementResolutionError,
    resolve_signature_free_machine, resolve_signature_free_requirement,
};

#[derive(Debug, Clone)]
struct AuthoredRouteResolution {
    domain_symbol: SymbolHandle,
    route: DomainEstablishmentRoute,
    /// `(source span, selection kind, selected symbol)` records the authored
    /// path's segment resolutions: requirement routes retain their trait
    /// prefix and requirement leaf; a machine route retains its optional
    /// data-owner prefix plus the machine leaf.
    selections: Vec<(SourceSpan, SelectionKind, SymbolHandle)>,
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
        for (source_span, kind, symbol) in resolution.selections {
            record_route_selection_once(program, source_span, resolution.exposure, kind, symbol)?;
        }
    }
    Ok(())
}

fn record_route_selection_once(
    program: &mut SymbolResolvedTrees,
    source_span: SourceSpan,
    exposure: Exposure,
    kind: SelectionKind,
    symbol: SymbolHandle,
) -> Result<(), Diagnostic> {
    use symbol_resolved_trees::AuthoredDeclarationSelectionTarget;

    let retained = program
        .authored_declaration_selections()
        .iter()
        .any(|selection| {
            selection.source_span() == source_span
                && selection.exposure() == exposure
                && selection.kind() == kind
                && matches!(
                    selection.target(),
                    AuthoredDeclarationSelectionTarget::Resolved(target)
                        if target.selected_symbol() == symbol
                )
        });
    if !retained {
        program
            .record_resolved_authored_declaration_selection(source_span, exposure, kind, symbol)
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
            let rendered = path
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            let requirement = resolve_signature_free_requirement(program, path);
            let machine = resolve_signature_free_machine(program, path);
            let resolved = match (requirement, machine) {
                (Ok(_), Ok(machine)) => {
                    return Err(Diagnostic::error(format!(
                        "domain `{}` establishment route `{rendered}` is ambiguous across declaration kinds: it names both a trait requirement and machine `{}`",
                        domain.name,
                        machine.machine.name
                    )));
                }
                (Err(_), Ok(machine)) => {
                    if !machine_authorizes_domain_subject(program, machine.machine, domain.symbol) {
                        return Err(Diagnostic::error(format!(
                            "domain `{}` authorizes `{rendered}` but that machine does not name the domain on its exact result",
                            domain.name
                        )));
                    }
                    let mut selections = Vec::new();
                    if machine.machine.attached_data_symbol.is_valid() {
                        selections.push((
                            path_source_span(&path[..path.len() - 1]),
                            SelectionKind::TypeReference,
                            machine.machine.attached_data_symbol,
                        ));
                    }
                    selections.push((
                        path.last().expect("route path nonempty").source_span(),
                        SelectionKind::StaticPathSegment,
                        machine.machine.symbol,
                    ));
                    resolutions.push(AuthoredRouteResolution {
                        domain_symbol: domain.symbol,
                        route: DomainEstablishmentRoute::ExactMachine {
                            machine: machine.machine.symbol,
                        },
                        selections,
                        exposure: if domain.is_public {
                            Exposure::PublicInterface
                        } else {
                            Exposure::PrivateImplementation
                        },
                    });
                    continue;
                }
                (resolved, Err(machine_error)) => resolved.map_err(|error| match error {
                    SignatureFreeRequirementResolutionError::InvalidPath => Diagnostic::error(
                        format!(
                            "domain `{}` establishment route must name an exact `Trait::requirement` or one exact machine declaration",
                            domain.name
                        ),
                    ),
                    SignatureFreeRequirementResolutionError::TraitNotUnique => {
                        Diagnostic::error(format!(
                            "domain `{}` establishment route `{rendered}` does not resolve to one exact trait",
                            domain.name
                        ))
                    }
                    SignatureFreeRequirementResolutionError::RequirementNotUnique => {
                        match machine_error {
                            SignatureFreeMachineResolutionError::NotUnique => Diagnostic::error(format!(
                                "domain `{}` establishment route `{rendered}` resolves to neither one exact trait requirement nor one exact machine declaration",
                                domain.name
                            )),
                        }
                    }
                })?,
            };
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
                selections: vec![
                    (
                        path_source_span(trait_path),
                        SelectionKind::TypeReference,
                        route.source_symbol(),
                    ),
                    (
                        requirement_name.source_span(),
                        SelectionKind::StaticPathSegment,
                        route.established_declaration(),
                    ),
                ],
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

/// An exact-machine route is authorized only when the named machine's own
/// invocation produces the domain subject: its `ensures` memberships on
/// `result` or a state return type that names the domain. Parameter subjects
/// stay boundary-requirement-only; a machine route cannot borrow requirement
/// authority it does not declare.
fn machine_authorizes_domain_subject(
    program: &SymbolResolvedTrees,
    machine: &symbol_resolved_trees::machine::Machine,
    domain_symbol: SymbolHandle,
) -> bool {
    if ensured_result_domain_symbols(program, program.machine_contracts(machine))
        .contains(&domain_symbol)
    {
        return true;
    }
    program
        .machine_state_handles(machine.states)
        .iter()
        .map(|state_handle| program.machine_state(*state_handle))
        .any(|state| {
            state.return_type.as_ref().is_some_and(|return_type| {
                type_reference_domain_symbols(program, return_type).contains(&domain_symbol)
            })
        })
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
    requirement: &symbol_resolved_trees::signature::StateSignature,
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
        let symbol_resolved_trees::types::TypeConstraint::Domain(name) = constraint else {
            continue;
        };
        let matched = program
            .domain_definitions
            .iter()
            .filter(|domain| {
                program
                    .symbols
                    .source_reference_can_see_symbol(name.name.source_span(), domain.symbol)
                    && crate::symbols::domain_name_reaches(
                        &program.symbols,
                        domain.symbol,
                        domain.name.as_str(),
                        name.name.as_str(),
                        name.name.source_span(),
                    )
            })
            .map(|domain| domain.symbol)
            .collect::<Vec<_>>();
        for matching in crate::symbols::prefer_module_local_domain(
            &program.symbols,
            matched,
            name.name.source_span(),
        ) {
            for atom in atomic_domain_symbols(program, matching) {
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
) -> Option<&symbol_resolved_trees::domain::DomainDefinition> {
    program
        .domain_definitions
        .iter()
        .find(|domain| domain.symbol == symbol)
}

fn expression_is_bare_result(
    program: &SymbolResolvedTrees,
    expression: symbol_resolved_trees::expression::ExpressionHandle,
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
