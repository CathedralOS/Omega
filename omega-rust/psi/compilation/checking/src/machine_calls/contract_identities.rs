//! Canonical template and application commitments. Contract clauses use the
//! shared declared-contract projection; type leaves are normalized by symbol
//! before length framing. Diagnostic spelling never supplies binder identity.
//! Instance construction retains exact template and argument identities, not
//! commitments. Checking encodes each retained authored template once against
//! the completed immutable effect graph. Its nominal binder dependencies remain
//! separate from each concrete instance's substituted reach; hashing an emitted
//! body's row would silently narrow the universal contract.

use crate::contract_structure;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::data::TypeParameterKind;
use typed_trees::type_identity::TypeIdentityRequest;

/// The universal template contract shared by every concrete application.
/// Capture it from the retained authored declaration, not a selected clone.
/// This encoding is binder-positional:
/// renaming a type, machine, or value parameter does not change the identity.
/// Virtual family heads consume the completed member envelope supplied by the
/// caller; encoding never infers another effect graph.
pub fn canonical_template_contract_bytes(
    program: &dyn crate::CheckingInput,
    machine_index: usize,
    reach_inference: &flow_effects::ServiceReachInferencePlan,
    operational: &flow_effects::OperationalPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let machine = &program.machines()[machine_index];
    let parameters = program.machine_type_parameters(machine);
    let captured = program
        .conformance_by_symbol(program.declaration_scope_parent(machine.symbol))
        .map(|map| program.conformance_type_parameters(map))
        .unwrap_or(&[]);
    let capture_binders = type_parameter_binders(captured, &[])
        .into_iter()
        .map(|(symbol, name)| (symbol, format!("$capture{name}")))
        .collect::<Vec<_>>();
    let binders = type_parameter_binders(parameters, &capture_binders);
    let content_conservation = crate::content_conservation_plans(program);
    let mut bytes = Vec::new();
    bytes.push(u8::from(machine.structural_type_equations_pending));
    bytes.extend(machine.name.as_str().as_bytes());
    bytes.push(0xff);
    bytes.push(match machine.supply_mode {
        language_semantics::MachineSupplyMode::CheckedBody => 1,
        language_semantics::MachineSupplyMode::Requirement => 2,
        language_semantics::MachineSupplyMode::Boundary => 3,
        language_semantics::MachineSupplyMode::AdmissionClaim => 4,
        language_semantics::MachineSupplyMode::ExternalRealization { .. } => 5,
        language_semantics::MachineSupplyMode::TopLevelRequirement => 6,
    });
    for (index, parameter) in parameters.iter().enumerate() {
        bytes.push(match parameter.kind {
            TypeParameterKind::Type => 1,
            TypeParameterKind::Const { .. } => 2,
            TypeParameterKind::Value { .. } => 5,
            TypeParameterKind::Machine { .. } => 3,
            TypeParameterKind::Proposition { .. } => 4,
            TypeParameterKind::Implementation => 6,
        });
        bytes.extend((index as u32).to_le_bytes());
        encode_data_properties(parameter.bounds, &mut bytes);
        encode_generic_parameter_contract(
            program,
            &parameter.kind,
            &binders,
            &content_conservation,
            &mut bytes,
        )?;
        bytes.push(0xfe);
    }
    let mut conformance_bounds = Vec::new();
    for bound in &machine.conformance_bounds {
        let mut encoded = Vec::new();
        encoded.extend(bound.trait_name.as_str().as_bytes());
        encoded.push(0);
        for argument in &bound.arguments {
            encode_identity_text(
                &program.type_identity_request_text(TypeIdentityRequest {
                    binders: &binders,
                    ..TypeIdentityRequest::ordinary(*argument)
                }),
                &mut encoded,
            );
            encoded.push(0);
        }
        conformance_bounds.push(encoded);
    }
    conformance_bounds.sort();
    for bound in conformance_bounds {
        bytes.extend(bound);
        bytes.push(0xfb);
    }
    let mut state_shapes = Vec::new();
    for state in program.machine_states(machine) {
        let mut shape = Vec::new();
        encode_state_shape(program, state, &binders, &content_conservation, &mut shape);
        state_shapes.push(shape);
    }
    state_shapes.sort();
    for shape in state_shapes {
        bytes.extend(shape);
        bytes.push(0xfd);
    }
    let (service_row, may_suspend, may_block) = if machine.target_family_interface {
        let reach = reach_inference.for_machine(machine.symbol).ok_or_else(|| {
            Diagnostic::error("target-family template has no published service-reach summary")
        })?;
        let operational = operational
            .machine_by_symbol(machine.symbol)
            .ok_or_else(|| {
                Diagnostic::error("target-family template has no completed operational summary")
            })?;
        (
            reach_inference.services(reach.published),
            operational.transitive_may_suspend,
            operational.transitive_may_block,
        )
    } else {
        (
            program
                .service_reach_rows
                .services(machine.service_reach_row),
            machine.suspends,
            machine.blocks,
        )
    };
    let mut service_reaches: Vec<_> = service_row
        .iter()
        .map(|service| {
            program
                .service_reaches
                .definition(*service)
                .expect("normalized service row references a registered service")
        })
        .map(|service| service.name.as_str())
        .collect();
    service_reaches.sort_unstable();
    service_reaches.dedup();
    for service in service_reaches {
        bytes.extend(service.as_bytes());
        bytes.push(0);
    }
    bytes.push(u8::from(
        machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
            || machine.is_public
            || program
                .authored_service_reach_rows_for(machine.symbol)
                .next()
                .is_some()
            || !service_row.is_empty(),
    ));
    bytes.push(u8::from(machine.service_reach_is_installation_bound));
    bytes.push(u8::from(may_suspend));
    bytes.push(u8::from(may_block));
    let contract_binders = program
        .machine_states(machine)
        .first()
        .map(|state| formal_parameter_binders(program.state_parameters(state), &binders))
        .unwrap_or_else(|| binders.clone());
    let contracts = contract_structure::declared_contracts(
        program,
        program.machine_contracts(machine),
        &contract_binders,
        &content_conservation,
        true,
    );
    bytes.extend(
        program
            .type_identities
            .with(|table| checked_trees::contract_declarations_commitment(&contracts, table))
            .as_bytes(),
    );
    let termination = contract_structure::declared_termination(
        program,
        &machine.termination_plan.interface,
        &contract_binders,
    );
    bytes.extend(checked_trees::declared_termination_commitment(&termination).as_bytes());
    bytes.extend(crate::static_machine_template_reach_contract_bytes(
        program,
        reach_inference,
        machine,
    )?);
    Ok(bytes)
}

