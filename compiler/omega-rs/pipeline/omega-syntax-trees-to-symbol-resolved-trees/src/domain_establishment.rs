use omega_core::semantics::{DomainEstablishmentRoute, DomainPredicateBody, MachineSupplyMode};
use omega_core::symbols::SymbolHandle;
use omega_symbol_resolved_trees::SymbolResolvedTrees;
use omega_symbol_resolved_trees::domain::ProofFact;
use omega_symbol_resolved_trees::expression::ExpressionNode;
use omega_symbol_resolved_trees::signature::{SignatureContract, SignatureContractKind};
use omega_symbol_resolved_trees::types::TypeReference;

/// Normalize authored domain-introduction relationships after every
/// declaration and contract fact has a symbol.
///
/// This is the sole projection point for the currently landed route sources.
/// Checked consumers consult these identities instead of reconstructing owner
/// authority from attachment names or contract placement.
pub(crate) fn normalize_domain_establishment_routes(program: &mut SymbolResolvedTrees) {
    let mut additions = Vec::new();
    collect_owner_machine_routes(program, &mut additions);
    collect_owner_operator_routes(program, &mut additions);
    collect_boundary_requirement_routes(program, &mut additions);

    program.domain_definitions.for_each_mut(|domain| {
        domain.establishment_routes.clear();
        for (domain_symbol, route) in &additions {
            if *domain_symbol == domain.symbol && !domain.establishment_routes.contains(route) {
                domain.establishment_routes.push(*route);
            }
        }
    });
}

fn collect_owner_machine_routes(
    program: &SymbolResolvedTrees,
    additions: &mut Vec<(SymbolHandle, DomainEstablishmentRoute)>,
) {
    for machine in program
        .machines
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
    {
        let Some(attached) = machine.attached_data.as_ref() else {
            continue;
        };
        let route = DomainEstablishmentRoute::OwnerCheckedMachine {
            machine: machine.symbol,
        };
        collect_owner_contract_routes(
            program,
            attached.as_str(),
            program.machine_contracts(machine),
            route,
            additions,
        );
        for state_handle in program.machine_state_handles(machine.states) {
            let state = program.machine_state(*state_handle);
            collect_owner_contract_routes(
                program,
                attached.as_str(),
                program.signature_contracts(state.contracts),
                route,
                additions,
            );
        }
    }
}

fn collect_owner_contract_routes(
    program: &SymbolResolvedTrees,
    attached: &str,
    contracts: &[SignatureContract],
    route: DomainEstablishmentRoute,
    additions: &mut Vec<(SymbolHandle, DomainEstablishmentRoute)>,
) {
    for domain_symbol in ensured_domain_symbols(program, contracts, false) {
        let Some(domain) = domain_definition(program, domain_symbol) else {
            continue;
        };
        if domain.predicate_body != DomainPredicateBody::Bodyless {
            continue;
        }
        if named_carrier(program, &domain.target_type)
            .is_some_and(|carrier| same_semantic_name(attached, carrier))
        {
            additions.push((domain_symbol, route));
        }
    }
}

fn collect_owner_operator_routes(
    program: &SymbolResolvedTrees,
    additions: &mut Vec<(SymbolHandle, DomainEstablishmentRoute)>,
) {
    for domain in &program.domain_definitions {
        if domain.predicate_body != DomainPredicateBody::Bodyless {
            continue;
        }
        for operator in program.operator_definitions(domain.operators) {
            let route = DomainEstablishmentRoute::OwnerOperator {
                operator: operator.symbol,
            };
            if ensured_domain_symbols(
                program,
                program.signature_contracts(operator.contracts),
                false,
            )
            .contains(&domain.symbol)
            {
                additions.push((domain.symbol, route));
            }
        }
    }
}

fn collect_boundary_requirement_routes(
    program: &SymbolResolvedTrees,
    additions: &mut Vec<(SymbolHandle, DomainEstablishmentRoute)>,
) {
    for trait_definition in program
        .traits
        .iter()
        .filter(|definition| definition.is_boundary)
    {
        for signature in program.trait_machine_signatures(trait_definition.machines) {
            let route = DomainEstablishmentRoute::BoundaryRequirement {
                boundary_trait: trait_definition.symbol,
                requirement: signature.symbol,
            };
            for domain_symbol in ensured_domain_symbols(
                program,
                program.signature_contracts(signature.contracts),
                true,
            ) {
                additions.push((domain_symbol, route));
            }
        }
    }
}

fn ensured_domain_symbols(
    program: &SymbolResolvedTrees,
    contracts: &[SignatureContract],
    require_bare_result: bool,
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
            if require_bare_result && !expression_is_bare_result(program, membership.value) {
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
) -> Option<&omega_symbol_resolved_trees::domain::DomainDefinition> {
    program
        .domain_definitions
        .iter()
        .find(|domain| domain.symbol == symbol)
}

fn expression_is_bare_result(
    program: &SymbolResolvedTrees,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
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

fn named_carrier<'program>(
    program: &'program SymbolResolvedTrees,
    type_reference: &'program TypeReference,
) -> Option<&'program str> {
    match type_reference {
        TypeReference::Named { name, .. } => Some(name.as_str()),
        TypeReference::Reference(reference) => {
            named_carrier(program, program.child_type_reference(reference.referee))
        }
        TypeReference::Constrained(constrained) => {
            named_carrier(program, program.child_type_reference(constrained.base_type))
        }
        _ => None,
    }
}

fn same_semantic_name(left: &str, right: &str) -> bool {
    left == right
        || (!left.contains("::") && right.rsplit("::").next().is_some_and(|leaf| leaf == left))
        || (!right.contains("::") && left.rsplit("::").next().is_some_and(|leaf| leaf == right))
}
