use crate::proof_facts::{ProofFactOwner, validate_domain_fact_payloads};
use crate::symbols::TopLevelSymbols;
use crate::{
    TypeReferenceOwner, type_reference_label, type_references_match, validate_type_reference_handle,
};
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_facts::FactPlan;
use omega_typed_trees::TypedTrees;

pub(crate) fn validate_domain_definitions(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    fact_plan: &FactPlan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for domain in program.domain_definitions() {
        validate_type_reference_handle(
            program,
            domain.target_type,
            symbols,
            diagnostics,
            TypeReferenceOwner::DomainTarget {
                domain: domain.name.as_str(),
                generic_depth: 0,
            },
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
}

fn validate_domain_membership_targets(
    program: &TypedTrees,
    fact_plan: &FactPlan,
    domain: &omega_typed_trees::domain::DomainDefinition,
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
) -> impl Iterator<Item = omega_facts::DomainMembershipFact> + '_ {
    fact_plan.domain_memberships_for_symbol(symbol)
}

fn domain_definition_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&omega_typed_trees::domain::DomainDefinition> {
    if !symbol.is_valid() {
        return None;
    }

    program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == symbol)
}