fn fnv1a_report_fingerprint(bytes: &[u8]) -> u64 {
    let mut hash = semantic_vocabulary::Fnv1a::new();
    hash.bytes(bytes);
    hash.finish()
}

/// Hash canonical template bytes under the published template domain separator.
pub fn machine_template_commitment(
    canonical_template_contract_bytes: &[u8],
) -> typed_trees::typed_trees::MachineTemplateCommitment {
    let mut strong =
        crate::digest_encoder::DomainDigestEncoder::new(b"omega.machine-template.v2\0");
    strong.raw(canonical_template_contract_bytes);
    typed_trees::typed_trees::MachineTemplateCommitment::from_digest(strong.finish())
}

/// Deterministic identity of an authored generic machine declaration before
/// monomorphization consumes its binders. Trust receipts and separate-
/// compilation caches use this same identity; callers cannot substitute the
/// identity of one concrete instance for the universal template grant.
pub fn generic_machine_template_report_fingerprint(
    program: &dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
) -> Option<u64> {
    let bytes = canonical_template_bytes_for_symbol(program, machine_symbol)?;
    Some(fnv1a_report_fingerprint(&bytes))
}

/// Canonical contract bytes for `machine_symbol`'s universal template —
/// `None` when the machine carries no template, `Err` flattened to `None`
/// when the reachability computation cannot encode one.
fn canonical_template_bytes_for_symbol(
    program: &dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
) -> Option<Vec<u8>> {
    let machine_index = generic_machine_index(program, machine_symbol)?;
    let operational = crate::infer_operational_may(program);
    let service_reaches = crate::infer_service_reaches(program, &operational);
    canonical_template_contract_bytes(program, machine_index, &service_reaches, &operational).ok()
}

/// Index of `machine_symbol` when the machine declares type parameters —
/// a parameterless machine carries no template to commit.
fn generic_machine_index(
    program: &dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
) -> Option<usize> {
    let machine_index = program.machine_position_by_symbol(machine_symbol)?;
    (!program
        .machine_type_parameters(&program.machines()[machine_index])
        .is_empty())
    .then_some(machine_index)
}

/// Domain-separated strong commitment to the exact canonical universal
/// template captured before monomorphization consumes its binders.
pub fn generic_machine_template_commitment(
    program: &dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
) -> Option<typed_trees::typed_trees::MachineTemplateCommitment> {
    let bytes = canonical_template_bytes_for_symbol(program, machine_symbol)?;
    Some(machine_template_commitment(&bytes))
}

