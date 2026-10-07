//! Owner index construction: the reachable-expression sweep that populates
//! the environment index and the membership queries it answers.

use super::environments::{
    contract_owner_environment, environment_from_parameters, executable_site_environment,
    machine_environment,
};
use super::{Binding, ExecutableSite, InferredType, OwnerIndex, TypeEnvironment};
use checked_trees::CheckFacts;
/// Handle-keyed indexes and reach sets take the integer hasher.
type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;
type HashSet<T> = std::collections::HashSet<T, symbols::BuildSymbolHasher>;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateParameter;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// Build the whole-program exact-owner index in one pass, preserving the
/// collectors' per-expression ordering: contracts, domains, propositions,
/// measures, parameter constraints, rankings, then executable sites.
/// Build the whole-program exact-owner index in one pass, preserving the
/// collectors' per-expression ordering: contracts, domains, propositions,
/// measures, parameter constraints, rankings, then executable sites.
pub(super) fn build_owner_environment_index(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> OwnerIndex {
    let mut index = OwnerIndex::default();
    for (handle, contract) in facts.proof.contract_facts.iter() {
        let reached = proof_fact_handle_reachable_expressions(program, contract.fact);
        for expression in &reached {
            index
                .entries
                .entry(*expression)
                .or_default()
                .contract_facts
                .push(handle);
        }
        let Some(environment) = contract_owner_environment(program, contract.owner) else {
            continue;
        };
        index.add_all(reached, || environment);
    }
    for domain in program.domain_definitions() {
        let mut reached = HashSet::default();
        for fact in program.proof_facts(domain) {
            reached.extend(proof_fact_reachable_expressions(program, fact));
        }
        index.add_all(reached, || TypeEnvironment {
            bindings: vec![Binding {
                name: domain.subject_name.to_string(),
                symbol: domain.subject_symbol,
                type_reference: domain.target_type,
            }],
            ..Default::default()
        });
    }
    index_proposition_environments(program, &mut index);
    index_measure_environments(program, &mut index);
    index_parameter_constraint_environments(program, &mut index);
    index_ranking_environments(program, &mut index);
    index_executable_environments(program, &mut index);
    index
}

/// Every expression handle a proof fact's roots reach — the indexed shape of
/// `proof_fact_value_contains_expression`, which asks membership in exactly
/// this set.

/// Every expression handle a proof fact's roots reach — the indexed shape of
/// `proof_fact_value_contains_expression`, which asks membership in exactly
/// this set.
pub(super) fn proof_fact_reachable_expressions(
    program: &TypedTrees,
    fact: &typed_trees::domain::ProofFact,
) -> HashSet<ExpressionHandle> {
    let mut reached = HashSet::default();
    match fact {
        typed_trees::domain::ProofFact::Expression(root) => {
            reached.extend(
                crate::authored_selections::member_targets::reachable_expressions(program, *root),
            );
        }
        typed_trees::domain::ProofFact::Membership(membership) => {
            reached.extend(
                crate::authored_selections::member_targets::reachable_expressions(
                    program,
                    membership.value,
                ),
            );
        }
        typed_trees::domain::ProofFact::Proposition(application) => {
            for root in program
                .expression_table
                .expression_handles(application.arguments)
            {
                reached.extend(
                    crate::authored_selections::member_targets::reachable_expressions(
                        program, *root,
                    ),
                );
            }
        }
    }
    reached
}

pub(super) fn proof_fact_handle_reachable_expressions(
    program: &TypedTrees,
    fact: arena::Handle<typed_trees::domain::ProofFact>,
) -> HashSet<ExpressionHandle> {
    proof_fact_reachable_expressions(program, program.proof_facts.get(fact))
}

/// Every expression a type reference's embedded roots reach — the indexed
/// shape of `type_reference_contains_expression`, whose recursion this walk
/// mirrors arm for arm.

/// Every expression a type reference's embedded roots reach — the indexed
/// shape of `type_reference_contains_expression`, whose recursion this walk
/// mirrors arm for arm.
pub(super) fn type_reference_reachable_expressions(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> HashSet<ExpressionHandle> {
    let mut reached = HashSet::default();
    collect_type_reference_expressions(program, type_reference, &mut Vec::new(), &mut reached);
    reached
}

pub(super) fn collect_type_reference_expressions(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    visited: &mut Vec<TypeReferenceHandle>,
    reached: &mut HashSet<ExpressionHandle>,
) {
    if !type_reference.is_valid() || visited.contains(&type_reference) {
        return;
    }
    visited.push(type_reference);
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_type_reference_expressions(program, *referee, visited, reached)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            collect_type_reference_expressions(program, *base_type, visited, reached);
            for constraint in program.type_reference_table.constraints(*constraints) {
                match constraint {
                    TypeConstraintNode::Range {
                        minimum, maximum, ..
                    } => {
                        reached.extend(
                            crate::authored_selections::member_targets::reachable_expressions(
                                program, *minimum,
                            ),
                        );
                        reached.extend(
                            crate::authored_selections::member_targets::reachable_expressions(
                                program, *maximum,
                            ),
                        );
                    }
                    TypeConstraintNode::Domain(domain) => {
                        for argument in &domain.arguments {
                            collect_type_reference_expressions(
                                program, *argument, visited, reached,
                            );
                        }
                    }
                    TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {}
                }
            }
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            collect_type_reference_expressions(program, *element_type, visited, reached)
        }
        TypeReferenceNode::Generic { arguments, .. } => {
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                collect_type_reference_expressions(program, *argument, visited, reached);
            }
        }
        TypeReferenceNode::ConstExpression(root) => {
            reached.extend(
                crate::authored_selections::member_targets::reachable_expressions(program, *root),
            );
        }
        TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Unit => {}
    }
}

