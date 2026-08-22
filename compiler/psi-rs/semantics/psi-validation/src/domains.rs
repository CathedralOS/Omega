use crate::proof_facts::{ProofFactOwner, validate_domain_fact_payloads};
use crate::symbols::TopLevelSymbols;
use crate::type_references::{
    TypeReferenceOwner, type_reference_label, type_references_match,
    validate_type_reference_handle_with_type_parameters,
};
use psi_diagnostics::Diagnostic;
use psi_facts::FactPlan;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;

pub(crate) fn validate_domain_definitions(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    fact_plan: &FactPlan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_domain_aliases(program, diagnostics);
    validate_progress_profile_domains(program, diagnostics);
    validate_repeated_normalized_domain_identities(program, fact_plan, diagnostics);

    for domain in program.domain_definitions() {
        validate_type_reference_handle_with_type_parameters(
            program,
            domain.target_type,
            symbols,
            diagnostics,
            TypeReferenceOwner::DomainTarget {
                domain: domain.name.as_str(),
                generic_depth: 0,
            },
            program.domain_type_parameters(domain),
            &[],
        );
        validate_domain_fact_payloads(
            program,
            fact_plan,
            domain.symbol,
            diagnostics,
            ProofFactOwner::Domain(domain.name.as_str()),
        );
        validate_domain_membership_targets(program, fact_plan, domain, diagnostics);
    }

    validate_domain_membership_cycles(program, fact_plan, diagnostics);
    reject_checked_progress_dependencies_until_covered(program, diagnostics);
}

fn validate_progress_profile_domains(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for domain in program.domain_definitions().iter().filter(|domain| {
        domain.classification == Some(psi_language_semantics::DomainClassification::ProgressProfile)
    }) {
        if domain.alias.is_some() {
            diagnostics.push(Diagnostic::error(format!(
                "progress profile `{}` must be an atomic domain, not a transparent alias",
                domain.name
            )));
        }
        if domain.predicate_body.is_present() || !program.proof_facts(domain).is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "progress profile `{}` must be predicate-free; remove its `requires` facts",
                domain.name
            )));
        }
        if !program.domain_operators(domain).is_empty() || !domain.semantic_roles.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "progress profile `{}` is opaque and cannot contribute operators or other domain semantic roles",
                domain.name
            )));
        }
        if domain.establishment_routes.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "progress profile `{}` requires at least one exact `established by` boundary requirement",
                domain.name
            )));
        } else if domain.establishment_routes.iter().any(|route| {
            !matches!(
                route,
                psi_language_semantics::DomainEstablishmentRoute::BoundaryRequirement { .. }
            )
        }) {
            diagnostics.push(Diagnostic::error(format!(
                "progress profile `{}` may be established only by exact boundary trait requirements",
                domain.name
            )));
        }
    }
}

/// Public schemas are normalized, but checked call-edge instantiation and
/// receipt/manifest coverage are a later TPR6 slice. Keep checked bodies fail
/// closed while allowing bodyless requirements to publish the contract that
/// those edges will instantiate.
fn reject_checked_progress_dependencies_until_covered(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        let psi_language_semantics::TerminationInterface::Published(
            psi_language_semantics::TerminationGuarantee::Terminates { premises },
        ) = &machine.termination_plan.interface
        else {
            continue;
        };
        if machine.supply_mode == psi_language_semantics::MachineSupplyMode::CheckedBody
            && !premises.is_empty()
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has subject-bearing progress dependencies, but checked call-edge premise coverage is not implemented yet",
                machine.name
            )));
        }
    }
}