/// Read the admission claim retained on the authored template.
pub fn accepted_template_commitment(
    program: &dyn crate::CheckingInput,
    machine_index: usize,
) -> Option<String> {
    let machine = &program.machines()[machine_index];
    (machine.supply_mode == language_semantics::MachineSupplyMode::AdmissionClaim)
        .then(|| machine.name.as_str().to_owned())
}

/// Publish universal template and application commitments after checking has
/// produced the public contract plans. Authored generic declarations remain
/// generic through preparation, so this reads them directly; no early graph
/// snapshot, per-round inference, or preparation-produced digest is authority.
/// Equal template symbols share their encoding within this readonly invocation.
pub fn prepare_specialization_contract_identities(
    program: &dyn crate::CheckingInput,
    contracts: &checked_trees::MachineContractPlans,
    effects: &crate::PreparedEffectPlans,
) -> Result<checked_trees::CheckedSpecializationIdentities, Vec<Diagnostic>> {
    let mut templates: Vec<(
        SymbolHandle,
        Vec<u8>,
        typed_trees::typed_trees::MachineTemplateCommitment,
    )> = Vec::new();
    let mut identities = Vec::with_capacity(
        program.machine_specializations.len() + program.captured_machine_applications.len(),
    );
    for (_, capture) in program.captured_machine_applications.iter() {
        super::calls::capture_binding_is_exact(program, capture).map_err(|reason| {
            vec![Diagnostic::error(format!(
                "capture application does not replay its complete supplying substitution: {reason}"
            ))]
        })?;
        if program
            .executable_application_template(capture.instance)
            .map_err(|message| vec![Diagnostic::error(message)])?
            != Some(capture.template)
        {
            return Err(vec![Diagnostic::error(
                "capture application has conflicting executable ownership",
            )]);
        }
        let template = program.machine(capture.template).ok_or_else(|| {
            vec![Diagnostic::error(
                "capture application lost its authored template",
            )]
        })?;
        let instance = program.machine(capture.instance).ok_or_else(|| {
            vec![Diagnostic::error(
                "capture application lost its retained executable instance",
            )]
        })?;
        let states = program
            .captured_machine_states
            .span(capture.states)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "capture application has a stale state correspondence",
                )]
            })?;
        let originals = program.machine_states(template);
        let applied = program.machine_states(instance);
        if states.len() != originals.len()
            || states.len() != applied.len()
            || !states.iter().zip(originals.iter().zip(applied)).all(
                |(mapping, (original, applied))| {
                    mapping.template == original.symbol && mapping.instance == applied.symbol
                },
            )
        {
            return Err(vec![Diagnostic::error(
                "capture application does not retain its complete original state correspondence",
            )]);
        }
        let index = program
            .machine_position_by_symbol(capture.template)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "capture application lost its authored template",
                )]
            })?;
        let template_bytes = canonical_template_contract_bytes(
            program,
            index,
            &effects.service_reaches,
            &effects.operational,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        let template_commitment = machine_template_commitment(&template_bytes);
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.machine-capture-application.v1\0",
        );
        digest.raw(&template_commitment.as_bytes());
        let bytes = canonical_capture_application_bytes(program, contracts, capture)
            .map_err(|diagnostic| vec![diagnostic])?;
        digest.raw(&bytes);
        identities.push(checked_trees::CheckedMachineSpecializationIdentity {
            instance: capture.instance,
            template_contract_commitment: template_commitment,
            machine_argument_contract_commitments: captured_machine_contracts(
                program, contracts, capture,
            )
            .map_err(|diagnostic| vec![diagnostic])?
            .into_boxed_slice(),
            commitment: typed_trees::typed_trees::MachineSpecializationCommitment::from_digest(
                digest.finish(),
            ),
        });
    }
    for specialization in &program.machine_specializations {
        let template_index = match templates
            .iter()
            .position(|(symbol, _, _)| *symbol == specialization.template)
        {
            Some(template_index) => template_index,
            None => {
                let machine_index = program
                    .machine_position_by_symbol(specialization.template)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "generic specialization lost its authored template declaration",
                        )]
                    })?;
                let bytes = canonical_template_contract_bytes(
                    program,
                    machine_index,
                    &effects.service_reaches,
                    &effects.operational,
                )
                .map_err(|diagnostic| vec![diagnostic])?;
                let commitment = machine_template_commitment(&bytes);
                let template_index = templates.len();
                templates.push((specialization.template, bytes, commitment));
                template_index
            }
        };
        let (_, template_bytes, template_commitment) = &templates[template_index];
        let mut composed_template_bytes = template_bytes.clone();
        if let Some(capture_identity) = identities
            .iter()
            .find(|identity| identity.instance == specialization.template)
        {
            composed_template_bytes.extend(b"omega.invocation-of-captured-method.v1\0");
            composed_template_bytes.extend(capture_identity.commitment.as_bytes());
        }
        identities.push(
            machine_specialization_identity(
                program,
                contracts,
                specialization,
                &effects.operational,
                &composed_template_bytes,
                *template_commitment,
            )
            .map_err(|diagnostic| vec![diagnostic])?,
        );
    }
    Ok(checked_trees::CheckedSpecializationIdentities::with_rows(
        identities,
    ))
}

