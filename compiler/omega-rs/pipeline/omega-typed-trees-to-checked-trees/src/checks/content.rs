//! Content-bearing signature conservation gates.
//!
//! A borrow lends access; it never supplies an owned claim that can survive
//! the call. This first P1c consumer rejects the exact retained-custody shape
//! where a content-bearing result has compatible content-bearing inputs, but
//! every compatible source is borrowed. It deliberately keys compatibility by
//! the retained compiler-owned algebra identity, never carrier or operation
//! names.

use omega_checked_trees::CheckFacts;
use omega_core::content::ContentProjectionPlan;
use omega_core::diagnostics::Diagnostic;
use omega_core::semantics::Multiplicity;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::domain::ProofFact;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use omega_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn check_retained_content_custody(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    if facts.qualifications.content.plans.is_empty() {
        return Ok(());
    }

    let mut diagnostics = Vec::new();

    for trait_definition in program.traits() {
        for signature in program.trait_machine_signatures(trait_definition) {
            let contracts = program
                .state_signature_contracts(signature)
                .iter()
                .collect::<Vec<_>>();
            check_callable(
                program,
                facts,
                &format!("{}::{}", trait_definition.name, signature.name),
                program.state_signature_parameters(signature),
                signature.return_type,
                &contracts,
                &mut diagnostics,
            );
        }
    }

    for machine in program.machines() {
        for (state_index, state) in program.machine_states(machine).iter().enumerate() {
            let mut contracts = program.state_contracts(state).iter().collect::<Vec<_>>();
            if state_index == 0 {
                contracts.extend(program.machine_contracts(machine));
            }
            let label = if state_index == 0 {
                machine.name.to_string()
            } else {
                format!("{}::{}", machine.name, state.name)
            };
            check_callable(
                program,
                facts,
                &label,
                program.state_parameters(state),
                state.return_type,
                &contracts,
                &mut diagnostics,
            );
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

#[allow(clippy::too_many_arguments)]
fn check_callable(
    program: &TypedTrees,
    facts: &CheckFacts,
    label: &str,
    parameters: &[StateParameter],
    return_type: TypeReferenceHandle,
    contracts: &[&SignatureContract],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut result_domains = Vec::new();
    append_type_domains(program, return_type, &mut result_domains);
    for contract in contracts
        .iter()
        .filter(|contract| contract.kind == SignatureContractKind::Ensures)
    {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Membership(membership) = fact else {
                continue;
            };
            if expression_is_bare_result(program, membership.value) {
                push_unique(&mut result_domains, membership.domain_symbol);
            }
        }
    }

    for result_domain in result_domains {
        let Some(result_plan) = facts.qualifications.content.for_domain(result_domain) else {
            continue;
        };
        let mut borrowed_sources = Vec::new();
        let mut has_owned_source = false;

        for parameter in parameters {
            let mut parameter_domains = Vec::new();
            append_type_domains(program, parameter.type_reference, &mut parameter_domains);
            for contract in contracts
                .iter()
                .filter(|contract| contract.kind == SignatureContractKind::Requires)
            {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    let ProofFact::Membership(membership) = fact else {
                        continue;
                    };
                    if expression_names_parameter(program, membership.value, parameter) {
                        push_unique(&mut parameter_domains, membership.domain_symbol);
                    }
                }
            }

            let compatible = parameter_domains.iter().any(|domain| {
                facts
                    .qualifications
                    .content
                    .for_domain(*domain)
                    .is_some_and(|input_plan| compatible_content(input_plan, result_plan))
            });
            if !compatible {
                continue;
            }

            if type_contains_reference(program, parameter.type_reference) {
                borrowed_sources.push(parameter.name.as_str());
            } else if program.type_multiplicity(parameter.type_reference) == Multiplicity::Linear {
                has_owned_source = true;
            }
        }

        if borrowed_sources.is_empty() || has_owned_source {
            continue;
        }

        let result_name = domain_name(program, result_domain);
        let borrowed = borrowed_sources
            .iter()
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", ");
        diagnostics.push(Diagnostic::error(format!(
            "callable `{label}` returns content-bearing custody `{result_name}` sourced only from borrowed parameter{} {borrowed}; retained-after-return authority requires a consumed owned input",
            if borrowed_sources.len() == 1 { "" } else { "s" },
        )));
    }
}

fn compatible_content(left: &ContentProjectionPlan, right: &ContentProjectionPlan) -> bool {
    left.algebra == right.algebra
}

fn append_type_domains(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    domains: &mut Vec<SymbolHandle>,
) {
    if !type_reference.is_valid() {
        return;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            append_type_domains(program, *referee, domains);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            append_type_domains(program, *base_type, domains);
            for constraint in program.type_reference_table.constraints(*constraints) {
                if let TypeConstraintNode::Domain(domain) = constraint {
                    push_unique(domains, domain.symbol);
                }
            }
        }
        _ => {}
    }
}

fn type_contains_reference(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    if !type_reference.is_valid() {
        return false;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { .. } => true,
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_contains_reference(program, *base_type)
        }
        _ => false,
    }
}

fn expression_is_bare_result(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    matches!(program.expression_table.name_path_members(path.members), [name] if name.as_str() == "result")
}

fn expression_names_parameter(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameter: &StateParameter,
) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    matches!(program.expression_table.name_path_members(path.members), [name]
        if path.symbol == parameter.symbol || name.as_str() == parameter.name.as_str())
}

fn push_unique(domains: &mut Vec<SymbolHandle>, domain: SymbolHandle) {
    if domain.is_valid() && !domains.contains(&domain) {
        domains.push(domain);
    }
}

fn domain_name(program: &TypedTrees, symbol: SymbolHandle) -> &str {
    program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == symbol)
        .map(|domain| domain.name.as_str())
        .unwrap_or("<unknown domain>")
}