fn validate_domain_aliases(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for domain in program.domain_definitions() {
        let Some(alias) = domain.alias.as_ref() else {
            continue;
        };
        if alias.constituents.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "domain alias `{}` must name at least one constituent",
                domain.name
            )));
        }
        if domain.predicate_body.is_present()
            || !program.proof_facts(domain).is_empty()
            || !program.domain_operators(domain).is_empty()
        {
            diagnostics.push(Diagnostic::error(format!(
                "domain alias `{}` must be transparent; it cannot also declare predicate facts or operators",
                domain.name
            )));
        }

        for constituent in &alias.constituents {
            let label = program
                .domain_path_members(constituent.domain)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            if label == "Carry::Portable"
                || psi_language_semantics::CarryPermission::from_name(&label).is_some()
            {
                // Compiler carry atoms are deliberately subject-polymorphic,
                // and public aliases may bundle this closed public vocabulary.
                continue;
            }
            let Some(referenced) = domain_definition_by_symbol(program, constituent.domain_symbol)
            else {
                if label.starts_with("Carry::") {
                    diagnostics.push(Diagnostic::error(format!(
                        "domain alias `{}` references unknown compiler carry permission `{label}`; expected `Carry::AcrossSuspend`, `Carry::AnyCpu`, `Carry::AnyThread`, `Carry::MovableAddress`, or `Carry::Portable`",
                        domain.name
                    )));
                } else {
                    diagnostics.push(Diagnostic::error(format!(
                        "domain alias `{}` references unknown domain `{label}`",
                        domain.name
                    )));
                }
                continue;
            };
            if !type_references_match(program, domain.target_type, referenced.target_type) {
                diagnostics.push(Diagnostic::error(format!(
                    "domain alias `{}` includes `{}` but they classify different types: `{}` vs `{}`",
                    domain.name,
                    referenced.name,
                    type_reference_label(program, domain.target_type),
                    type_reference_label(program, referenced.target_type)
                )));
            }
            if domain.is_public && !referenced.is_public {
                diagnostics.push(Diagnostic::error(format!(
                    "public domain alias `{}` cannot publish private constituent `{}`",
                    domain.name, referenced.name
                )));
            }
        }
    }

    let mut reported = Vec::new();
    for domain in program.domain_definitions() {
        validate_domain_alias_cycle_from(
            program,
            domain.symbol,
            &mut Vec::new(),
            &mut reported,
            diagnostics,
        );
    }
}

fn validate_domain_alias_cycle_from(
    program: &TypedTrees,
    domain_symbol: SymbolHandle,
    path: &mut Vec<SymbolHandle>,
    reported: &mut Vec<SymbolHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !domain_symbol.is_valid() || reported.contains(&domain_symbol) {
        return;
    }
    if let Some(cycle_start) = path.iter().position(|symbol| *symbol == domain_symbol) {
        let cycle_symbols = path[cycle_start..]
            .iter()
            .copied()
            .chain(std::iter::once(domain_symbol))
            .collect::<Vec<_>>();
        let cycle = cycle_symbols
            .iter()
            .filter_map(|symbol| domain_definition_by_symbol(program, *symbol))
            .map(|domain| domain.name.to_string())
            .collect::<Vec<_>>()
            .join(" -> ");
        diagnostics.push(Diagnostic::error(format!("domain alias cycle: {cycle}")));
        reported.extend(cycle_symbols);
        return;
    }

    let Some(domain) = domain_definition_by_symbol(program, domain_symbol) else {
        return;
    };
    let Some(alias) = domain.alias.as_ref() else {
        return;
    };
    path.push(domain_symbol);
    for constituent in &alias.constituents {
        validate_domain_alias_cycle_from(
            program,
            constituent.domain_symbol,
            path,
            reported,
            diagnostics,
        );
    }
    path.pop();
}

fn validate_repeated_normalized_domain_identities(
    program: &TypedTrees,
    fact_plan: &FactPlan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let domains = program.domain_definitions();
    for (index, domain) in domains.iter().enumerate() {
        if domains[..index]
            .iter()
            .any(|prior| prior.name == domain.name)
        {
            continue;
        }
        let normalized_facts = normalized_domain_facts(program, fact_plan, domain.symbol);
        let mut peers = domains
            .iter()
            .skip(index + 1)
            .filter(|peer| peer.name == domain.name);
        if peers.any(|peer| {
            peer.semantic_id != domain.semantic_id
                || peer.predicate_body != domain.predicate_body
                || peer.classification != domain.classification
                || peer.semantic_roles != domain.semantic_roles
                || peer.establishment_routes != domain.establishment_routes
                || normalized_domain_facts(program, fact_plan, peer.symbol) != normalized_facts
        }) {
            diagnostics.push(Diagnostic::error(format!(
                "domain `{}` is declared more than once with different normalized semantics; repeated capacity specializations may share a name only when their semantic identities are equal",
                domain.name
            )));
        }
    }
}