fn captured_machine_contracts(
    program: &dyn crate::CheckingInput,
    contracts: &checked_trees::MachineContractPlans,
    capture: &typed_trees::typed_trees::CapturedMachineApplication,
) -> Result<Vec<[u8; 32]>, Diagnostic> {
    let application = &capture.conformance_application;
    let selected = if application.machine_arguments.is_empty() {
        &[][..]
    } else {
        if !application.callable_application.is_valid() {
            return Err(Diagnostic::error(
                "capture lost its checked callback application",
            ));
        }
        program
            .expression_table
            .static_machine_application_selected_machines(application.callable_application)
    };
    if selected.len() != application.machine_arguments.len() {
        return Err(Diagnostic::error(
            "capture callback contract roster differs from map telescope",
        ));
    }
    selected
        .iter()
        .map(|argument| {
            let selected = argument.symbol;
            let owner = program
                .machines()
                .iter()
                .find(|machine| {
                    program
                        .machine_states(machine)
                        .iter()
                        .any(|state| state.symbol == selected)
                })
                .ok_or_else(|| {
                    Diagnostic::error("capture machine operand has no exact callable owner")
                })?;
            contracts
                .for_machine(owner.symbol)
                .map(|contract| contract.commitment.as_bytes())
                .ok_or_else(|| {
                    Diagnostic::error("capture machine operand has no checked public contract")
                })
        })
        .collect()
}

fn canonical_capture_application_bytes(
    program: &dyn crate::CheckingInput,
    contracts: &checked_trees::MachineContractPlans,
    capture: &typed_trees::typed_trees::CapturedMachineApplication,
) -> Result<Vec<u8>, Diagnostic> {
    use typed_trees::static_applications::StaticMachineBindingValue;
    let application = &capture.conformance_application;
    let declaration = program
        .conformance_by_symbol(application.declaration)
        .ok_or_else(|| Diagnostic::error("capture application lost its map declaration"))?;
    let bindings = program
        .expression_table
        .static_machine_application_bindings(capture.binding_application);
    let mut bytes = Vec::new();
    bytes.extend(application.commitment.as_bytes());
    encode_identity_text(&capture.normalized_template_identity, &mut bytes);
    let owner = program
        .expression_table
        .static_machine_application(capture.supplying_application)
        .lexical_owner;
    encode_identity_text(&conformance_symbol_identity(program, owner), &mut bytes);
    let parameters = program.conformance_type_parameters(declaration);
    let binders = type_parameter_binders(parameters, &[]);
    let conservation = crate::content_conservation_plans(program);
    bytes.extend((parameters.len() as u64).to_le_bytes());
    for parameter in parameters {
        encode_data_properties(parameter.bounds, &mut bytes);
        encode_generic_parameter_contract(
            program,
            &parameter.kind,
            &binders,
            &conservation,
            &mut bytes,
        )?;
        let binding = bindings
            .iter()
            .find(|binding| binding.parameter == parameter.symbol)
            .ok_or_else(|| {
                Diagnostic::error("capture application has an incomplete typed binder tuple")
            })?;
        match &binding.value {
            StaticMachineBindingValue::Type(reference) => {
                bytes.push(1);
                encode_identity_text(
                    &program.normalized_type_identity_text(*reference),
                    &mut bytes,
                );
            }
            StaticMachineBindingValue::Const(reference) => {
                bytes.push(2);
                encode_identity_text(
                    &program.normalized_type_identity_text(*reference),
                    &mut bytes,
                );
            }
            StaticMachineBindingValue::Machine(argument) => {
                bytes.push(3);
                encode_identity_text(
                    &conformance_symbol_identity(program, argument.symbol),
                    &mut bytes,
                );
            }
            _ => {
                return Err(Diagnostic::error(
                    "capture application retains a runtime or unsupported evidence operand",
                ));
            }
        }
    }
    for contract in captured_machine_contracts(program, contracts, capture)? {
        bytes.extend(contract);
    }
    match &capture.accepted_template_commitment {
        Some(commitment) => {
            bytes.push(1);
            encode_identity_text(commitment, &mut bytes);
        }
        None => bytes.push(0),
    }
    Ok(bytes)
}

