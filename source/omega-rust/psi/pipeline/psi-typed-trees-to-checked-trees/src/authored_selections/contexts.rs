use psi_checked_trees::{CheckFacts, ContractProofFactOwner};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableMemberExpression};
use psi_typed_trees::signature::StateParameter;
use psi_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OwnerMemberTarget {
    Declaration(SymbolHandle),
    CollectionLength,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InferredType {
    TypeReference(TypeReferenceHandle),
    Nominal(SymbolHandle),
    CompilerString,
}

#[derive(Debug, Clone)]
struct Binding {
    name: String,
    symbol: SymbolHandle,
    type_reference: TypeReferenceHandle,
}

#[derive(Debug, Clone, Default)]
struct TypeEnvironment {
    self_type: Option<InferredType>,
    result_type: Option<TypeReferenceHandle>,
    bindings: Vec<Binding>,
}

pub(super) fn checked_member_target_from_exact_owner(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: ExpressionHandle,
    member: &TableMemberExpression,
) -> Option<OwnerMemberTarget> {
    let mut target = None;
    for environment in exact_owner_environments(program, facts, expression)? {
        let candidate = member_target_in_environment(program, member, &environment)?;
        retain_consistent(&mut target, candidate)?;
    }
    target
}

pub(super) fn checked_collection_view_intrinsic_from_exact_owner(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: ExpressionHandle,
    call: &psi_typed_trees::expression::TableCallExpression,
) -> Option<psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic> {
    use psi_language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionIntrinsic as Intrinsic, CollectionViewOperation,
    };

    if call.target_symbol.is_valid()
        || !call.receiver.is_valid()
        || !program
            .expression_table
            .expression_handles(call.arguments)
            .is_empty()
    {
        return None;
    }

    let expected = match call.target.as_str() {
        "as_slice" => CollectionViewOperation::SharedSlice,
        "as_mut_slice" => CollectionViewOperation::MutableSlice,
        "as_view" => CollectionViewOperation::TextView,
        "bytes" => CollectionViewOperation::Bytes,
        _ => return None,
    };
    let mut retained = None;
    for environment in exact_owner_environments(program, facts, expression)? {
        let receiver =
            infer_expression_type(program, call.receiver, &environment, &mut Vec::new())?;
        let candidate = collection_view_operation_for_receiver(program, expected, receiver)?;
        if retained.is_some_and(|operation| operation != candidate) {
            return None;
        }
        retained = Some(candidate);
    }
    retained.map(|operation| Intrinsic::CollectionView(operation))
}

pub(super) fn checked_machine_call_target_from_exact_owner(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: ExpressionHandle,
    call: &psi_typed_trees::expression::TableCallExpression,
) -> Option<SymbolHandle> {
    checked_machine_call_target_from_type_owners(program, facts, expression, call)
        .or_else(|| checked_machine_call_target_from_executable_owner(program, expression, call))
}

fn checked_machine_call_target_from_type_owners(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: ExpressionHandle,
    call: &psi_typed_trees::expression::TableCallExpression,
) -> Option<SymbolHandle> {
    let mut target = None;
    for environment in exact_owner_environments(program, facts, expression)? {
        let candidate = call_target_in_environment(program, call, &environment)?;
        retain_consistent(&mut target, candidate)?;
    }
    target
}