pub(super) fn index_proposition_environments(program: &TypedTrees, index: &mut OwnerIndex) {
    use typed_trees::proposition::{PropositionBody, PropositionFormula};

    for proposition in program.propositions() {
        let PropositionBody::Transparent { proposition: body } = &proposition.body else {
            continue;
        };
        let mut reached = HashSet::default();
        match body {
            PropositionFormula::Application(application) => {
                for root in program
                    .expression_table
                    .expression_handles(application.arguments)
                {
                    reached.extend(
                        crate::authored_selections::member_targets::reachable_expressions(
                            program, *root,
                        ),
                    );
                }
            }
            PropositionFormula::BooleanExpression(root) => {
                reached.extend(
                    crate::authored_selections::member_targets::reachable_expressions(
                        program, *root,
                    ),
                );
            }
        }
        index.add_all(reached, || {
            environment_from_parameters(
                program.proposition_parameters(proposition),
                TypeReferenceHandle::invalid(),
                None,
            )
        });
    }
}

pub(super) fn index_measure_environments(program: &TypedTrees, index: &mut OwnerIndex) {
    for measure in program.measures() {
        let Some(parameter) = measure.parameter.as_ref() else {
            continue;
        };
        let mut reached = HashSet::default();
        for root in program.expression_table.expression_handles(measure.body) {
            reached.extend(
                crate::authored_selections::member_targets::reachable_expressions(program, *root),
            );
        }
        index.add_all(reached, || {
            environment_from_parameters(std::slice::from_ref(parameter), measure.return_type, None)
        });
    }
}

