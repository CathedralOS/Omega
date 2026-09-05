use super::*;

pub(crate) fn specialize_selected_generic_operator_providers(
    templates: &TypedTrees,
    program: &mut TypedTrees,
    selected: &[crate::SelectedGenericOperatorProviderSpecialization],
) -> Result<usize, Vec<Diagnostic>> {
    if selected.is_empty() {
        return Ok(0);
    }
    materialize_static_argument_types(program);
    let mut diagnostics = Vec::new();
    let mut materialized = 0_usize;

    for request in selected {
        // Clone the immutable authored graph per request so concrete type
        // references discovered after ordinary specialization can be copied
        // into the clone without mutating the template authority shared by
        // another selected provider.
        let mut source = templates.clone();
        materialize_static_argument_types(&mut source);
        let Some(machine_index) = source
            .machines()
            .iter()
            .position(|machine| machine.symbol == request.realization_machine)
        else {
            diagnostics.push(Diagnostic::error(
                "selected generic operator provider names no realization machine",
            ));
            continue;
        };
        let template_machine = source.machines()[machine_index].clone();
        let Some(operator) =
            typed_trees::operator::declaration_by_symbol(&source, request.requirement_operator)
                .cloned()
        else {
            diagnostics.push(Diagnostic::error(
                "selected generic operator provider names no operator requirement",
            ));
            continue;
        };
        let [namespace, requirement] = source.operator_path_members(operator.name) else {
            diagnostics.push(Diagnostic::error(
                "selected generic operator provider requirement has no exact two-part path",
            ));
            continue;
        };
        if typed_trees::operator::resolve_satisfied_checked_operator(
            &source,
            &template_machine,
            namespace.as_str(),
            requirement.as_str(),
        )
        .is_none_or(|resolved| resolved.symbol != operator.symbol)
        {
            diagnostics.push(Diagnostic::error(format!(
                "selected generic operator provider `{}` does not realize its exact requirement",
                template_machine.name,
            )));
            continue;
        }

        let current_operator =
            typed_trees::operator::declaration_by_symbol(program, request.requirement_operator)
                .expect("selected provider requirement survives specialization");
        let applications = match selected_operator_applications(program, current_operator) {
            Ok(applications) => applications,
            Err(mut errors) => {
                diagnostics.append(&mut errors);
                continue;
            }
        };
        if applications.is_empty() {
            continue;
        }
        let template = selected_operator_candidate(&source, machine_index);
        let mut concrete = Vec::<(SpecializationKey, Candidate)>::new();
        for application in applications {
            let application = copy_application_types(program, &mut source, &application);
            let candidate = match selected_operator_candidate_for_application(
                &source,
                &template,
                &template_machine,
                &application,
            ) {
                Ok(candidate) => candidate,
                Err(error) => {
                    diagnostics.push(error);
                    continue;
                }
            };
            let key = SpecializationKey {
                type_arguments: candidate
                    .type_bindings
                    .iter()
                    .map(|binding| {
                        source
                            .normalized_type_identity(binding.expect("complete selected type"))
                            .into_string()
                    })
                    .collect(),
                const_arguments: candidate
                    .const_bindings
                    .iter()
                    .map(|binding| {
                        source
                            .normalized_type_identity(binding.expect("complete selected const"))
                            .into_string()
                    })
                    .collect(),
                machine_arguments: Vec::new(),
                evidence_arguments: Vec::new(),
            };
            if has_materialized_specialization(
                program,
                template_machine.symbol,
                operator.symbol,
                &key,
            ) {
                continue;
            }
            if !concrete.iter().any(|(retained, _)| retained == &key) {
                concrete.push((key, candidate));
            }
        }
        if concrete.is_empty() {
            continue;
        }

        let mut candidates = concrete
            .into_iter()
            .map(|(_, candidate)| candidate)
            .collect::<Vec<_>>();
        if approved_type_bounds(&source, &candidates)
            .iter()
            .any(|approved| !approved)
        {
            diagnostics.push(Diagnostic::error(format!(
                "selected generic operator provider `{}` has an application that violates its type-property bounds",
                template_machine.name,
            )));
            continue;
        }
        let mut bounds_valid = true;
        for candidate in &mut candidates {
            if let Err(mut errors) = validate_candidate_conformance_bounds(&source, candidate) {
                diagnostics.append(&mut errors);
                bounds_valid = false;
            }
        }
        if !bounds_valid {
            continue;
        }

        let canonical_template_contract_bytes =
            canonical_template_contract_bytes(&source, machine_index);
        let template_contract_report_fingerprint =
            fnv1a_report_fingerprint(&canonical_template_contract_bytes);
        let template_contract_commitment =
            machine_template_commitment(&canonical_template_contract_bytes);
        let normalized_template_identity = normalized_machine_identity(&source, &template_machine)
            .expect("selected generic provider has a normalized template identity");
        let accepted_template_commitment = accepted_template_commitment(&source, machine_index);

        for (ordinal, candidate) in candidates.iter().enumerate() {
            match clone_specialized_machine(
                &source,
                program,
                candidate,
                ordinal,
                template_contract_report_fingerprint,
                template_contract_commitment,
                canonical_template_contract_bytes.clone(),
                normalized_template_identity.clone(),
                accepted_template_commitment.clone(),
            ) {
                Ok(_) => {
                    materialized += 1;
                    let instance = program
                        .machine_specializations
                        .last()
                        .expect("clone retained specialization")
                        .instance;
                    if let Some(machine) = program
                        .machines_mut()
                        .iter_mut()
                        .find(|machine| machine.symbol == instance)
                    {
                        machine.is_public = false;
                    }
                }
                Err(error) => diagnostics.push(error),
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(materialized)
    } else {
        Err(diagnostics)
    }
}

fn copy_application_types(
    program: &TypedTrees,
    source: &mut TypedTrees,
    application: &[typed_trees::operator::ClosedOperatorApplicationArgument],
) -> Vec<typed_trees::operator::ClosedOperatorApplicationArgument> {
    application
        .iter()
        .map(|argument| match argument {
            typed_trees::operator::ClosedOperatorApplicationArgument::Type {
                binder_symbol,
                type_reference,
            } => typed_trees::operator::ClosedOperatorApplicationArgument::Type {
                binder_symbol: *binder_symbol,
                type_reference: copy_type_reference(program, source, *type_reference, &[]),
            },
            typed_trees::operator::ClosedOperatorApplicationArgument::Const {
                binder_symbol,
                declared_carrier,
                value,
            } => typed_trees::operator::ClosedOperatorApplicationArgument::Const {
                binder_symbol: *binder_symbol,
                declared_carrier: copy_type_reference(program, source, *declared_carrier, &[]),
                value: value.clone(),
            },
        })
        .collect()
}

fn has_materialized_specialization(
    program: &TypedTrees,
    template: SymbolHandle,
    requirement: SymbolHandle,
    key: &SpecializationKey,
) -> bool {
    program
        .machine_specializations
        .iter()
        .any(|specialization| {
            specialization.template == template
                && specialization.type_argument_identities == key.type_arguments
                && specialization.const_argument_identities == key.const_arguments
                && specialization.machine_arguments.is_empty()
                && specialization.conformance_arguments.is_empty()
                && specialization
                    .operator_realizations
                    .iter()
                    .any(|realization| realization.requirement_symbol == requirement)
        })
}

fn selected_operator_candidate(program: &TypedTrees, machine_index: usize) -> Candidate {
    let machine = &program.machines()[machine_index];
    let parameters = program.machine_type_parameters(machine);
    let mut type_parameters = Vec::new();
    let mut parameter_bounds = Vec::new();
    let mut const_parameters = Vec::new();
    let mut machine_parameters = Vec::new();
    for parameter in parameters {
        match &parameter.kind {
            TypeParameterKind::Type => {
                type_parameters.push((parameter.symbol, parameter.name.as_str().to_owned()));
                parameter_bounds.push(validation::declared_property_requirements(
                    &parameter.bounds,
                ));
            }
            TypeParameterKind::Const { type_reference } => const_parameters.push((
                parameter.symbol,
                parameter.name.as_str().to_owned(),
                *type_reference,
            )),
            TypeParameterKind::Machine { contract } => {
                let signature = program
                    .machine_parameter_contract_view(contract)
                    .expect("typed machine parameter has a contract")
                    .signature();
                machine_parameters.push((
                    parameter.symbol,
                    parameter.name.as_str().to_owned(),
                    signature.clone(),
                ));
            }
            TypeParameterKind::Proposition { .. } => {}
        }
    }
    let evidence_parameters = machine
        .conformance_bounds
        .iter()
        .filter(|bound| bound.binder.is_some())
        .cloned()
        .collect::<Vec<_>>();
    Candidate {
        machine_index,
        template_symbol: machine.symbol,
        template_name: machine.name.as_str().to_owned(),
        state_symbols: program
            .machine_states(machine)
            .iter()
            .map(|state| state.symbol)
            .collect(),
        type_bindings: vec![None; type_parameters.len()],
        const_bindings: vec![None; const_parameters.len()],
        machine_bindings: vec![None; machine_parameters.len()],
        evidence_bindings: vec![None; evidence_parameters.len()],
        type_parameters,
        parameter_bounds,
        conformance_bounds: machine.conformance_bounds.clone(),
        const_parameters,
        machine_parameters,
        evidence_parameters,
        inferred_conformance_arguments: Vec::new(),
        selected_bound_applications: Vec::new(),
        conflicted: false,
    }
}

fn selected_operator_candidate_for_application(
    program: &TypedTrees,
    template: &Candidate,
    machine: &typed_trees::machine::Machine,
    application: &[typed_trees::operator::ClosedOperatorApplicationArgument],
) -> Result<Candidate, Diagnostic> {
    let machine_parameters = program.machine_type_parameters(machine);
    if machine_parameters.len() != application.len()
        || !template.machine_parameters.is_empty()
        || !template.evidence_parameters.is_empty()
    {
        return Err(Diagnostic::error(format!(
            "selected generic operator provider `{}` has no supported exact type/const specialization tuple",
            machine.name,
        )));
    }
    let mut candidate = template.clone();
    for (parameter, argument) in machine_parameters.iter().zip(application) {
        match (&parameter.kind, argument) {
            (
                TypeParameterKind::Type,
                typed_trees::operator::ClosedOperatorApplicationArgument::Type {
                    type_reference,
                    ..
                },
            ) => {
                let index = candidate
                    .type_parameters
                    .iter()
                    .position(|(symbol, _)| *symbol == parameter.symbol)
                    .expect("selected type parameter retained by candidate");
                candidate.type_bindings[index] = Some(*type_reference);
            }
            (
                TypeParameterKind::Const { .. },
                typed_trees::operator::ClosedOperatorApplicationArgument::Const { value, .. },
            ) => {
                let Some(binding) = const_identity_type_reference(program, value) else {
                    return Err(Diagnostic::error(format!(
                        "selected generic operator provider `{}` has a const application without a supported canonical integer carrier",
                        machine.name,
                    )));
                };
                let index = candidate
                    .const_parameters
                    .iter()
                    .position(|(symbol, _, _)| *symbol == parameter.symbol)
                    .expect("selected const parameter retained by candidate");
                candidate.const_bindings[index] = Some(binding);
            }
            _ => {
                return Err(Diagnostic::error(format!(
                    "selected generic operator provider `{}` changes its requirement telescope category",
                    machine.name,
                )));
            }
        }
    }
    Ok(candidate)
}

fn const_identity_type_reference(
    program: &TypedTrees,
    value: &language_semantics::const_value::CanonicalConstIdentity,
) -> Option<TypeReferenceHandle> {
    let language_semantics::const_value::DecodedCanonicalConstValue::Integer { value, .. } =
        value.decode_encoding()?
    else {
        return None;
    };
    let value = value.to_string();
    program
        .type_reference_table
        .named_references()
        .find(|(_, symbol, name)| !symbol.is_valid() && *name == value)
        .map(|(handle, _, _)| handle)
}

fn selected_operator_applications(
    program: &TypedTrees,
    operator: &typed_trees::operator::OperatorDefinition,
) -> Result<Vec<Vec<typed_trees::operator::ClosedOperatorApplicationArgument>>, Vec<Diagnostic>> {
    let mut symbol_diagnostics = Vec::new();
    let symbols = validation::TopLevelSymbols::build(program, &mut symbol_diagnostics);
    if symbol_diagnostics.iter().any(Diagnostic::is_error) {
        return Err(symbol_diagnostics);
    }
    let mut applications = Vec::new();
    let mut diagnostics = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                for root in executable_statement_expression_roots(program, statement) {
                    let mut expressions = Vec::new();
                    collect_expression_tree(program, root, &mut expressions);
                    for expression in expressions {
                        match selected_operator_application_at_expression(
                            program,
                            &symbols,
                            operator,
                            machine,
                            state,
                            statement_index,
                            expression,
                        ) {
                            Ok(Some(application)) if !application.is_empty() => {
                                if !applications.contains(&application) {
                                    applications.push(application);
                                }
                            }
                            Ok(Some(_)) | Ok(None) => {}
                            Err(error) => diagnostics.push(error),
                        }
                    }
                }
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(applications)
    } else {
        Err(diagnostics)
    }
}

fn executable_statement_expression_roots(
    program: &TypedTrees,
    statement: &StatementNode,
) -> Vec<typed_trees::expression::ExpressionHandle> {
    let mut roots = Vec::new();
    match statement {
        StatementNode::Assignment(assignment) => {
            roots.extend([assignment.target, assignment.value]);
        }
        StatementNode::Call(call) => {
            roots.extend_from_slice(program.statement_table.expression_handles(call.arguments))
        }
        StatementNode::Expression(expression) => roots.push(*expression),
        StatementNode::LocalData(local) => roots.push(local.initial_value),
        StatementNode::Transition(transition) => {
            if let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard {
                roots.push(guard);
            }
            for target in [transition.target, transition.continuation] {
                match program.statement_table.transition_target(target) {
                    typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => roots
                        .extend_from_slice(program.statement_table.expression_handles(*arguments)),
                    typed_trees::statement::TransitionTargetNode::Value(expression) => {
                        roots.push(*expression)
                    }
                    typed_trees::statement::TransitionTargetNode::SelfTarget
                    | typed_trees::statement::TransitionTargetNode::Terminal => {}
                }
            }
        }
        StatementNode::AssemblyFact(_) => {}
    }
    roots.retain(|expression| expression.is_valid());
    roots
}

fn selected_operator_application_at_expression(
    program: &TypedTrees,
    symbols: &validation::TopLevelSymbols<'_>,
    operator: &typed_trees::operator::OperatorDefinition,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> Result<Option<Vec<typed_trees::operator::ClosedOperatorApplicationArgument>>, Diagnostic> {
    let (operands, explicit_static_arguments) =
        match program.expression_table.expression(expression) {
            ExpressionNode::Call(call) => {
                if typed_trees::operator::resolve_named_expression_call(program, call)
                    .is_none_or(|resolved| resolved.symbol != operator.symbol)
                {
                    return Ok(None);
                }
                let explicit = program.expression_table.expression_handles(call.arguments);
                let parameters = program.operator_parameters(operator);
                let mut operands = Vec::with_capacity(explicit.len() + 1);
                if call.receiver.is_valid() && parameters.len() == explicit.len() + 1 {
                    operands.push(call.receiver);
                }
                operands.extend_from_slice(explicit);
                (operands, Some(call.machine_arguments.as_ref()))
            }
            ExpressionNode::Binary(binary) => {
                let Some(spelling) = binary_operator_spelling(binary.operator) else {
                    return Ok(None);
                };
                if operator.spelling != Some(spelling) {
                    return Ok(None);
                }
                (vec![binary.left, binary.right], None)
            }
            ExpressionNode::Indexed(indexed)
                if !matches!(
                    program.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) && operator.spelling
                    == Some(language_core::operator_spelling::OperatorSpelling::Index) =>
            {
                (vec![indexed.collection, indexed.index], None)
            }
            _ => return Ok(None),
        };
    let origin = checked_trees::CheckedValueOrigin::StateStatement {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        statement_index,
        role: Default::default(),
    };
    let operand_types = operands
        .iter()
        .map(|operand| {
            crate::operators::expression_type_reference_for_origin(program, *operand, origin)
                .or_else(|| {
                    validation::declared_place_type_raw(program, machine, Some(state), *operand)
                })
                .or_else(|| validation::landed_integer_literal_type_reference(program, *operand))
        })
        .collect::<Vec<_>>();

    if let Some(explicit_static_arguments) = explicit_static_arguments {
        return match validation::validate_named_operator_application(
            program,
            symbols,
            operator,
            explicit_static_arguments,
            &operand_types,
        )? {
            Some(application) => Ok(Some(application)),
            // A generic template may forward one of its own binders here.
            // That is a symbolic demand, not a concrete specialization and
            // not final coverage. A concrete clone is revisited by the outer
            // fixed-point loop; validation still rejects an emitted open use.
            None => Ok(None),
        };
    }
    let Some(spelling) = operator.spelling else {
        return Ok(None);
    };
    if !typed_trees::operator::resolve_spelling_for_operands(program, spelling, &operand_types)
        .iter()
        .any(|candidate| candidate.operator.symbol == operator.symbol)
    {
        return Ok(None);
    }
    let Some(application) = typed_trees::operator::closed_operator_application_for_operands(
        program,
        operator,
        &operand_types,
    ) else {
        // Generic templates and declaration-derived expressions may retain
        // the spelling while their operands are still open. They are symbolic
        // demand, not a concrete specialization request.
        return Ok(None);
    };
    validation::validate_closed_operator_application(program, symbols, operator, &application)?;
    Ok(Some(application))
}

fn binary_operator_spelling(
    operator: typed_trees::expression::BinaryOperator,
) -> Option<language_core::operator_spelling::OperatorSpelling> {
    use language_core::operator_spelling::OperatorSpelling;
    use typed_trees::expression::BinaryOperator;

    Some(match operator {
        BinaryOperator::Add => OperatorSpelling::Add,
        BinaryOperator::Subtract => OperatorSpelling::Subtract,
        BinaryOperator::Multiply => OperatorSpelling::Multiply,
        BinaryOperator::Divide => OperatorSpelling::Divide,
        BinaryOperator::Modulo => OperatorSpelling::Modulo,
        BinaryOperator::Equal => OperatorSpelling::Equal,
        BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        BinaryOperator::And
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight => return None,
    })
}