fn checked_machine_call_target_from_executable_owner(
    program: &TypedTrees,
    expression: ExpressionHandle,
    call: &psi_typed_trees::expression::TableCallExpression,
) -> Option<SymbolHandle> {
    let mut target = None;
    for machine in program.machines() {
        if program
            .machine_specializations
            .iter()
            .any(|specialization| {
                specialization.instance == machine.symbol
                    && specialization.template != specialization.instance
            })
        {
            continue;
        }
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
                let (receiver_symbol, receiver_path) =
                    crate::lookup::call_receiver_parts(program, call.receiver);
                let mut candidate = crate::lookup::resolve_state_call_target(
                    program,
                    machine,
                    state,
                    receiver_symbol,
                    call.target_symbol,
                    receiver_path.as_deref(),
                    &call.target,
                );
                if !candidate.is_valid() && !call.receiver.is_valid() {
                    let matching = program
                        .machine_type_parameters(machine)
                        .iter()
                        .filter(|parameter| {
                            parameter.name == call.target
                                && matches!(
                                    parameter.kind,
                                    psi_typed_trees::data::TypeParameterKind::Machine { .. }
                                )
                        })
                        .map(|parameter| parameter.symbol)
                        .collect::<Vec<_>>();
                    if let [selected] = matching.as_slice() {
                        candidate = *selected;
                    }
                }
                if !candidate.is_valid() && call.receiver.is_valid() {
                    let mut environment = machine_environment(program, machine, Some(state));
                    environment.bindings.extend(
                        statements
                            .iter()
                            .take(statement_index)
                            .filter_map(|statement| match statement {
                                psi_typed_trees::statement::StatementNode::LocalData(local)
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
                    if let Some(receiver_type) =
                        infer_expression_type(program, call.receiver, &environment, &mut Vec::new())
                    {
                        candidate =
                            attached_machine_target(program, receiver_type, call.target.as_str())
                                .unwrap_or_else(SymbolHandle::invalid);
                    }
                }
                if !candidate.is_valid() || target.is_some_and(|retained| retained != candidate) {
                    return None;
                }
                target = Some(candidate);
            }
        }
    }
    target
}

fn call_target_in_environment(
    program: &TypedTrees,
    call: &psi_typed_trees::expression::TableCallExpression,
    environment: &TypeEnvironment,
) -> Option<SymbolHandle> {
    if !call.receiver.is_valid() {
        return None;
    }
    let receiver_type = exact_static_data_receiver(program, call.receiver, environment)
        .or_else(|| infer_expression_type(program, call.receiver, environment, &mut Vec::new()))?;
    attached_machine_target(program, receiver_type, call.target.as_str())
}

fn exact_static_data_receiver(
    program: &TypedTrees,
    receiver: ExpressionHandle,
    environment: &TypeEnvironment,
) -> Option<InferredType> {
    let ExpressionNode::Name(path) = program.expression_table.expression(receiver) else {
        return None;
    };
    let members = program.expression_table.name_path_members(path.members);
    let selected = crate::lookup::resolve_name_path_member_symbol(
        program,
        path,
        members.len().checked_sub(1)?,
    );
    if selected.is_valid() && program.symbols.get(selected).kind == psi_symbols::SymbolKind::Data {
        return Some(InferredType::Nominal(selected));
    }

    // Proof-owned static receivers can remain intentionally unresolved until
    // checked finalization. Rejoin the authored qualifier only to nominal
    // identities supplied by this exact owner environment. The text narrows
    // that closed identity set; it never performs a program-wide name lookup.
    let authored_path = members
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let mut retained = None;
    for inferred in environment
        .self_type
        .into_iter()
        .chain(environment.result_type.map(InferredType::TypeReference))
        .chain(
            environment
                .bindings
                .iter()
                .map(|binding| InferredType::TypeReference(binding.type_reference)),
        )
    {
        let Some(candidate) = inferred_nominal_symbol(program, inferred) else {
            continue;
        };
        if program.symbols.get(candidate).kind != psi_symbols::SymbolKind::Data
            || program.symbols.display_path(candidate, "::") != authored_path
        {
            continue;
        }
        retain_consistent(&mut retained, candidate)?;
    }
    retained.map(InferredType::Nominal)
}

fn attached_machine_target(
    program: &TypedTrees,
    receiver_type: InferredType,
    target_name: &str,
) -> Option<SymbolHandle> {
    let attached_data = inferred_nominal_symbol(program, receiver_type)?;
    let mut candidates = program
        .machines()
        .iter()
        .filter(|machine| {
            !program
                .machine_specializations
                .iter()
                .any(|specialization| {
                    specialization.instance == machine.symbol
                        && specialization.template != specialization.instance
                })
        })
        .filter(|machine| machine.attached_data_symbol == attached_data)
        .flat_map(|machine| program.machine_states(machine))
        .filter(|state| state.name.as_str() == target_name)
        .map(|state| state.symbol);
    let selected = candidates.next()?;
    candidates.next().is_none().then_some(selected)
}

fn retain_consistent<T: Copy + PartialEq>(retained: &mut Option<T>, candidate: T) -> Option<()> {
    if retained.is_some_and(|target| target != candidate) {
        return None;
    }
    *retained = Some(candidate);
    Some(())
}

fn contract_owner_environment(
    program: &TypedTrees,
    owner: ContractProofFactOwner,
) -> Option<TypeEnvironment> {
    match owner {
        ContractProofFactOwner::Machine { machine_symbol } => {
            let machine = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == machine_symbol)?;
            let entry = program.machine_states(machine).first();
            Some(machine_environment(program, machine, entry))
        }
        ContractProofFactOwner::MachineState {
            machine_symbol,
            state_symbol,
        } => {
            let machine = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == machine_symbol)?;
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
            let operator =
                psi_typed_trees::operator::declaration_by_symbol(program, operator_symbol)?;
            Some(environment_from_parameters(
                program.operator_parameters(operator),
                operator.return_type,
                None,
            ))
        }
        ContractProofFactOwner::OperatorUse {
            operator_symbol, ..
        } => {
            let operator =
                psi_typed_trees::operator::declaration_by_symbol(program, operator_symbol)?;
            Some(environment_from_parameters(
                program.operator_parameters(operator),
                operator.return_type,
                None,
            ))
        }
        ContractProofFactOwner::Unknown => None,
    }
}

