use super::*;

pub(crate) fn specialize_selected_generic_operator_providers(
    program: &mut TypedTrees,
    selected: &[crate::SelectedGenericOperatorProviderSpecialization],
) -> Result<(), Vec<Diagnostic>> {
    if selected.is_empty() {
        return Ok(());
    }
    materialize_static_argument_types(program);
    let source = program.clone();
    let mut diagnostics = Vec::new();

    for request in selected {
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
        let template_machine = &source.machines()[machine_index];
        let Some(operator) =
            psi_typed_trees::operator::declaration_by_symbol(&source, request.requirement_operator)
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
        if psi_typed_trees::operator::resolve_satisfied_checked_operator(
            &source,
            template_machine,
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

        let applications = match selected_named_operator_applications(&source, operator) {
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
            let candidate = match selected_operator_candidate_for_application(
                &source,
                &template,
                template_machine,
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
        let normalized_template_identity = normalized_machine_identity(&source, template_machine)
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
        Ok(())
    } else {
        Err(diagnostics)
    }
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
                parameter_bounds.push(psi_validation::declared_property_requirements(
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
    machine: &psi_typed_trees::machine::Machine,
    application: &[psi_typed_trees::operator::ClosedOperatorApplicationArgument],
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
                psi_typed_trees::operator::ClosedOperatorApplicationArgument::Type {
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
                psi_typed_trees::operator::ClosedOperatorApplicationArgument::Const {
                    value, ..
                },
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
    value: &psi_language_semantics::const_value::CanonicalConstIdentity,
) -> Option<TypeReferenceHandle> {
    let psi_language_semantics::const_value::DecodedCanonicalConstValue::Integer { value, .. } =
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

fn selected_named_operator_applications(
    program: &TypedTrees,
    operator: &psi_typed_trees::operator::OperatorDefinition,
) -> Result<Vec<Vec<psi_typed_trees::operator::ClosedOperatorApplicationArgument>>, Vec<Diagnostic>>
{
    let mut symbol_diagnostics = Vec::new();
    let symbols = psi_validation::TopLevelSymbols::build(program, &mut symbol_diagnostics);
    if symbol_diagnostics.iter().any(Diagnostic::is_error) {
        return Err(symbol_diagnostics);
    }
    let mut applications = Vec::new();
    let mut diagnostics = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let expression = match statement {
                    StatementNode::Expression(expression) => Some(*expression),
                    StatementNode::LocalData(local) if local.initial_value.is_valid() => {
                        Some(local.initial_value)
                    }
                    StatementNode::Assignment(assignment) if assignment.value.is_valid() => {
                        Some(assignment.value)
                    }
                    _ => None,
                };
                let Some(expression) = expression else {
                    continue;
                };
                let ExpressionNode::Call(call) = program.expression_table.expression(expression)
                else {
                    continue;
                };
                if psi_typed_trees::operator::resolve_named_expression_call(program, call)
                    .is_none_or(|resolved| resolved.symbol != operator.symbol)
                {
                    continue;
                }
                let explicit = program.expression_table.expression_handles(call.arguments);
                let parameters = program.operator_parameters(operator);
                let mut operands = Vec::with_capacity(explicit.len() + 1);
                if call.receiver.is_valid() && parameters.len() == explicit.len() + 1 {
                    operands.push(call.receiver);
                }
                operands.extend_from_slice(explicit);
                let operand_types = operands
                    .iter()
                    .map(|operand| {
                        psi_validation::declared_place_type_raw(
                            program,
                            machine,
                            Some(state),
                            *operand,
                        )
                    })
                    .collect::<Vec<_>>();
                match psi_validation::validate_named_operator_application(
                    program,
                    &symbols,
                    operator,
                    &call.machine_arguments,
                    &operand_types,
                ) {
                    Ok(Some(application)) if !application.is_empty() => {
                        if !applications.contains(&application) {
                            applications.push(application);
                        }
                    }
                    Ok(Some(_)) => {}
                    Ok(None) => diagnostics.push(Diagnostic::error(format!(
                        "selected generic operator `{}` has an open application",
                        program
                            .operator_path_members(operator.name)
                            .iter()
                            .map(|member| member.as_str())
                            .collect::<Vec<_>>()
                            .join("::"),
                    ))),
                    Err(error) => diagnostics.push(error),
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
