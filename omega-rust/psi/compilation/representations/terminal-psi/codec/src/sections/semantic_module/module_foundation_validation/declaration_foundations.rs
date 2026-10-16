//! Module-level declarations: structural domains, services and their
//! reach, boundary machines and provider candidates.

use super::{
    has_service, has_structural_type, require_known_services, validate_service_parent_graph,
    validate_structural_parameters, validate_structural_qualifications,
};
use std::collections::BTreeMap;

use crate::codec_error::{CodecError, malformed};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, StructuralPlaceKind};
use terminal_psi::TerminalModule;
use terminal_psi::{BoundaryMachineDeclaration, ProviderCandidateConformance, TerminalMachine};

/// Every structural domain names a known carrier type.
pub(super) fn validate_domains(module: &TerminalModule) -> Result<(), CodecError> {
    for domain in &module.structural_domains {
        if domain.predicate.as_ref().is_some_and(|predicate| {
            !terminal_semantics::domain_predicate_is_closed(
                module.structural_types.iter(),
                domain.carrier,
                predicate,
            )
        }) {
            return malformed(
                "structural domain predicate escapes its declared subject or carrier",
            );
        }
        if !has_structural_type(module, domain.carrier) {
            return malformed("structural domain references an unknown carrier type");
        }
    }
    Ok(())
}

/// Every service names known parents other than itself, the parent graph
/// is well founded, and the root service reach and its installation
/// dependencies name known services.
pub(super) fn validate_services(module: &TerminalModule) -> Result<(), CodecError> {
    for service in &module.services {
        if service
            .parents
            .iter()
            .any(|parent| *parent == service.id || !has_service(module, *parent))
        {
            return malformed("service references itself or an unknown parent");
        }
    }
    validate_service_parent_graph(module)?;
    require_known_services(module, &module.root_service_reach.concrete)?;
    for dependency in &module.root_service_reach.installation_dependencies {
        if dependency.requirement_identity.is_absent() {
            return malformed("installation reach requirement identity must be nonempty");
        }
        require_known_services(module, &dependency.upper_bound)?;
    }
    Ok(())
}

/// One boundary machine's attachment, structural parameters,
/// requirements and service reach name known types, domains and services.
pub(super) fn validate_boundary_machine(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
) -> Result<(), CodecError> {
    terminal_semantics::closed_conformances::validate_boundary_callable(module, boundary)
        .map_err(CodecError::MalformedStructuralFoundation)?;
    if module
        .identities
        .text(boundary.owner.identity())
        .is_none_or(str::is_empty)
    {
        return malformed("boundary schema owner identity must be present");
    }
    if boundary.kind != terminal_psi::BoundaryRequirementKind::Machine
        && boundary.attachment.is_some()
    {
        return malformed("boundary requirement kind has an invalid nominal attachment");
    }
    if boundary
        .attachment
        .is_some_and(|attachment| !has_structural_type(module, attachment))
    {
        return malformed("boundary machine has an unknown attachment type");
    }
    validate_structural_parameters(module, &boundary.structural_parameters)?;
    super::scalar_observations::boundary(module, boundary)?;
    if let terminal_psi::BoundaryMachineResult::Structural(result) = &boundary.result {
        if !has_structural_type(module, result.structural_type) {
            return malformed("boundary result references an unknown structural type");
        }
        validate_structural_qualifications(
            module,
            result.structural_type,
            &result.qualifications,
            &result.projected_qualifications,
        )?;
    }
    if !boundary.has_valid_parameter_order() {
        return malformed("boundary parameter order does not cover its lanes exactly");
    }
    for requirement in &boundary.requires {
        let Some(parameter) = boundary
            .structural_parameters
            .get(requirement.argument_index as usize)
        else {
            return malformed("boundary requirement has an unknown argument index");
        };
        let Some(domain) = module.structural_domain(requirement.domain) else {
            return malformed("boundary requirement references an unknown domain");
        };
        if domain.carrier != parameter.structural_type {
            return malformed("boundary requirement domain has the wrong carrier type");
        }
    }
    require_known_services(module, &boundary.published_service_ceiling)?;
    require_known_services(module, &boundary.fixed_service_reach)?;
    validate_boundary_scalar_rows(boundary)?;
    if let Some(owner) = boundary.owning_service {
        if boundary.kind != terminal_psi::BoundaryRequirementKind::Trait {
            return malformed("non-trait boundary names an owning service");
        }
        require_known_services(module, &[owner])?;
    }
    Ok(())
}