fn machine_environment(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
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

fn signature_environment(
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

fn environment_from_parameters(
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

fn proof_fact_contains_expression(
    program: &TypedTrees,
    fact: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    expression: ExpressionHandle,
) -> bool {
    proof_fact_value_contains_expression(program, program.proof_facts.get(fact), expression)
}

fn proof_fact_value_contains_expression(
    program: &TypedTrees,
    fact: &psi_typed_trees::domain::ProofFact,
    expression: ExpressionHandle,
) -> bool {
    match fact {
        psi_typed_trees::domain::ProofFact::Expression(root) => {
            super::expression_contains(program, *root, expression, &mut Vec::new())
        }
        psi_typed_trees::domain::ProofFact::Membership(membership) => {
            super::expression_contains(program, membership.value, expression, &mut Vec::new())
        }
        psi_typed_trees::domain::ProofFact::Proposition(application) => program
            .expression_table
            .expression_handles(application.arguments)
            .iter()
            .any(|root| super::expression_contains(program, *root, expression, &mut Vec::new())),
    }
}

fn exact_owner_environments(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: ExpressionHandle,
) -> Option<Vec<TypeEnvironment>> {
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
                self_type: Some(InferredType::TypeReference(domain.target_type)),
                ..Default::default()
            });
        }
    }
    collect_proposition_environments(program, expression, &mut environments);
    collect_parameter_constraint_environments(program, expression, &mut environments)?;
    collect_ranking_environments(program, expression, &mut environments);
    collect_executable_environments(program, expression, &mut environments)?;
    (!environments.is_empty()).then_some(environments)
}

fn collect_proposition_environments(
    program: &TypedTrees,
    expression: ExpressionHandle,
    environments: &mut Vec<TypeEnvironment>,
) {
    use psi_typed_trees::proposition::{PropositionBody, PropositionFormula};

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
                    super::expression_contains(program, *root, expression, &mut Vec::new())
                }),
            PropositionFormula::BooleanExpression(root) => {
                super::expression_contains(program, *root, expression, &mut Vec::new())
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

fn collect_ranking_environments(
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
        if !roots
            .into_iter()
            .any(|root| super::expression_contains(program, root, expression, &mut Vec::new()))
        {
            continue;
        }
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == custody.machine)
        else {
            continue;
        };
        environments.push(machine_environment(
            program,
            machine,
            program.machine_states(machine).first(),
        ));
    }
}

