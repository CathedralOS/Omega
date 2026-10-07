//! Environment construction: the parameter-bound type environments an
//! owner, contract, machine, signature, or proof-fact vocabulary supplies.

use super::indexing::{proof_fact_contains_expression, proof_fact_value_contains_expression};
use super::inference::type_reference_contains_expression;
use super::{Binding, InferredType, OwnerEnvironmentIndex, TypeEnvironment};
use checked_trees::{CheckFacts, ContractProofFactOwner};
/// Handle-keyed indexes and reach sets take the integer hasher.
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateParameter;
use typed_trees::types::TypeReferenceHandle;

/// The environment one statement position establishes: the machine's own
/// environment extended by the `LocalData` bindings preceding the statement.
pub(super) fn executable_site_environment(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statements: &[typed_trees::statement::StatementNode],
    statement_index: usize,
) -> TypeEnvironment {
    let mut environment = machine_environment(program, machine, Some(state));
    environment.bindings.extend(
        statements
            .iter()
            .take(statement_index)
            .filter_map(|statement| match statement {
                typed_trees::statement::StatementNode::LocalData(local)
                    if local.type_reference.is_valid() =>
                {
                    Some(Binding {
                        name: local.name.as_str().to_owned(),
                        symbol: local.symbol,
                        type_reference: local.type_reference,
                    })
                }
                _ => None,
            }),
    );
    environment
}

pub(super) fn contract_owner_environment(
    program: &TypedTrees,
    owner: ContractProofFactOwner,
) -> Option<TypeEnvironment> {
    match owner {
        ContractProofFactOwner::Machine { machine_symbol } => {
            let machine = crate::lookup::machine_by_symbol(program, machine_symbol)?;
            let entry = program.machine_states(machine).first();
            Some(machine_environment(program, machine, entry))
        }
        ContractProofFactOwner::MachineState {
            machine_symbol,
            state_symbol,
        } => {
            let machine = crate::lookup::machine_by_symbol(program, machine_symbol)?;
            let state = program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == state_symbol)?;
            Some(machine_environment(program, machine, Some(state)))
        }
        ContractProofFactOwner::StateSignature {
            owner_symbol,
            state_symbol,
        } => signature_environment(program, owner_symbol, state_symbol),
        ContractProofFactOwner::OperatorDeclaration { operator_symbol } => {
            let operator = typed_trees::operator::declaration_by_symbol(program, operator_symbol)?;
            Some(environment_from_parameters(
                program.operator_parameters(operator),
                operator.return_type,
                None,
            ))
        }
        ContractProofFactOwner::OperatorUse {
            operator_symbol, ..
        } => {
            let operator = typed_trees::operator::declaration_by_symbol(program, operator_symbol)?;
            Some(environment_from_parameters(
                program.operator_parameters(operator),
                operator.return_type,
                None,
            ))
        }
        ContractProofFactOwner::Unknown => None,
    }
}

pub(super) fn machine_environment(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
) -> TypeEnvironment {
    let parameters = state.map_or(&[][..], |state| program.state_parameters(state));
    let result_type =
        state.and_then(|state| state.return_type.is_valid().then_some(state.return_type));
    let self_type = machine
        .attached_data_symbol
        .is_valid()
        .then_some(InferredType::Nominal(machine.attached_data_symbol));
    environment_from_parameters(
        parameters,
        result_type.unwrap_or_else(TypeReferenceHandle::invalid),
        self_type,
    )
}