/// One provider candidate carries nonempty identities and names a known
/// boundary, candidate machine and signature types.
pub(super) fn validate_provider_candidate(
    module: &TerminalModule,
    candidate: &ProviderCandidateConformance,
) -> Result<(), CodecError> {
    if candidate.receiving_application.is_absent() {
        if !candidate.receiving_region_arguments.is_empty() {
            return malformed("provider regions have no receiving application");
        }
    } else if module
        .identities
        .text(candidate.receiving_application)
        .is_none_or(str::is_empty)
        || candidate
            .receiving_region_arguments
            .iter()
            .any(|region| module.identities.text(*region).is_none_or(str::is_empty))
    {
        return malformed("provider receiving application references an empty identity");
    }
    if candidate.requirement_identity.is_absent()
        || module
            .identities
            .text(candidate.source_requirement_identity)
            .is_none_or(str::is_empty)
        || module
            .identities
            .text(candidate.source_candidate_identity)
            .is_none_or(str::is_empty)
        || candidate.provider_identity.is_absent()
        || candidate.candidate_identity.is_absent()
        || module
            .identities
            .text(candidate.provider_declaration.identity())
            .is_none_or(str::is_empty)
    {
        return malformed("provider candidate identities must be nonempty");
    }
    let Some(boundary) = module.boundary_machine(candidate.boundary) else {
        return malformed("provider candidate references an unknown terminal ID");
    };
    if !candidate.receiving_application.is_absent()
        && boundary.kind != terminal_psi::BoundaryRequirementKind::Trait
    {
        return malformed("provider receiving trait application names a non-trait boundary");
    }
    let Some(machine) = module.machine(candidate.candidate) else {
        return malformed("provider candidate references an unknown terminal ID");
    };
    if boundary.role != terminal_psi::BoundaryDeclarationRole::Callable {
        return malformed("provider candidate references contract evidence rather than a callable");
    }
    validate_provider_scalar_contract(boundary, machine, &module.structural_domains)?;
    for parameter in &candidate.signature.parameters {
        if !has_structural_type(module, parameter.structural_type) {
            return malformed("provider signature references an unknown structural type");
        }
        if parameter
            .qualifications
            .iter()
            .any(|domain| !module.structural_domain(*domain).is_some())
        {
            return malformed("provider signature references an unknown structural domain");
        }
    }
    require_known_services(module, &candidate.refinement.realized_service_ceiling)?;
    Ok(())
}

/// The declaration's scalar contract rows are propositions over its own
/// telescope: `scalar_requires` names input formals and structural-parameter
/// places (a `ViewExtent` bound is how a view's caller-owed bound publishes),
/// `scalar_ensures` additionally names the scalar result. Every call and
/// provider candidate instantiates them in its own namespace, so a row may
/// not reach outside this declaration's formals.
fn validate_boundary_scalar_rows(boundary: &BoundaryMachineDeclaration) -> Result<(), CodecError> {
    let Some(mut formals) = boundary.scalar_contract_parameters() else {
        return malformed("boundary scalar contract formals overflow");
    };
    let places = boundary
        .structural_parameters
        .iter()
        .map(|parameter| {
            (
                parameter.place,
                StructuralPlaceKind::Parameter {
                    position: parameter.position,
                },
            )
        })
        .collect::<Vec<_>>();
    let requires = PropositionContext::from_value_types_and_places(
        formals.iter().map(|formal| (formal.id, formal.scalar_type)),
        places.iter().copied(),
    )
    .map_err(|_| CodecError::MalformedStructuralFoundation("boundary scalar requires telescope"))?;
    for proposition in &boundary.scalar_requires {
        requires.validate(proposition).map_err(|_| {
            CodecError::MalformedStructuralFoundation(
                "boundary scalar requires names an undeclared formal",
            )
        })?;
    }
    if boundary.scalar_ensures.is_empty() {
        return Ok(());
    }
    if let Some(result) = boundary.scalar_contract_result() {
        formals.push(result);
    }
    let ensures = PropositionContext::from_value_types_and_places(
        formals.iter().map(|formal| (formal.id, formal.scalar_type)),
        places.iter().copied(),
    )
    .map_err(|_| CodecError::MalformedStructuralFoundation("boundary scalar ensures telescope"))?;
    for proposition in &boundary.scalar_ensures {
        ensures.validate(proposition).map_err(|_| {
            CodecError::MalformedStructuralFoundation(
                "boundary scalar ensures names an undeclared formal",
            )
        })?;
    }
    Ok(())
}