fn collect_parameter_constraint_environments(
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

fn collect_telescope_constraint_environments(
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

fn collect_executable_environments(
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
                let mut environment = machine_environment(program, machine, Some(state));
                environment
                    .bindings
                    .extend(
                        statements
                            .iter()
                            .take(statement_index)
                            .filter_map(|statement| match statement {
                                psi_typed_trees::statement::StatementNode::LocalData(local)
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
                environments.push(environment);
            }
        }
    }
    Some(())
}

fn type_reference_contains_expression(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    expression: ExpressionHandle,
    visited: &mut Vec<TypeReferenceHandle>,
) -> bool {
    if !type_reference.is_valid() || visited.contains(&type_reference) {
        return false;
    }
    visited.push(type_reference);
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            type_reference_contains_expression(program, *referee, expression, visited)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            type_reference_contains_expression(program, *base_type, expression, visited)
                || program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| match constraint {
                        TypeConstraintNode::Range { minimum, maximum } => {
                            super::expression_contains(
                                program,
                                *minimum,
                                expression,
                                &mut Vec::new(),
                            ) || super::expression_contains(
                                program,
                                *maximum,
                                expression,
                                &mut Vec::new(),
                            )
                        }
                        TypeConstraintNode::Domain(domain) => {
                            domain.arguments.iter().any(|argument| {
                                type_reference_contains_expression(
                                    program, *argument, expression, visited,
                                )
                            })
                        }
                        TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {
                            false
                        }
                    })
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            type_reference_contains_expression(program, *element_type, expression, visited)
        }
        TypeReferenceNode::Generic { arguments, .. } => program
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .any(|argument| {
                type_reference_contains_expression(program, *argument, expression, visited)
            }),
        TypeReferenceNode::ConstExpression(root) => {
            super::expression_contains(program, *root, expression, &mut Vec::new())
        }
        TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Unit => false,
    }
}

fn member_target_in_environment(
    program: &TypedTrees,
    member: &TableMemberExpression,
    environment: &TypeEnvironment,
) -> Option<OwnerMemberTarget> {
    let receiver_type =
        infer_expression_type(program, member.receiver, environment, &mut Vec::new())?;
    resolve_member_symbol(program, receiver_type, member)
        .map(OwnerMemberTarget::Declaration)
        .or_else(|| {
            (member.member.as_str() == "len" && inferred_type_is_collection(program, receiver_type))
                .then_some(OwnerMemberTarget::CollectionLength)
        })
}

fn infer_expression_type(
    program: &TypedTrees,
    expression: ExpressionHandle,
    environment: &TypeEnvironment,
    visited: &mut Vec<ExpressionHandle>,
) -> Option<InferredType> {
    if !expression.is_valid() || visited.contains(&expression) {
        return None;
    }
    visited.push(expression);
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            infer_expression_type(program, atomic.value, environment, visited)
        }
        ExpressionNode::Borrow(inner) => {
            infer_expression_type(program, inner.target, environment, visited)
        }
        ExpressionNode::Cast(cast) => Some(InferredType::TypeReference(cast.target_type)),
        ExpressionNode::Name(path) => {
            if path.symbol.is_valid()
                && let Some(type_reference) = super::type_reference_for_symbol(program, path.symbol)
            {
                return Some(InferredType::TypeReference(type_reference));
            }
            let [name] = program.expression_table.name_path_members(path.members) else {
                return None;
            };
            if name.as_str() == "self" {
                return environment.self_type;
            }
            if name.as_str() == "result" {
                return environment.result_type.map(InferredType::TypeReference);
            }
            let mut matches = environment
                .bindings
                .iter()
                .filter(|binding| {
                    (path.head_symbol.is_valid() && binding.symbol == path.head_symbol)
                        || (path.symbol.is_valid() && binding.symbol == path.symbol)
                        || (!path.head_symbol.is_valid()
                            && !path.symbol.is_valid()
                            && binding.name == name.as_str())
                })
                .map(|binding| binding.type_reference);
            let first = matches.next()?;
            matches
                .all(|type_reference| type_reference == first)
                .then_some(InferredType::TypeReference(first))
        }
        ExpressionNode::Member(member) => {
            let receiver = infer_expression_type(program, member.receiver, environment, visited)?;
            let symbol = resolve_member_symbol(program, receiver, member)?;
            super::type_reference_for_symbol(program, symbol).map(InferredType::TypeReference)
        }
        ExpressionNode::Call(call) => {
            if let Some(operator) =
                psi_typed_trees::operator::resolve_named_expression_call(program, call)
            {
                return operator
                    .return_type
                    .is_valid()
                    .then_some(InferredType::TypeReference(operator.return_type));
            }
            program
                .machines()
                .iter()
                .flat_map(|machine| program.machine_states(machine))
                .find(|state| state.symbol == call.target_symbol)
                .and_then(|state| {
                    state
                        .return_type
                        .is_valid()
                        .then_some(InferredType::TypeReference(state.return_type))
                })
        }
        ExpressionNode::Indexed(indexed) => {
            let collection =
                infer_expression_type(program, indexed.collection, environment, visited)?;
            collection_element_type(program, collection).map(InferredType::TypeReference)
        }
        ExpressionNode::StructLiteral(literal) => literal
            .type_symbol
            .is_valid()
            .then_some(InferredType::Nominal(literal.type_symbol)),
        ExpressionNode::String(_) => Some(InferredType::CompilerString),
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::Unary(_)
        | ExpressionNode::ZeroValue(_) => None,
    }
}