/// Encode one specialization's identity from the template contract and
/// arguments specialization retained plus the selected machines' checked
/// contracts. Template authority is produced here, not accepted from the
/// derivation loop; closed conformance inputs have their ordinary admission.
fn machine_specialization_identity(
    program: &dyn crate::CheckingInput,
    contracts: &checked_trees::MachineContractPlans,
    specialization: &typed_trees::typed_trees::MachineSpecialization,
    operational: &flow_effects::OperationalPlan,
    template_contract_bytes: &[u8],
    template_contract_commitment: typed_trees::typed_trees::MachineTemplateCommitment,
) -> Result<checked_trees::CheckedMachineSpecializationIdentity, Diagnostic> {
    let instance = program
        .machine_by_symbol(specialization.instance)
        .ok_or_else(|| Diagnostic::error("generic specialization lost its concrete instance"))?;
    let instance_identity = normalized_machine_identity(program, instance).ok_or_else(|| {
        Diagnostic::error("generic specialization instance has no normalized callable identity")
    })?;

    let mut machine_owner_identities = Vec::new();
    let mut machine_contract_commitments = Vec::new();
    for state_symbol in &specialization.machine_arguments {
        let owner = program
            .machines()
            .iter()
            .find(|machine| {
                typed_trees::state::state_by_symbol(program.machine_states(machine), *state_symbol).is_some()
            })
            .ok_or_else(|| Diagnostic::error(format!(
                "generic specialization references static machine symbol {:?}, but no owning machine exists",
                state_symbol
            )))?;
        let contract = contracts.for_machine(owner.symbol).ok_or_else(|| Diagnostic::error(
            format!(
                "generic specialization selected `{}`, but its normalized machine contract identity is missing",
                owner.name
            ),
        ))?;
        let owner_identity = normalized_machine_identity(program, owner).ok_or_else(|| {
            Diagnostic::error(format!(
                "generic specialization selected `{}`, but its owner identity cannot be normalized",
                owner.name
            ))
        })?;
        let selected_state_identity = conformance_symbol_identity(program, *state_symbol);
        machine_owner_identities.push(format!(
            "{owner_identity}|selected={selected_state_identity}"
        ));
        machine_contract_commitments.push(contract.commitment.as_bytes());
    }

    let mut bytes = Vec::new();
    encode_identity_bytes(template_contract_bytes, &mut bytes);
    bytes.extend(template_contract_commitment.as_bytes());
    encode_identity_text(&specialization.normalized_template_identity, &mut bytes);
    encode_identity_text(&instance_identity, &mut bytes);
    encode_identity_texts(&specialization.type_argument_identities, &mut bytes);
    encode_identity_texts(&specialization.const_argument_identities, &mut bytes);
    bytes.extend((machine_owner_identities.len() as u64).to_le_bytes());
    for (owner, commitment) in machine_owner_identities
        .iter()
        .zip(machine_contract_commitments.iter())
    {
        encode_identity_text(owner, &mut bytes);
        bytes.extend(commitment);
    }
    bytes.extend((specialization.conformance_applications.len() as u64).to_le_bytes());
    for application in &specialization.conformance_applications {
        bytes.extend(application.commitment.as_bytes());
    }
    let operator_realization_bytes = crate::canonical_closed_operator_realization_bytes(
        program,
        specialization.instance,
        &specialization.operator_realizations,
    )
    .map_err(Diagnostic::error)?;
    encode_identity_bytes(&operator_realization_bytes, &mut bytes);
    let static_call_bindings =
        crate::static_machine_call_binding_bytes(program, operational, specialization)?;
    encode_identity_bytes(&static_call_bindings, &mut bytes);
    match &specialization.accepted_template_commitment {
        Some(commitment) => {
            bytes.push(1);
            encode_identity_text(commitment, &mut bytes);
        }
        None => bytes.push(0),
    }
    let mut strong =
        crate::digest_encoder::DomainDigestEncoder::new(b"omega.machine-specialization.v2\0");
    strong.raw(&bytes);
    Ok(checked_trees::CheckedMachineSpecializationIdentity {
        instance: specialization.instance,
        template_contract_commitment,
        machine_argument_contract_commitments: machine_contract_commitments.into_boxed_slice(),
        commitment: typed_trees::typed_trees::MachineSpecializationCommitment::from_digest(
            strong.finish(),
        ),
    })
}

