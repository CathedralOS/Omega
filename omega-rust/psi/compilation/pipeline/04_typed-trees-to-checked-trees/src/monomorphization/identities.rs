//! Canonical template and application commitments. Contract clauses use the
//! shared declared-contract projection; type leaves are normalized by symbol
//! before length framing. Diagnostic spelling never supplies binder identity.
use super::{Diagnostic, Sha256, SymbolHandle, TypeParameterKind, TypedTrees};
use crate::monomorphization::conformance_symbol_identity;
use crate::monomorphization::normalized_machine_identity;
use sha2::Digest;
use typed_trees::type_identity::TypeIdentityRequest;

/// The universal template contract shared by every concrete application.
/// Capture it from the retained authored declaration, not a selected clone.
/// This encoding is binder-positional:
/// renaming a type, machine, or value parameter does not change the identity.
pub(super) fn canonical_template_contract_bytes(
    program: &dyn ::validation::CheckingInput,
    machine_index: usize,
    reach_inference: &flow_effects::ServiceReachInferencePlan,
) -> Result<Vec<u8>, Diagnostic> {
    let machine = &program.machines()[machine_index];
    let parameters = program.machine_type_parameters(machine);
    let binders = type_parameter_binders(parameters, &[]);
    let content_conservation = validation::content_conservation_plans(program);
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
        });
        bytes.extend((index as u32).to_le_bytes());
        encode_data_properties(parameter.bounds, &mut bytes);
        match &parameter.kind {
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => encode_identity_text(
                program
                    .type_identity(TypeIdentityRequest {
                        binders: &binders,
                        ..TypeIdentityRequest::ordinary(*type_reference)
                    })
                    .as_str(),
                &mut bytes,
            ),
            TypeParameterKind::Machine { contract } => match program
                .machine_parameter_contract_view(contract)
                .expect("typed machine-parameter contract must retain a valid requirement identity")
            {
                typed_trees::data::MachineParameterContractView::Structural(signature) => {
                    bytes.push(1);
                    encode_state_signature(
                        program,
                        signature,
                        &binders,
                        &content_conservation,
                        &mut bytes,
                    );
                }
                typed_trees::data::MachineParameterContractView::Nominal {
                    trait_definition,
                    requirement,
                } => {
                    bytes.push(2);
                    let identity = program
                        .trait_requirement_callable_identity_text(trait_definition, requirement);
                    encode_identity_text(&identity, &mut bytes);
                    let requirement_binders = type_parameter_binders(
                        program.trait_type_parameters(trait_definition),
                        &binders,
                    );
                    encode_state_signature(
                        program,
                        requirement,
                        &requirement_binders,
                        &content_conservation,
                        &mut bytes,
                    );
                }
            },
            TypeParameterKind::Proposition { contract } => {
                bytes.extend((contract.parameters.len() as u32).to_le_bytes());
                let proposition_parameters =
                    program.state_parameters.span_or_empty(contract.parameters);
                let proposition_binders =
                    formal_parameter_binders(proposition_parameters, &binders);
                for parameter in proposition_parameters {
                    bytes.push(u8::from(parameter.is_mutable));
                    encode_identity_text(
                        program
                            .type_identity(TypeIdentityRequest {
                                binders: &proposition_binders,
                                ..TypeIdentityRequest::ordinary(parameter.type_reference)
                            })
                            .as_str(),
                        &mut bytes,
                    );
                }
            }
            TypeParameterKind::Type => {}
        }
        bytes.push(0xfe);
    }
    let mut conformance_bounds = Vec::new();
    for bound in &machine.conformance_bounds {
        let mut encoded = Vec::new();
        encoded.extend(bound.trait_name.as_str().as_bytes());
        encoded.push(0);
        for argument in &bound.arguments {
            encode_identity_text(
                program
                    .type_identity(TypeIdentityRequest {
                        binders: &binders,
                        ..TypeIdentityRequest::ordinary(*argument)
                    })
                    .as_str(),
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
    let mut service_reaches: Vec<_> = program
        .service_reach_rows
        .services(machine.service_reach_row)
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
            || !program
                .service_reach_rows
                .services(machine.service_reach_row)
                .is_empty(),
    ));
    bytes.push(u8::from(machine.service_reach_is_installation_bound));
    bytes.push(u8::from(machine.suspends));
    bytes.push(u8::from(machine.blocks));
    let contract_binders = program
        .machine_states(machine)
        .first()
        .map(|state| formal_parameter_binders(program.state_parameters(state), &binders))
        .unwrap_or_else(|| binders.clone());
    let contracts = crate::facts::contract_structure::declared_contracts(
        program,
        program.machine_contracts(machine),
        &contract_binders,
        &content_conservation,
        true,
    );
    bytes.extend(checked_trees::contract_declarations_commitment(&contracts).as_bytes());
    let termination = crate::facts::contract_structure::declared_termination(
        program,
        &machine.termination_plan.interface,
        &contract_binders,
    );
    bytes.extend(checked_trees::declared_termination_commitment(&termination).as_bytes());
    bytes.extend(validation::static_machine_template_reach_contract_bytes(
        program,
        reach_inference,
        machine,
    )?);
    Ok(bytes)
}

