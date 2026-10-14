//! Static binder custody encoded into machine specialization identities.
//!
//! The template retains its inferred finite reach dependency, not the row of
//! whichever application happens to be emitted, so private helper
//! contributions change the template commitment. Each application also
//! encodes which binder every rewritten call selected. Stage 04 folds these
//! bytes into the template and specialization commitments once; no later
//! stage recomputes them.

use diagnostics::Diagnostic;
use flow_effects::OperationalPlan;
use symbols::SymbolHandle;
use typed_trees::data::TypeParameterKind;

/// Original finite reach dependency followed by the exact binder contracts.
/// The caller owns inference so one immutable graph is shared across a batch.
/// Service declarations use package-qualified identity; variables use positions
/// in the full telescope, including non-machine and unused binders.
pub fn static_machine_template_reach_contract_bytes(
    program: &dyn crate::CheckingInput,
    inferred: &flow_effects::ServiceReachInferencePlan,
    machine: &typed_trees::machine::Machine,
) -> Result<Vec<u8>, Diagnostic> {
    let mut summaries = inferred
        .machines()
        .iter()
        .filter(|summary| summary.machine == machine.symbol);
    let summary = summaries.next().ok_or_else(|| {
        Diagnostic::error("generic template has no inferred service-reach dependency")
    })?;
    if summaries.next().is_some() {
        return Err(Diagnostic::error(
            "generic template has ambiguous reach dependencies",
        ));
    }
    let parameters = program.machine_type_parameters(machine);
    if parameters.len() != machine.type_parameters.len() {
        return Err(Diagnostic::error(
            "generic reach dependency lost its template telescope",
        ));
    }
    let mut bytes = b"omega.static-machine-reach-dependency.v1\0".to_vec();
    let mut services = Vec::new();
    for service in inferred.services(summary.dependency.concrete) {
        let definition = program
            .service_reaches
            .definition(*service)
            .ok_or_else(|| Diagnostic::error("generic reach dependency has an unknown service"))?;
        let mut encoded = Vec::new();
        encode_declaration(program, definition.symbol, &mut encoded);
        services.push(encoded);
    }
    services.sort();
    services.dedup();
    bytes.extend((services.len() as u64).to_le_bytes());
    for service in services {
        bytes.extend(service);
    }
    let dependencies = inferred
        .dependency_parameters
        .span_or_empty(summary.dependency.parameters);
    if dependencies.len() != summary.dependency.parameters.len() {
        return Err(Diagnostic::error(
            "generic reach dependency lost its binder span",
        ));
    }
    let mut ordinals = Vec::with_capacity(dependencies.len());
    let capture = program.conformance_by_symbol(program.declaration_scope_parent(machine.symbol));
    let captured_parameters = capture
        .map(|declaration| program.conformance_type_parameters(declaration))
        .unwrap_or(&[]);
    if capture
        .is_some_and(|declaration| captured_parameters.len() != declaration.type_parameters.len())
    {
        return Err(Diagnostic::error(
            "capture reach dependency lost its map telescope",
        ));
    }
    let mut captured_ordinals = Vec::new();
    for dependency in dependencies {
        let (scope, scope_parameters) = if parameters
            .iter()
            .any(|parameter| parameter.symbol == *dependency)
        {
            (false, parameters)
        } else {
            (true, captured_parameters)
        };
        let mut matches = scope_parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| parameter.symbol == *dependency);
        let Some((ordinal, parameter)) = matches.next() else {
            return Err(Diagnostic::error(
                "generic reach variable is outside its invocation and capture telescopes",
            ));
        };
        if matches.next().is_some()
            || !matches!(
                parameter.kind,
                TypeParameterKind::Machine {
                    contract: typed_trees::data::MachineParameterContract::Nominal { .. }
                }
            )
        {
            return Err(Diagnostic::error(
                "generic reach variable is not one exact nominal binder",
            ));
        }
        if scope {
            captured_ordinals.push(ordinal as u64);
        } else {
            ordinals.push(ordinal as u64);
        }
    }
    ordinals.sort_unstable();
    if ordinals.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(Diagnostic::error(
            "generic reach dependency repeats a binder",
        ));
    }
    bytes.extend((ordinals.len() as u64).to_le_bytes());
    for ordinal in ordinals {
        bytes.extend(ordinal.to_le_bytes());
    }
    bytes.extend(static_machine_parameter_contract_bytes(program, parameters));
    if let Some(capture) = capture {
        // Owned rows retain their map's nominal dependencies independently of
        // invocation binders. Splicing these into the invocation telescope
        // would give same-spelled local binders the wrong contract authority.
        bytes.extend(b"omega.static-machine-capture-reach.v1\0");
        encode_declaration(program, capture.symbol, &mut bytes);
        captured_ordinals.sort_unstable();
        if captured_ordinals.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(Diagnostic::error(
                "capture reach dependency repeats a binder",
            ));
        }
        bytes.extend((captured_ordinals.len() as u64).to_le_bytes());
        for ordinal in captured_ordinals {
            bytes.extend(ordinal.to_le_bytes());
        }
        bytes.extend(static_machine_parameter_contract_bytes(
            program,
            captured_parameters,
        ));
    }
    Ok(bytes)
}

/// Stable operational projection embedded in the full template commitment.
/// Parameter types specialize in private instances; callable kind, service identity,
/// and fixed acknowledgement envelopes must not narrow with them.
pub fn static_machine_parameter_contract_bytes(
    program: &dyn crate::CheckingInput,
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
            // Invalid typed inputs receive a distinct encoding.
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
                let identity =
                    program.trait_requirement_callable_identity_text(trait_definition, requirement);
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
        for invocation in crate::machine_calls::effect_inference::declared_signature_invocations(
            program, signature,
        ) {
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

fn encode_declaration(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
    bytes: &mut Vec<u8>,
) {
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
    program: &dyn crate::CheckingInput,
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
            let Some(binder_ordinal) = parameters
                .iter()
                .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Machine { .. }))
                .position(|parameter| parameter.symbol == call.static_machine_parameter)
            else {
                return Err(Diagnostic::error(
                    "static machine binding footprint lost its binder",
                ));
            };
            let Some((_, selected)) =
                crate::declarations::transitions::resolved_transition_target_state(
                    program,
                    call.target_state_symbol,
                )
            else {
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