fn encode_identity_bytes(value: &[u8], bytes: &mut Vec<u8>) {
    bytes.extend((value.len() as u64).to_le_bytes());
    bytes.extend(value);
}

fn encode_identity_text(value: &str, bytes: &mut Vec<u8>) {
    encode_identity_bytes(value.as_bytes(), bytes);
}

fn encode_identity_texts(values: &[String], bytes: &mut Vec<u8>) {
    bytes.extend((values.len() as u64).to_le_bytes());
    for value in values {
        encode_identity_text(value, bytes);
    }
}

/// Alpha-normalized contract payload of one generic binder. Property demands
/// are deliberately separate: providers may weaken those rather than match
/// them. Callers must compare binder categories before comparing payloads.
pub fn generic_parameter_contract_bytes(
    program: &dyn crate::CheckingInput,
    parameter: &typed_trees::data::TypeParameter,
    binders: &[(SymbolHandle, String)],
    content_conservation: &[crate::ContentConservationSourcePlan],
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::new();
    encode_generic_parameter_contract(
        program,
        &parameter.kind,
        binders,
        content_conservation,
        &mut bytes,
    )?;
    Ok(bytes)
}

fn encode_generic_parameter_contract(
    program: &dyn crate::CheckingInput,
    kind: &TypeParameterKind,
    binders: &[(SymbolHandle, String)],
    content_conservation: &[crate::ContentConservationSourcePlan],
    bytes: &mut Vec<u8>,
) -> Result<(), Diagnostic> {
    match kind {
        TypeParameterKind::Const { type_reference }
        | TypeParameterKind::Value { type_reference } => encode_identity_text(
            &program.type_identity_request_text(TypeIdentityRequest {
                binders,
                ..TypeIdentityRequest::ordinary(*type_reference)
            }),
            bytes,
        ),
        TypeParameterKind::Machine { contract } => match program
            .machine_parameter_contract_view(contract)
            .ok_or_else(|| {
                Diagnostic::error(
                    "typed machine-parameter contract has no retained requirement identity",
                )
            })? {
            typed_trees::data::MachineParameterContractView::Structural(signature) => {
                bytes.push(1);
                encode_state_signature(program, signature, binders, content_conservation, bytes);
            }
            typed_trees::data::MachineParameterContractView::Nominal {
                trait_definition,
                requirement,
            } => {
                bytes.push(2);
                let identity =
                    program.trait_requirement_callable_identity_text(trait_definition, requirement);
                encode_identity_text(&identity, bytes);
                let typed_trees::data::MachineParameterContract::Nominal {
                    trait_arguments, ..
                } = contract
                else {
                    return Err(Diagnostic::error(
                        "nominal machine-parameter view lost its authored trait application",
                    ));
                };
                // The declaration does not identify its application: even a
                // phantom trait argument carries exact nominal authority.
                bytes.extend((trait_arguments.len() as u64).to_le_bytes());
                for argument in trait_arguments {
                    encode_identity_text(
                        &program.type_identity_request_text(TypeIdentityRequest {
                            binders,
                            ..TypeIdentityRequest::ordinary(*argument)
                        }),
                        bytes,
                    );
                }
                let requirement_binders = type_parameter_binders(
                    program.trait_type_parameters(trait_definition),
                    binders,
                );
                encode_state_signature(
                    program,
                    requirement,
                    &requirement_binders,
                    content_conservation,
                    bytes,
                );
            }
        },
        TypeParameterKind::Proposition { contract } => {
            bytes.extend((contract.parameters.len() as u32).to_le_bytes());
            let proposition_parameters =
                program.state_parameters.span_or_empty(contract.parameters);
            let proposition_binders = formal_parameter_binders(proposition_parameters, binders);
            for parameter in proposition_parameters {
                encode_parameter(program, parameter, &proposition_binders, bytes);
            }
        }
        TypeParameterKind::Type | TypeParameterKind::Implementation => {}
    }
    Ok(())
}

fn encode_data_properties(properties: typed_trees::data::DataProperties, output: &mut Vec<u8>) {
    output.push(match properties.multiplicity {
        language_semantics::Multiplicity::Unrestricted => 1,
        language_semantics::Multiplicity::Affine => 2,
        language_semantics::Multiplicity::Linear => 3,
    });
    if let Some(carry) = properties.carry {
        output.extend(format!("{carry}").as_bytes());
    }
    output.push(0);
}