fn fnv1a_report_fingerprint(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    bytes.iter().fold(OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(PRIME)
    })
}

pub(super) fn machine_template_commitment(
    canonical_template_contract_bytes: &[u8],
) -> typed_trees::typed_trees::MachineTemplateCommitment {
    let mut strong = Sha256::new();
    strong.update(b"omega.machine-template.v2\0");
    strong.update(canonical_template_contract_bytes);
    typed_trees::typed_trees::MachineTemplateCommitment::from_digest(strong.finalize().into())
}

/// Deterministic identity of an authored generic machine declaration before
/// monomorphization consumes its binders. Trust receipts and separate-
/// compilation caches use this same identity; callers cannot substitute the
/// identity of one concrete instance for the universal template grant.
pub fn generic_machine_template_report_fingerprint(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
) -> Option<u64> {
    let bytes = canonical_template_bytes_for_symbol(program, machine_symbol)?;
    Some(fnv1a_report_fingerprint(&bytes))
}

/// Canonical contract bytes for `machine_symbol`'s universal template —
/// `None` when the machine carries no template, `Err` flattened to `None`
/// when the reachability computation cannot encode one.
fn canonical_template_bytes_for_symbol(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
) -> Option<Vec<u8>> {
    let machine_index = generic_machine_index(program, machine_symbol)?;
    let operational = validation::infer_operational_may(program);
    let service_reaches = validation::infer_service_reaches(program, &operational);
    canonical_template_contract_bytes(program, machine_index, &service_reaches).ok()
}

/// Index of `machine_symbol` when the machine declares type parameters —
/// a parameterless machine carries no template to commit.
fn generic_machine_index(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
) -> Option<usize> {
    let machine_index = program
        .machines()
        .iter()
        .position(|machine| machine.symbol == machine_symbol)?;
    (!program
        .machine_type_parameters(&program.machines()[machine_index])
        .is_empty())
    .then_some(machine_index)
}

/// Domain-separated strong commitment to the exact canonical universal
/// template captured before monomorphization consumes its binders.
pub fn generic_machine_template_commitment(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
) -> Option<typed_trees::typed_trees::MachineTemplateCommitment> {
    let bytes = canonical_template_bytes_for_symbol(program, machine_symbol)?;
    Some(machine_template_commitment(&bytes))
}

pub(super) fn accepted_template_commitment(
    program: &dyn ::validation::CheckingInput,
    machine_index: usize,
) -> Option<String> {
    let machine = &program.machines()[machine_index];
    (machine.supply_mode == language_semantics::MachineSupplyMode::AdmissionClaim)
        .then(|| machine.name.as_str().to_owned())
}

/// MP5: specialization runs before checked contract plans exist, so it
/// leaves each instance's selected machine-contract commitments and its
/// specialization commitment empty. This is their one computation: read the
/// selected contracts from the plans and fold them, with the identities
/// specialization recorded, into the commitment Terminal Psi and package
/// review carry.
pub(crate) fn prepare_specialization_contract_identities(
    program: &dyn ::validation::CheckingInput,
    contracts: &checked_trees::MachineContractPlans,
) -> Result<checked_trees::CheckedSpecializationIdentities, Vec<Diagnostic>> {
    let operational = validation::infer_operational_may(program);
    let identities: Result<Vec<_>, _> = program
        .machine_specializations
        .iter()
        .map(|specialization| {
            machine_specialization_identity(program, contracts, specialization, &operational)
        })
        .collect();
    identities
        .map(checked_trees::CheckedSpecializationIdentities::with_rows)
        .map_err(|diagnostic| vec![diagnostic])
}