fn resolve_member_symbol(
    program: &TypedTrees,
    receiver_type: InferredType,
    member: &TableMemberExpression,
) -> Option<SymbolHandle> {
    let nominal = inferred_nominal_symbol(program, receiver_type)?;
    let data = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == nominal)?;

    let mut candidates = Vec::new();
    for data_member in program.data_members(data) {
        match data_member {
            psi_typed_trees::data::DataMember::Field(field)
                if field.name.as_str() == member.member.as_str() =>
            {
                if !candidates.contains(&field.symbol) {
                    candidates.push(field.symbol);
                }
            }
            psi_typed_trees::data::DataMember::Variant(variant) => {
                if member
                    .case_variant
                    .as_ref()
                    .is_some_and(|case| case != &variant.name)
                {
                    continue;
                }
                for field in program
                    .data_payload_fields(variant)
                    .iter()
                    .filter(|field| field.name.as_str() == member.member.as_str())
                {
                    if !candidates.contains(&field.symbol) {
                        candidates.push(field.symbol);
                    }
                }
            }
            _ => {}
        }
    }
    let [selected] = candidates.as_slice() else {
        return None;
    };
    Some(*selected)
}

fn inferred_nominal_symbol(program: &TypedTrees, inferred: InferredType) -> Option<SymbolHandle> {
    match inferred {
        InferredType::Nominal(symbol) => symbol.is_valid().then_some(symbol),
        InferredType::TypeReference(type_reference) => {
            let symbol = program.type_reference_table.type_symbol(type_reference);
            symbol.is_valid().then_some(symbol)
        }
        InferredType::CompilerString => None,
    }
}

fn inferred_type_is_collection(program: &TypedTrees, inferred: InferredType) -> bool {
    match inferred {
        InferredType::Nominal(_) => false,
        InferredType::TypeReference(type_reference) => {
            super::type_reference_is_collection(program, type_reference)
        }
        InferredType::CompilerString => false,
    }
}

fn collection_view_operation_for_receiver(
    program: &TypedTrees,
    operation: psi_language_semantics::declaration_selection::CollectionViewOperation,
    receiver: InferredType,
) -> Option<psi_language_semantics::declaration_selection::CollectionViewOperation> {
    use psi_language_semantics::declaration_selection::CollectionViewOperation;

    match operation {
        CollectionViewOperation::SharedSlice | CollectionViewOperation::MutableSlice
            if inferred_type_is_collection(program, receiver) =>
        {
            Some(operation)
        }
        CollectionViewOperation::TextView | CollectionViewOperation::Bytes
            if inferred_type_is_compiler_text(program, receiver) =>
        {
            Some(operation)
        }
        _ => None,
    }
}

fn inferred_type_is_compiler_text(program: &TypedTrees, inferred: InferredType) -> bool {
    let InferredType::TypeReference(mut type_reference) = inferred else {
        return matches!(inferred, InferredType::CompilerString);
    };
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => type_reference = *referee,
            TypeReferenceNode::Named { symbol, name } => {
                return !symbol.is_valid() && matches!(name.as_str(), "String" | "Str");
            }
            _ => return false,
        }
    }
}

fn collection_element_type(
    program: &TypedTrees,
    inferred: InferredType,
) -> Option<TypeReferenceHandle> {
    let InferredType::TypeReference(type_reference) = inferred else {
        return None;
    };
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => collection_element_type(program, InferredType::TypeReference(*referee)),
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => Some(*element_type),
        _ => None,
    }
}