/// Generic slots retain their kind and position across nested signatures.
/// A retained outer symbol keeps its original slot rather than consuming a
/// second position when a nested telescope carries that declaration again.
fn type_parameter_binders(
    parameters: &[typed_trees::data::TypeParameter],
    enclosing_binders: &[(SymbolHandle, String)],
) -> Vec<(SymbolHandle, String)> {
    let mut binders = enclosing_binders.to_vec();
    for parameter in parameters {
        if binders
            .iter()
            .any(|(symbol, _)| *symbol == parameter.symbol)
        {
            continue;
        }
        let prefix = match parameter.kind {
            TypeParameterKind::Type => "T",
            TypeParameterKind::Const { .. } => "C",
            TypeParameterKind::Value { .. } => "V",
            TypeParameterKind::Machine { .. } => "M",
            TypeParameterKind::Proposition { .. } => "P",
            TypeParameterKind::Implementation => "I",
        };
        let binder_position = binders.len();
        binders.push((parameter.symbol, format!("${prefix}{binder_position}")));
    }
    binders
}

/// Formal subjects occupy a distinct namespace from generic proposition slots.
fn formal_parameter_binders(
    parameters: &[typed_trees::signature::StateParameter],
    enclosing_binders: &[(SymbolHandle, String)],
) -> Vec<(SymbolHandle, String)> {
    let mut binders = enclosing_binders.to_vec();
    binders.extend(
        parameters
            .iter()
            .enumerate()
            .map(|(parameter_position, parameter)| {
                (parameter.symbol, format!("$argument{parameter_position}"))
            }),
    );
    binders
}

fn encode_state_signature(
    program: &dyn crate::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
    binders: &[(SymbolHandle, String)],
    content_conservation: &[crate::ContentConservationSourcePlan],
    output: &mut Vec<u8>,
) {
    let generic_binders =
        type_parameter_binders(program.state_signature_type_parameters(signature), binders);
    let parameters = program.state_signature_parameters(signature);
    let contract_binders = formal_parameter_binders(parameters, &generic_binders);
    output.extend((parameters.len() as u64).to_le_bytes());
    for parameter in parameters {
        encode_parameter(program, parameter, &contract_binders, output);
    }
    encode_identity_text(
        &program.type_identity_request_text(TypeIdentityRequest {
            binders: &contract_binders,
            ..TypeIdentityRequest::ordinary(signature.return_type)
        }),
        output,
    );
    for service in program
        .service_reach_rows
        .services(signature.service_reach_row)
    {
        let service = program
            .service_reaches
            .definition(*service)
            .expect("normalized signature service row references a registered service");
        output.extend(service.name.as_bytes());
        output.push(0);
    }
    output.push(u8::from(signature.service_reach_is_installation_bound));
    output.push(u8::from(signature.suspends));
    output.push(u8::from(signature.blocks));
    let contracts = contract_structure::declared_contracts(
        program,
        program.state_signature_contracts(signature),
        &contract_binders,
        content_conservation,
        true,
    );
    output.extend(
        program
            .type_identities
            .with(|table| checked_trees::contract_declarations_commitment(&contracts, table))
            .as_bytes(),
    );
    let termination = contract_structure::declared_termination(
        program,
        &language_semantics::TerminationInterface::Published(
            signature.termination_guarantee.clone(),
        ),
        &contract_binders,
    );
    output.extend(checked_trees::declared_termination_commitment(&termination).as_bytes());
}

/// Encode a retained signature with its enclosing trait's positional binders.
pub fn canonical_state_signature_bytes(
    program: &dyn crate::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
) -> Vec<u8> {
    let binders = program
        .traits()
        .iter()
        .find(|definition| {
            program
                .trait_machine_signatures(definition)
                .iter()
                .any(|requirement| requirement.symbol == signature.symbol)
        })
        .map(|definition| type_parameter_binders(program.trait_type_parameters(definition), &[]))
        .unwrap_or_default();
    let content_conservation = crate::content_conservation_plans(program);
    let mut bytes = Vec::new();
    encode_state_signature(
        program,
        signature,
        &binders,
        &content_conservation,
        &mut bytes,
    );
    bytes
}