/// Encode one specialization's identity from the template contract and
/// arguments specialization recorded plus the selected machines' contract
/// identities. Specialization already validated the template commitment,
/// the accepted-template custody and the closed conformance applications it
/// recorded; this reads them as given.
fn machine_specialization_identity(
    program: &dyn ::validation::CheckingInput,
    contracts: &checked_trees::MachineContractPlans,
    specialization: &typed_trees::typed_trees::MachineSpecialization,
    operational: &flow_effects::OperationalPlan,
) -> Result<checked_trees::CheckedMachineSpecializationIdentity, Diagnostic> {
    let instance = crate::lookup::machine_by_symbol(program, specialization.instance)
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
                program
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == *state_symbol)
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
    encode_identity_bytes(
        &specialization.canonical_template_contract_bytes,
        &mut bytes,
    );
    bytes.extend(specialization.template_contract_commitment.as_bytes());
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
    let operator_realization_bytes = validation::canonical_closed_operator_realization_bytes(
        program,
        specialization.instance,
        &specialization.operator_realizations,
    )
    .map_err(Diagnostic::error)?;
    encode_identity_bytes(&operator_realization_bytes, &mut bytes);
    let static_call_bindings =
        validation::static_machine_call_binding_bytes(program, operational, specialization)?;
    encode_identity_bytes(&static_call_bindings, &mut bytes);
    match &specialization.accepted_template_commitment {
        Some(commitment) => {
            bytes.push(1);
            encode_identity_text(commitment, &mut bytes);
        }
        None => bytes.push(0),
    }
    let mut strong = Sha256::new();
    strong.update(b"omega.machine-specialization.v2\0");
    strong.update(&bytes);
    Ok(checked_trees::CheckedMachineSpecializationIdentity {
        instance: specialization.instance,
        machine_argument_contract_commitments: machine_contract_commitments.into_boxed_slice(),
        commitment: typed_trees::typed_trees::MachineSpecializationCommitment::from_digest(
            strong.finalize().into(),
        ),
    })
}

pub(super) fn encode_identity_bytes(value: &[u8], bytes: &mut Vec<u8>) {
    bytes.extend((value.len() as u64).to_le_bytes());
    bytes.extend(value);
}

pub(super) fn encode_identity_text(value: &str, bytes: &mut Vec<u8>) {
    encode_identity_bytes(value.as_bytes(), bytes);
}

pub(super) fn encode_identity_texts(values: &[String], bytes: &mut Vec<u8>) {
    bytes.extend((values.len() as u64).to_le_bytes());
    for value in values {
        encode_identity_text(value, bytes);
    }
}

pub(super) fn encode_data_properties(
    properties: typed_trees::data::DataProperties,
    output: &mut Vec<u8>,
) {
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

pub(super) fn encode_state_signature(
    program: &dyn ::validation::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
    binders: &[(SymbolHandle, String)],
    content_conservation: &[validation::ContentConservationSourcePlan],
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
        program
            .type_identity(TypeIdentityRequest {
                binders: &contract_binders,
                ..TypeIdentityRequest::ordinary(signature.return_type)
            })
            .as_str(),
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
    let contracts = crate::facts::contract_structure::declared_contracts(
        program,
        program.state_signature_contracts(signature),
        &contract_binders,
        content_conservation,
        true,
    );
    output.extend(checked_trees::contract_declarations_commitment(&contracts).as_bytes());
    let termination = crate::facts::contract_structure::declared_termination(
        program,
        &language_semantics::TerminationInterface::Published(
            signature.termination_guarantee.clone(),
        ),
        &contract_binders,
    );
    output.extend(checked_trees::declared_termination_commitment(&termination).as_bytes());
}

pub(crate) fn canonical_state_signature_bytes(
    program: &dyn ::validation::CheckingInput,
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
    let content_conservation = validation::content_conservation_plans(program);
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

pub(super) fn encode_state_shape(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    content_conservation: &[validation::ContentConservationSourcePlan],
    output: &mut Vec<u8>,
) {
    let parameters = program.state_parameters(state);
    let contract_binders = formal_parameter_binders(parameters, binders);
    output.extend((parameters.len() as u64).to_le_bytes());
    for parameter in parameters {
        encode_parameter(program, parameter, &contract_binders, output);
    }
    encode_identity_text(
        program
            .type_identity(TypeIdentityRequest {
                binders: &contract_binders,
                ..TypeIdentityRequest::ordinary(state.return_type)
            })
            .as_str(),
        output,
    );
    let contracts = crate::facts::contract_structure::declared_contracts(
        program,
        program.state_contracts(state),
        &contract_binders,
        content_conservation,
        true,
    );
    output.extend(checked_trees::contract_declarations_commitment(&contracts).as_bytes());
}

pub(super) fn encode_parameter(
    program: &dyn ::validation::CheckingInput,
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
        program
            .type_identity(TypeIdentityRequest {
                binders,
                ..TypeIdentityRequest::ordinary(parameter.type_reference)
            })
            .as_str(),
        output,
    );
}

pub(super) fn specialization_selection_report_fingerprint(
    template: &str,
    type_arguments: &[String],
    const_arguments: &[String],
    machine_arguments: &[String],
    evidence_arguments: &[String],
) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    for part in std::iter::once(template)
        .chain(type_arguments.iter().map(String::as_str))
        .chain(const_arguments.iter().map(String::as_str))
        .chain(machine_arguments.iter().map(String::as_str))
        .chain(evidence_arguments.iter().map(String::as_str))
    {
        for byte in part.as_bytes().iter().copied().chain(std::iter::once(0xff)) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(PRIME);
        }
    }
    hash
}