pub(super) fn index_parameter_constraint_environments(
    program: &TypedTrees,
    index: &mut OwnerIndex,
) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let parameters = program.state_parameters(state);
            for (parameter_index, parameter) in parameters.iter().enumerate() {
                index.add_all(
                    type_reference_reachable_expressions(program, parameter.type_reference),
                    || TypeEnvironment {
                        self_type: machine
                            .attached_data_symbol
                            .is_valid()
                            .then_some(InferredType::Nominal(machine.attached_data_symbol)),
                        bindings: environment_from_parameters(
                            &parameters[..parameter_index],
                            TypeReferenceHandle::invalid(),
                            None,
                        )
                        .bindings,
                        ..Default::default()
                    },
                );
            }
        }
    }
    for definition in program.traits() {
        for signature in program.trait_machine_signatures(definition) {
            index_telescope_parameters(
                program,
                program.state_signature_parameters(signature),
                index,
            );
        }
    }
    for operator in program.operators().iter().chain(
        program
            .domain_definitions()
            .iter()
            .flat_map(|domain| program.domain_operators(domain)),
    ) {
        index_telescope_parameters(program, program.operator_parameters(operator), index);
    }
    for proposition in program.propositions() {
        index_telescope_parameters(program, program.proposition_parameters(proposition), index);
    }
}

pub(super) fn index_telescope_parameters(
    program: &TypedTrees,
    parameters: &[StateParameter],
    index: &mut OwnerIndex,
) {
    for (parameter_index, parameter) in parameters.iter().enumerate() {
        index.add_all(
            type_reference_reachable_expressions(program, parameter.type_reference),
            || {
                environment_from_parameters(
                    &parameters[..parameter_index],
                    TypeReferenceHandle::invalid(),
                    None,
                )
            },
        );
    }
}

pub(super) fn index_ranking_environments(program: &TypedTrees, index: &mut OwnerIndex) {
    for custody in &program.ranking_expression_custody {
        let Some(machine) = crate::lookup::machine_by_symbol(program, custody.machine) else {
            continue;
        };
        let mut reached = HashSet::default();
        let roots = custody
            .subjects
            .iter()
            .chain(&custody.view_arguments)
            .copied()
            .chain(custody.rank_range);
        for root in roots {
            reached.extend(
                crate::authored_selections::member_targets::reachable_expressions(program, root),
            );
        }
        index.add_all(reached, || {
            machine_environment(program, machine, program.machine_states(machine).first())
        });
    }
}

pub(super) fn index_executable_environments(program: &TypedTrees, index: &mut OwnerIndex) {
    // One buffer serves every statement; a fresh vector per statement spent
    // most of this walk growing it.
    let mut expressions = Vec::new();
    for (machine_index, machine) in program.machines().iter().enumerate() {
        for (state_index, state) in program.machine_states(machine).iter().enumerate() {
            let statements = program.statement_table.statements(state.statement_nodes);
            for (statement_index, statement) in statements.iter().enumerate() {
                expressions.clear();
                crate::monomorphization::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                if expressions.is_empty() {
                    continue;
                }
                let environment = index.intern(executable_site_environment(
                    program,
                    machine,
                    state,
                    statements,
                    statement_index,
                ));
                for &expression in &expressions {
                    index.add(expression, environment);
                    let entry = index.entries.entry(expression).or_default();
                    entry.executable_sites.push(ExecutableSite {
                        machine: machine_index,
                        state: state_index,
                        statement: statement_index,
                    });
                }
            }
        }
    }
}

pub(in crate::authored_selections) fn proof_fact_contains_expression(
    program: &TypedTrees,
    fact: arena::Handle<typed_trees::domain::ProofFact>,
    expression: ExpressionHandle,
) -> bool {
    proof_fact_value_contains_expression(program, program.proof_facts.get(fact), expression)
}

pub(super) fn proof_fact_value_contains_expression(
    program: &TypedTrees,
    fact: &typed_trees::domain::ProofFact,
    expression: ExpressionHandle,
) -> bool {
    match fact {
        typed_trees::domain::ProofFact::Expression(root) => {
            crate::authored_selections::member_targets::expression_contains(
                program, *root, expression,
            )
        }
        typed_trees::domain::ProofFact::Membership(membership) => {
            crate::authored_selections::member_targets::expression_contains(
                program,
                membership.value,
                expression,
            )
        }
        typed_trees::domain::ProofFact::Proposition(application) => program
            .expression_table
            .expression_handles(application.arguments)
            .iter()
            .any(|root| {
                crate::authored_selections::member_targets::expression_contains(
                    program, *root, expression,
                )
            }),
    }
}