/// Complete original callable schema, independent of closed applications.
/// Unlike an overload key, this includes binder categories, demands, domains,
/// predicates and the public operational/termination contract.
pub fn canonical_requirement_schema_bytes(
    program: &dyn crate::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
) -> Result<Vec<u8>, Diagnostic> {
    let binders = crate::contract_structure::signature_generic_binders(program, signature);
    let content = crate::content_conservation_plans(program);
    let mut bytes = b"omega.requirement-schema.v1\0".to_vec();
    let signature_bytes = canonical_state_signature_bytes(program, signature);
    bytes.extend((signature_bytes.len() as u64).to_le_bytes());
    bytes.extend(signature_bytes);
    bytes.extend((signature.lifetime_parameters.len() as u64).to_le_bytes());
    let parameters = program.state_signature_type_parameters(signature);
    bytes.extend((parameters.len() as u64).to_le_bytes());
    for parameter in parameters {
        bytes.push(match parameter.kind {
            TypeParameterKind::Type => 1,
            TypeParameterKind::Const { .. } => 2,
            TypeParameterKind::Value { .. } => 3,
            TypeParameterKind::Machine { .. } => 4,
            TypeParameterKind::Implementation => 5,
            TypeParameterKind::Proposition { .. } => 6,
        });
        encode_data_properties(parameter.bounds, &mut bytes);
        let contract = generic_parameter_contract_bytes(program, parameter, &binders, &content)?;
        bytes.extend((contract.len() as u64).to_le_bytes());
        bytes.extend(contract);
    }
    let predicates = program
        .proof_facts
        .span_or_empty(signature.where_facts)
        .iter()
        .map(|fact| checked_trees::DeclaredContractClause {
            kind: checked_trees::DeclaredContractKind::Requires,
            fact: Some(crate::contract_structure::declared_fact(
                program, fact, &binders, &content,
            )),
        })
        .collect::<Vec<_>>();
    bytes.extend(
        program
            .type_identities
            .with(|table| checked_trees::contract_declarations_commitment(&predicates, table))
            .as_bytes(),
    );
    Ok(bytes)
}

fn encode_state_shape(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    content_conservation: &[crate::ContentConservationSourcePlan],
    output: &mut Vec<u8>,
) {
    let parameters = program.state_parameters(state);
    let contract_binders = formal_parameter_binders(parameters, binders);
    output.extend((parameters.len() as u64).to_le_bytes());
    for parameter in parameters {
        encode_parameter(program, parameter, &contract_binders, output);
    }
    encode_identity_text(
        &program.type_identity_request_text(TypeIdentityRequest {
            binders: &contract_binders,
            ..TypeIdentityRequest::ordinary(state.return_type)
        }),
        output,
    );
    let contracts = contract_structure::declared_contracts(
        program,
        program.state_contracts(state),
        &contract_binders,
        content_conservation,
        true,
    );
    output.extend(
        program
            .type_identities
            .with(|table| checked_trees::contract_declarations_commitment(&contracts, table))
            .as_bytes(),
    );
}

fn encode_parameter(
    program: &dyn crate::CheckingInput,
    parameter: &typed_trees::signature::StateParameter,
    binders: &[(SymbolHandle, String)],
    output: &mut Vec<u8>,
) {
    output.push(u8::from(parameter.is_mutable));
    output.push(u8::from(parameter.is_const));
    output.push(match parameter.relevance {
        language_core::BindingRelevance::Relevant => 0,
        language_core::BindingRelevance::Erased => 1,
    });
    encode_identity_text(
        &program.type_identity_request_text(TypeIdentityRequest {
            binders,
            ..TypeIdentityRequest::ordinary(parameter.type_reference)
        }),
        output,
    );
}

/// Fold selected argument reports in the published template and argument order.
pub fn specialization_selection_report_fingerprint(
    template: &str,
    type_arguments: &[String],
    const_arguments: &[String],
    machine_arguments: &[String],
    evidence_arguments: &[String],
) -> u64 {
    let mut hash = semantic_vocabulary::Fnv1a::new();
    for part in std::iter::once(template)
        .chain(type_arguments.iter().map(String::as_str))
        .chain(const_arguments.iter().map(String::as_str))
        .chain(machine_arguments.iter().map(String::as_str))
        .chain(evidence_arguments.iter().map(String::as_str))
    {
        hash.bytes_terminated(part.as_bytes(), 0xff);
    }
    hash.finish()
}

pub(crate) fn conformance_symbol_identity(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> String {
    let path = program.symbols.display_path(symbol, "::");
    let Some(package) = program.symbols.symbol_package_identity(symbol) else {
        return format!("unmanaged::{path}");
    };
    let digest = package.digest();
    let mut owner = String::with_capacity(digest.len() * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest {
        owner.push(char::from(HEX[usize::from(byte >> 4)]));
        owner.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    format!("package:{owner}::{path}")
}

/// Join the declaration's package identity and path to its callable identity.
/// Return absence when the retained callable cannot be normalized.
pub fn normalized_machine_identity(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Option<String> {
    let declaration = conformance_symbol_identity(program, machine.symbol);
    let overload = program.machine_callable_identity_text(machine)?;
    Some(format!("{declaration}|{overload}"))
}