/// Scalar agreement between a boundary requirement's published contract and
/// one provider candidate's own checked contract, translated into the
/// candidate's value namespace. The declaration's rows are the public
/// contract: a candidate may strengthen them — narrower `requires`, wider
/// `ensures` — but never narrow them, so a selected body that cannot cover
/// the declared guarantees is malformed rather than silently weakening what
/// callers import.
fn validate_provider_scalar_contract(
    boundary: &BoundaryMachineDeclaration,
    machine: &TerminalMachine,
    domains: &[terminal_psi::StructuralDomainDeclaration],
) -> Result<(), CodecError> {
    if boundary.scalar_parameters.len() != machine.parameters.len()
        || boundary
            .scalar_parameters
            .iter()
            .zip(&machine.parameters)
            .any(|(required, actual)| *required != actual.scalar_type)
    {
        return malformed("provider candidate scalar signature disagrees with its boundary");
    }
    if !boundary
        .result
        .accepts_provider_result(&machine.result, boundary.identity, domains)
    {
        return malformed("provider candidate result disagrees with its boundary");
    }
    if boundary.structural_parameters.len() != machine.structural_parameters.len()
        || boundary
            .structural_parameters
            .iter()
            .zip(&machine.structural_parameters)
            .any(|(required, actual)| {
                required.position != actual.position
                    || required.structural_type != actual.structural_type
            })
    {
        return malformed("provider candidate structural signature disagrees with its boundary");
    }
    // An erased formal has no boundary actual to substitute.
    if !machine.contract.erased_scalar_formals.is_empty()
        || !machine.contract.erased_proof_formals.is_empty()
    {
        return malformed("provider candidate carries erased formals its boundary cannot bind");
    }
    let Some(formals) = boundary.scalar_contract_parameters() else {
        return malformed("boundary scalar contract formals overflow");
    };
    let mut substitutions = formals
        .iter()
        .zip(&machine.parameters)
        .map(|(formal, actual)| (formal.id, ScalarTerm::value(actual.id, actual.scalar_type)))
        .collect::<BTreeMap<_, _>>();
    if let Some(formal) = boundary.scalar_contract_result()
        && let Some(actual) = machine.result.scalar()
    {
        substitutions.insert(formal.id, ScalarTerm::value(actual.id, actual.scalar_type));
    }
    // Structural observations use declaration-local places just as scalar
    // operands use declaration-local values. Independently allocated boundary
    // and provider formals need the same positional substitution; equal source
    // predicates must not depend on their unrelated place allocation order.
    let places = boundary
        .structural_parameters
        .iter()
        .zip(&machine.structural_parameters)
        .map(|(formal, actual)| (formal.place, (actual.place, Vec::new())))
        .collect::<BTreeMap<_, _>>();
    let instantiate = |row: &Proposition| {
        let values =
            terminal_semantics::substitution::substitute_proposition_values(row, &substitutions);
        terminal_semantics::substitution::substitute_proposition_structural_places(&values, &places)
    };
    // Roster rows are folded `Conjunction`s on the machine side, so coverage
    // compares conjunct sets: a published `A && B` is the established facts
    // `A` and `B`, never a weaker reading. `contract.requires` merges authored
    // clauses with generated runtime obligations (entry storage, view
    // bounds), so requires agreement replays over the authored
    // `source_checked_requires` subset: every requirement the candidate
    // actually demands must occur in the declared roster after substitution
    // — a candidate may drop obligations but never demand ones its callers
    // do not owe.
    let declared = boundary
        .scalar_requires
        .iter()
        .map(instantiate)
        .collect::<Vec<_>>();
    let declared_conjuncts = declared.iter().flat_map(conjuncts).collect::<Vec<_>>();
    if machine
        .contract
        .source_checked_requires
        .iter()
        .flat_map(conjuncts)
        .any(|part| !declared_conjuncts.contains(&part))
    {
        return malformed("provider candidate requires beyond its boundary's declared obligations");
    }
    // Every declared guarantee must occur in the candidate's own checked
    // ensures roster after substitution.
    let roster = machine
        .contract
        .ensures
        .iter()
        .flat_map(|clause| conjuncts(&clause.proposition))
        .collect::<Vec<_>>();
    if boundary.scalar_ensures.iter().any(|row| {
        let substituted = instantiate(row);
        !conjuncts(&substituted)
            .iter()
            .all(|part| roster.contains(part))
    }) {
        return malformed("provider candidate does not ensure its boundary's declared guarantees");
    }
    Ok(())
}

/// The flat conjunct list of a contract proposition. Contract rosters fold
/// their rows through `connective`, which flattens nested conjunctions, so a
/// `Conjunction`'s parts are the atomic roster entries; anything else is one.
fn conjuncts(proposition: &Proposition) -> Vec<&Proposition> {
    match proposition {
        Proposition::Conjunction(parts) => parts
            .iter()
            .filter(|part| !matches!(part, Proposition::Truth))
            .collect(),
        Proposition::Truth => Vec::new(),
        proposition => vec![proposition],
    }
}
