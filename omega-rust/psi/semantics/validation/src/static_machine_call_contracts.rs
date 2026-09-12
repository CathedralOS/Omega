//! Exact custody of a static binder after its executable target is substituted.

use diagnostics::Diagnostic;
use flow_effects::OperationalPlan;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;

/// Stable operational projection embedded in the full template commitment.
/// Parameter types may specialize in place; callable kind, service identity,
/// and fixed acknowledgement envelopes must not narrow with them.
pub fn static_machine_parameter_contract_bytes(
    program: &TypedTrees,
    parameters: &[typed_trees::data::TypeParameter],
) -> Vec<u8> {
    let mut bytes = b"omega.static-machine-operational-contracts.v1\0".to_vec();
    bytes.extend((parameters.len() as u64).to_le_bytes());
    for (ordinal, parameter) in parameters.iter().enumerate() {
        let TypeParameterKind::Machine { contract } = &parameter.kind else {
            continue;
        };
        bytes.extend((ordinal as u64).to_le_bytes());
        let Some(contract) = program.machine_parameter_contract_view(contract) else {
            // Invalid typed inputs receive a distinct encoding; validation
            // rejects a retained call before using a missing contract.
            bytes.push(0);
            continue;
        };
        match contract {
            typed_trees::data::MachineParameterContractView::Structural(_) => bytes.push(1),
            typed_trees::data::MachineParameterContractView::Nominal {
                trait_definition,
                requirement,
            } => {
                bytes.push(2);
                encode_declaration(program, trait_definition.symbol, &mut bytes);
                let identity = program
                    .normalized_trait_requirement_overload_identity(trait_definition, requirement)
                    .identity();
                bytes.extend((identity.len() as u64).to_le_bytes());
                bytes.extend(identity.as_bytes());
            }
        }
        let signature = contract.signature();
        let mut services = Vec::new();
        for service in program
            .service_reach_rows
            .services(signature.service_reach_row)
        {
            let mut encoded = Vec::new();
            if let Some(definition) = program.service_reaches.definition(*service) {
                encoded.push(1);
                encode_declaration(program, definition.symbol, &mut encoded);
            } else {
                encoded.push(0);
            }
            services.push(encoded);
        }
        services.sort();
        services.dedup();
        bytes.extend((services.len() as u64).to_le_bytes());
        for service in services {
            bytes.extend(service);
        }
        bytes.push(u8::from(signature.service_reach_is_installation_bound));
        bytes.push(u8::from(signature.suspends));
        bytes.push(u8::from(signature.blocks));
        let mut invocations = Vec::new();
        for invocation in
            crate::effect_inference::declared_signature_invocations(program, signature)
        {
            let mut encoded = Vec::new();
            match invocation {
                flow_effects::InvocationTarget::Parameter(ordinal) => {
                    encoded.push(1);
                    encoded.extend(ordinal.to_le_bytes());
                }
                flow_effects::InvocationTarget::Service(service) => {
                    encoded.push(2);
                    encode_declaration(program, service, &mut encoded);
                }
            }
            invocations.push(encoded);
        }
        invocations.sort();
        invocations.dedup();
        bytes.extend((invocations.len() as u64).to_le_bytes());
        for invocation in invocations {
            bytes.extend(invocation);
        }
    }
    bytes
}

fn encode_declaration(program: &TypedTrees, symbol: SymbolHandle, bytes: &mut Vec<u8>) {
    if let Some(package) = program.symbols.symbol_package_identity(symbol) {
        bytes.push(1);
        bytes.extend(package.digest());
    } else {
        bytes.push(0);
    }
    let path = program.symbols.display_path(symbol, "::");
    bytes.extend((path.len() as u64).to_le_bytes());
    bytes.extend(path.as_bytes());
}