fn normalized_domain_facts(
    program: &TypedTrees,
    fact_plan: &FactPlan,
    domain_symbol: SymbolHandle,
) -> Vec<String> {
    let mut facts = fact_plan
        .facts_for_symbol(domain_symbol)
        .filter_map(|fact| match fact.payload {
            psi_facts::FactPayload::BooleanExpression(expression) => Some(format!(
                "bool:{}",
                program.expression_table.display_name(expression)
            )),
            psi_facts::FactPayload::DomainMembership {
                value,
                domain_symbol,
                ..
            } => {
                let semantic_id = domain_definition_by_symbol(program, domain_symbol)
                    .map(|domain| domain.semantic_id)
                    .unwrap_or_default();
                Some(format!(
                    "membership:{}:{semantic_id:?}",
                    program.expression_table.display_name(value)
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    facts.sort_unstable();
    facts.dedup();
    facts
}

fn validate_domain_membership_targets(
    program: &TypedTrees,
    fact_plan: &FactPlan,
    domain: &psi_typed_trees::domain::DomainDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for membership in domain_membership_facts(fact_plan, domain.symbol) {
        let Some(referenced_domain) =
            domain_definition_by_symbol(program, membership.domain_symbol)
        else {
            continue;
        };

        if type_references_match(program, domain.target_type, referenced_domain.target_type) {
            continue;
        }

        diagnostics.push(Diagnostic::error(format!(
            "domain `{}` imports `{}` but they classify different types: `{}` vs `{}`",
            domain.name,
            referenced_domain.name,
            type_reference_label(program, domain.target_type),
            type_reference_label(program, referenced_domain.target_type)
        )));
    }
}

fn validate_domain_membership_cycles(
    program: &TypedTrees,
    fact_plan: &FactPlan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut reported = Vec::new();
    for domain in program.domain_definitions() {
        let mut path = Vec::new();
        validate_domain_membership_cycle_from(
            program,
            fact_plan,
            domain.symbol,
            &mut path,
            &mut reported,
            diagnostics,
        );
    }
}

fn validate_domain_membership_cycle_from(
    program: &TypedTrees,
    fact_plan: &FactPlan,
    domain_symbol: SymbolHandle,
    path: &mut Vec<SymbolHandle>,
    reported: &mut Vec<SymbolHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !domain_symbol.is_valid() || reported.contains(&domain_symbol) {
        return;
    }

    if let Some(cycle_start) = path.iter().position(|symbol| *symbol == domain_symbol) {
        reported.push(domain_symbol);
        let cycle = path[cycle_start..]
            .iter()
            .copied()
            .chain(std::iter::once(domain_symbol))
            .filter_map(|symbol| domain_definition_by_symbol(program, symbol))
            .map(|domain| domain.name.to_string())
            .collect::<Vec<_>>()
            .join(" -> ");
        diagnostics.push(Diagnostic::error(format!(
            "domain membership cycle: {cycle}"
        )));
        return;
    }

    let Some(domain) = domain_definition_by_symbol(program, domain_symbol) else {
        return;
    };

    path.push(domain_symbol);
    for membership in domain_membership_facts(fact_plan, domain.symbol) {
        validate_domain_membership_cycle_from(
            program,
            fact_plan,
            membership.domain_symbol,
            path,
            reported,
            diagnostics,
        );
    }
    path.pop();
}

fn domain_membership_facts(
    fact_plan: &FactPlan,
    symbol: SymbolHandle,
) -> impl Iterator<Item = psi_facts::DomainMembershipFact> + '_ {
    fact_plan.domain_memberships_for_symbol(symbol)
}

fn domain_definition_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&psi_typed_trees::domain::DomainDefinition> {
    if !symbol.is_valid() {
        return None;
    }

    program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == symbol)
}