pub(super) fn signature_environment(
    program: &TypedTrees,
    owner_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Option<TypeEnvironment> {
    if let Some(signature) = program.traits().iter().find_map(|definition| {
        (definition.symbol == owner_symbol)
            .then(|| {
                program
                    .trait_machine_signatures(definition)
                    .iter()
                    .find(|signature| signature.symbol == state_symbol)
            })
            .flatten()
    }) {
        return Some(environment_from_parameters(
            program.state_signature_parameters(signature),
            signature.return_type,
            None,
        ));
    }
    let (_, signature) = program.machine_parameter_signature(state_symbol)?;
    Some(environment_from_parameters(
        program.state_signature_parameters(signature),
        signature.return_type,
        None,
    ))
}

pub(super) fn environment_from_parameters(
    parameters: &[StateParameter],
    result_type: TypeReferenceHandle,
    self_type: Option<InferredType>,
) -> TypeEnvironment {
    TypeEnvironment {
        self_type,
        result_type: result_type.is_valid().then_some(result_type),
        bindings: parameters
            .iter()
            .map(|parameter| Binding {
                name: parameter.name.as_str().to_owned(),
                symbol: parameter.symbol,
                type_reference: parameter.type_reference,
            })
            .collect(),
    }
}

pub(super) fn exact_owner_environments(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: ExpressionHandle,
    owner_index: Option<&OwnerEnvironmentIndex>,
) -> Option<Vec<TypeEnvironment>> {
    if let Some(owner_index) = owner_index {
        return owner_index.environments(program, facts, expression);
    }
    let mut environments = Vec::new();
    for (_, contract) in facts.proof.contract_facts.iter() {
        if !proof_fact_contains_expression(program, contract.fact, expression) {
            continue;
        }
        if let Some(environment) = contract_owner_environment(program, contract.owner) {
            environments.push(environment);
        }
    }
    for domain in program.domain_definitions() {
        if program
            .proof_facts(domain)
            .iter()
            .any(|fact| proof_fact_value_contains_expression(program, fact, expression))
        {
            environments.push(TypeEnvironment {
                bindings: vec![Binding {
                    name: domain.subject_name.to_string(),
                    symbol: domain.subject_symbol,
                    type_reference: domain.target_type,
                }],
                ..Default::default()
            });
        }
    }
    collect_proposition_environments(program, expression, &mut environments);
    collect_measure_environments(program, expression, &mut environments);
    collect_parameter_constraint_environments(program, expression, &mut environments)?;
    collect_ranking_environments(program, expression, &mut environments);
    collect_executable_environments(program, expression, &mut environments)?;
    (!environments.is_empty()).then_some(environments)
}

pub(super) fn collect_measure_environments(
    program: &TypedTrees,
    expression: ExpressionHandle,
    environments: &mut Vec<TypeEnvironment>,
) {
    for measure in program.measures() {
        if !program
            .expression_table
            .expression_handles(measure.body)
            .iter()
            .any(|root| {
                crate::authored_selections::member_targets::expression_contains(
                    program, *root, expression,
                )
            })
        {
            continue;
        }
        let Some(parameter) = measure.parameter.as_ref() else {
            continue;
        };
        environments.push(environment_from_parameters(
            std::slice::from_ref(parameter),
            measure.return_type,
            None,
        ));
    }
}

pub(super) fn collect_proposition_environments(
    program: &TypedTrees,
    expression: ExpressionHandle,
    environments: &mut Vec<TypeEnvironment>,
) {
    use typed_trees::proposition::{PropositionBody, PropositionFormula};

    for proposition in program.propositions() {
        let PropositionBody::Transparent { proposition: body } = &proposition.body else {
            continue;
        };
        let contains = match body {
            PropositionFormula::Application(application) => program
                .expression_table
                .expression_handles(application.arguments)
                .iter()
                .any(|root| {
                    crate::authored_selections::member_targets::expression_contains(
                        program, *root, expression,
                    )
                }),
            PropositionFormula::BooleanExpression(root) => {
                crate::authored_selections::member_targets::expression_contains(
                    program, *root, expression,
                )
            }
        };
        if contains {
            environments.push(environment_from_parameters(
                program.proposition_parameters(proposition),
                TypeReferenceHandle::invalid(),
                None,
            ));
        }
    }
}

pub(super) fn collect_ranking_environments(
    program: &TypedTrees,
    expression: ExpressionHandle,
    environments: &mut Vec<TypeEnvironment>,
) {
    for custody in &program.ranking_expression_custody {
        let roots = custody
            .subjects
            .iter()
            .chain(&custody.view_arguments)
            .copied()
            .chain(custody.rank_range);
        if !roots.into_iter().any(|root| {
            crate::authored_selections::member_targets::expression_contains(
                program, root, expression,
            )
        }) {
            continue;
        }
        let Some(machine) = crate::lookup::machine_by_symbol(program, custody.machine) else {
            continue;
        };
        environments.push(machine_environment(
            program,
            machine,
            program.machine_states(machine).first(),
        ));
    }
}

pub(super) fn collect_parameter_constraint_environments(
    program: &TypedTrees,
    expression: ExpressionHandle,
    environments: &mut Vec<TypeEnvironment>,
) -> Option<()> {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let parameters = program.state_parameters(state);
            for (index, parameter) in parameters.iter().enumerate() {
                if !type_reference_contains_expression(
                    program,
                    parameter.type_reference,
                    expression,
                    &mut Vec::new(),
                ) {
                    continue;
                }
                let environment = TypeEnvironment {
                    self_type: machine
                        .attached_data_symbol
                        .is_valid()
                        .then_some(InferredType::Nominal(machine.attached_data_symbol)),
                    bindings: environment_from_parameters(
                        &parameters[..index],
                        TypeReferenceHandle::invalid(),
                        None,
                    )
                    .bindings,
                    ..Default::default()
                };
                environments.push(environment);
            }
        }
    }

    for definition in program.traits() {
        for signature in program.trait_machine_signatures(definition) {
            collect_telescope_constraint_environments(
                program,
                program.state_signature_parameters(signature),
                expression,
                environments,
            )?;
        }
    }
    for operator in program.operators().iter().chain(
        program
            .domain_definitions()
            .iter()
            .flat_map(|domain| program.domain_operators(domain)),
    ) {
        collect_telescope_constraint_environments(
            program,
            program.operator_parameters(operator),
            expression,
            environments,
        )?;
    }
    for proposition in program.propositions() {
        collect_telescope_constraint_environments(
            program,
            program.proposition_parameters(proposition),
            expression,
            environments,
        )?;
    }
    Some(())
}

pub(super) fn collect_telescope_constraint_environments(
    program: &TypedTrees,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
    environments: &mut Vec<TypeEnvironment>,
) -> Option<()> {
    for (index, parameter) in parameters.iter().enumerate() {
        if !type_reference_contains_expression(
            program,
            parameter.type_reference,
            expression,
            &mut Vec::new(),
        ) {
            continue;
        }
        let environment =
            environment_from_parameters(&parameters[..index], TypeReferenceHandle::invalid(), None);
        environments.push(environment);
    }
    Some(())
}

pub(super) fn collect_executable_environments(
    program: &TypedTrees,
    expression: ExpressionHandle,
    environments: &mut Vec<TypeEnvironment>,
) -> Option<()> {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let statements = program.statement_table.statements(state.statement_nodes);
            for (statement_index, statement) in statements.iter().enumerate() {
                let mut expressions = Vec::new();
                crate::monomorphization::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                if !expressions.contains(&expression) {
                    continue;
                }
                environments.push(executable_site_environment(
                    program,
                    machine,
                    state,
                    statements,
                    statement_index,
                ));
            }
        }
    }
    Some(())
}