/// Application-local custody, deliberately separate from the public template
/// dependency. Removing a retained binder changes the application commitment
/// even when the remaining calls still form individually valid joins.
pub fn static_machine_call_binding_bytes(
    program: &TypedTrees,
    operational: &OperationalPlan,
    specialization: &typed_trees::typed_trees::MachineSpecialization,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = b"omega.static-machine-call-bindings.v1\0".to_vec();
    let mut owners = operational
        .machines()
        .iter()
        .filter(|machine| machine.symbol == specialization.instance);
    let Some(owner) = owners.next() else {
        return Err(Diagnostic::error(
            "static machine binding footprint lost its caller",
        ));
    };
    if owners.next().is_some() {
        return Err(Diagnostic::error(
            "static machine binding footprint has ambiguous callers",
        ));
    }
    let parameters = program
        .data_type_parameters
        .span_or_empty(specialization.template_parameters);
    for (state_ordinal, state) in operational
        .states
        .span_or_empty(owner.states)
        .iter()
        .enumerate()
    {
        for call in operational.calls.span_or_empty(state.calls) {
            if !call.static_machine_parameter.is_valid() {
                continue;
            }
            validate_call_selection(
                program,
                owner.symbol,
                call.static_machine_parameter,
                call.target_state_symbol,
            )?;
            let Some(binder_ordinal) = parameters
                .iter()
                .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Machine { .. }))
                .position(|parameter| parameter.symbol == call.static_machine_parameter)
            else {
                return Err(Diagnostic::error(
                    "static machine binding footprint lost its binder",
                ));
            };
            let Some((_, selected)) = crate::transitions::resolved_transition_target_state(
                program,
                call.target_state_symbol,
            ) else {
                return Err(Diagnostic::error(
                    "static machine binding footprint lost its selected entry",
                ));
            };
            bytes.extend((state_ordinal as u64).to_le_bytes());
            bytes.extend((call.statement_index as u64).to_le_bytes());
            bytes.extend((call.call_ordinal as u64).to_le_bytes());
            bytes.extend((binder_ordinal as u64).to_le_bytes());
            encode_declaration(program, selected.symbol, &mut bytes);
        }
    }
    Ok(bytes)
}

/// Compare the retained reader inputs with their committed template segment.
pub fn validate_static_machine_parameter_contracts(program: &TypedTrees) -> Result<(), Diagnostic> {
    for specialization in &program.machine_specializations {
        let parameters = program
            .data_type_parameters
            .span_or_empty(specialization.template_parameters);
        let expected = static_machine_parameter_contract_bytes(program, parameters);
        if !specialization
            .canonical_template_contract_bytes
            .ends_with(&expected)
        {
            return Err(Diagnostic::error(
                "retained static machine parameter contracts disagree with their canonical template",
            ));
        }
    }
    Ok(())
}

/// Rejoin retained call binders to their caller's exact specialization tuple.
/// Operational traversal supplies owner and call coordinates; neither readable
/// target names nor matching requirement shapes establish selection identity.
pub fn validate_static_machine_call_contracts(
    program: &TypedTrees,
    operational: &OperationalPlan,
) -> Result<(), Diagnostic> {
    validate_static_machine_parameter_contracts(program)?;
    for machine in operational.machines() {
        for state in operational.states.span_or_empty(machine.states) {
            for call in operational.calls.span_or_empty(state.calls) {
                if !call.static_machine_parameter.is_valid() {
                    continue;
                }
                validate_call_selection(
                    program,
                    machine.symbol,
                    call.static_machine_parameter,
                    call.target_state_symbol,
                )?;
            }
        }
    }
    Ok(())
}

fn validate_call_selection(
    program: &TypedTrees,
    caller: SymbolHandle,
    binder: SymbolHandle,
    target: SymbolHandle,
) -> Result<(), Diagnostic> {
    let mut specializations = program
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.instance == caller);
    let Some(specialization) = specializations.next() else {
        return Err(Diagnostic::error(
            "retained static machine call has no owning specialization",
        ));
    };
    if specializations.next().is_some() {
        return Err(Diagnostic::error(
            "retained static machine call has ambiguous owning specializations",
        ));
    }
    let parameters = program
        .data_type_parameters
        .span_or_empty(specialization.template_parameters);
    let mut matching_parameters = parameters
        .iter()
        .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Machine { .. }))
        .enumerate()
        .filter(|(_, parameter)| parameter.symbol == binder);
    let Some((ordinal, parameter)) = matching_parameters.next() else {
        return Err(Diagnostic::error(
            "retained static machine call binder is absent from its template parameters",
        ));
    };
    if matching_parameters.next().is_some() {
        return Err(Diagnostic::error(
            "retained static machine call binder is duplicated in its template parameters",
        ));
    }
    let TypeParameterKind::Machine { contract } = &parameter.kind else {
        return Err(Diagnostic::error(
            "retained static machine call lost its machine parameter contract",
        ));
    };
    if program.machine_parameter_contract_view(contract).is_none() {
        return Err(Diagnostic::error(
            "retained static machine call has no exact requirement contract",
        ));
    }
    let Some(selected) = specialization.machine_arguments.get(ordinal) else {
        return Err(Diagnostic::error(
            "retained static machine call has no selected argument at its binder ordinal",
        ));
    };
    let selected_entry = crate::transitions::resolved_transition_target_state(program, *selected)
        .map(|(_, state)| state.symbol);
    let actual_entry = crate::transitions::resolved_transition_target_state(program, target)
        .map(|(_, state)| state.symbol);
    if selected_entry.is_none() || selected_entry != actual_entry {
        return Err(Diagnostic::error(
            "retained static machine call target disagrees with its selected argument",
        ));
    }
    Ok(())
}
