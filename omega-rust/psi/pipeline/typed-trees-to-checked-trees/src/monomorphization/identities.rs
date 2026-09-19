//! Canonical template and application commitments, including independent replay.
use super::{
    Diagnostic, Sha256, StaticMachineArgument, SymbolHandle, SymbolKind, TypeParameterKind,
    TypedTrees,
};
use crate::monomorphization::conformance_symbol_identity;
use crate::monomorphization::normalized_machine_identity;
use sha2::Digest;
use typed_trees::type_identity::TypeIdentityRequest;

pub(super) fn encode_bound_static_argument(
    program: &TypedTrees,
    argument: &StaticMachineArgument,
    lifetime_binders: &[(String, String)],
    static_binders: &[(SymbolHandle, String)],
    bytes: &mut Vec<u8>,
) {
    if argument.type_reference.is_valid() {
        bytes.push(4);
        encode_normalized_text(
            program
                .type_identity(TypeIdentityRequest {
                    binders: static_binders,
                    ..TypeIdentityRequest::ordinary(argument.type_reference)
                })
                .as_str(),
            lifetime_binders,
            bytes,
        );
        return;
    }
    if let Some(literal) = &argument.const_literal {
        bytes.push(1);
        let text = literal.text();
        bytes.extend((text.len() as u64).to_le_bytes());
        bytes.extend(text.as_bytes());
        return;
    }
    if let Some(projection) = &argument.evidence_projection {
        bytes.push(2);
        bytes.extend(projection.term.as_str().as_bytes());
        bytes.push(0);
        bytes.extend(projection.member.as_str().as_bytes());
        return;
    }

    bytes.push(3);
    if let Some((_, identity)) = static_binders
        .iter()
        .find(|(symbol, _)| *symbol == argument.symbol)
    {
        bytes.extend(identity.as_bytes());
    } else if argument.symbol.is_valid()
        && matches!(
            program.symbols.get(argument.symbol).kind,
            SymbolKind::Conformance
        )
    {
        bytes.extend(conformance_symbol_identity(program, argument.symbol).as_bytes());
    } else {
        bytes.extend(argument.display_name().as_bytes());
    }
    let Some(application) = &argument.application else {
        bytes.push(0);
        return;
    };
    bytes.push(1);
    bytes.extend((application.lifetime_arguments.len() as u64).to_le_bytes());
    for lifetime in &application.lifetime_arguments {
        encode_normalized_text(&format!("'{}", lifetime.as_str()), lifetime_binders, bytes);
        bytes.push(0);
    }
    bytes.extend((application.arguments.len() as u64).to_le_bytes());
    for nested in &application.arguments {
        encode_bound_static_argument(program, nested, lifetime_binders, static_binders, bytes);
        bytes.push(0xfe);
    }
}

/// The universal template contract shared by every concrete application.
/// Capture it from the retained authored declaration, not a selected clone.
/// This encoding is binder-positional:
/// renaming a type, machine, or value parameter does not change the identity.
pub(super) fn canonical_template_contract_bytes(
    program: &TypedTrees,
    machine_index: usize,
    reach_inference: &flow_effects::ServiceReachInferencePlan,
) -> Result<Vec<u8>, Diagnostic> {
    let machine = &program.machines()[machine_index];
    let parameters = program.machine_type_parameters(machine);
    let binders: Vec<(String, String)> = parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let prefix = match parameter.kind {
                TypeParameterKind::Type => "T",
                TypeParameterKind::Const { .. } => "C",
                TypeParameterKind::Value { .. } => "V",
                TypeParameterKind::Machine { .. } => "M",
                TypeParameterKind::Proposition { .. } => "P",
            };
            (
                parameter.name.as_str().to_owned(),
                format!("${prefix}{index}"),
            )
        })
        .collect();
    let type_binders: Vec<(SymbolHandle, String)> = parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let prefix = match parameter.kind {
                TypeParameterKind::Type => "T",
                TypeParameterKind::Const { .. } => "C",
                TypeParameterKind::Value { .. } => "V",
                TypeParameterKind::Machine { .. } => "M",
                TypeParameterKind::Proposition { .. } => "P",
            };
            (parameter.symbol, format!("${prefix}{index}"))
        })
        .collect();
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
            | TypeParameterKind::Value { type_reference } => encode_normalized_text(
                program
                    .type_identity(TypeIdentityRequest {
                        binders: &type_binders,
                        ..TypeIdentityRequest::ordinary(*type_reference)
                    })
                    .as_str(),
                &binders,
                &mut bytes,
            ),
            TypeParameterKind::Machine { contract } => match program
                .machine_parameter_contract_view(contract)
                .expect("typed machine-parameter contract must retain a valid requirement identity")
            {
                typed_trees::data::MachineParameterContractView::Structural(signature) => {
                    bytes.push(1);
                    encode_state_signature(program, signature, &binders, &type_binders, &mut bytes);
                }
                typed_trees::data::MachineParameterContractView::Nominal {
                    trait_definition,
                    requirement,
                } => {
                    bytes.push(2);
                    let identity = program
                        .normalized_trait_requirement_overload_identity(
                            trait_definition,
                            requirement,
                        )
                        .identity();
                    bytes.extend((identity.len() as u64).to_le_bytes());
                    bytes.extend(identity.as_bytes());
                    encode_state_signature(
                        program,
                        requirement,
                        &binders,
                        &type_binders,
                        &mut bytes,
                    );
                }
            },
            TypeParameterKind::Proposition { contract } => {
                bytes.extend((contract.parameters.len() as u32).to_le_bytes());
                for parameter in program.state_parameters.span_or_empty(contract.parameters) {
                    bytes.push(u8::from(parameter.is_mutable));
                    bytes.push(u8::from(parameter.is_self));
                    encode_normalized_text(
                        program
                            .type_identity(TypeIdentityRequest {
                                binders: &type_binders,
                                ..TypeIdentityRequest::ordinary(parameter.type_reference)
                            })
                            .as_str(),
                        &binders,
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
        let subject_index = parameters
            .iter()
            .position(|parameter| parameter.symbol == bound.subject)
            .unwrap_or(usize::MAX);
        encoded.extend((subject_index as u64).to_le_bytes());
        if bound.binder.is_some() {
            encoded.push(3);
            encoded.extend(bound.carrier_name.as_str().as_bytes());
        } else if let Some(selected) = &bound.selected_conformance {
            encoded.push(2);
            encoded.extend(bound.carrier_name.as_str().as_bytes());
            encoded.push(0);
            if let Some(name) = bound.selected_conformance_name() {
                encoded.extend(name.as_str().as_bytes());
            }
            encoded.push(0);
            encode_bound_static_argument(program, selected, &binders, &type_binders, &mut encoded);
        } else {
            encoded.push(1);
            encoded.extend(bound.carrier_name.as_str().as_bytes());
        }
        encoded.push(0);
        for argument in &bound.arguments {
            encode_normalized_text(
                program
                    .type_identity(TypeIdentityRequest {
                        binders: &type_binders,
                        ..TypeIdentityRequest::ordinary(*argument)
                    })
                    .as_str(),
                &binders,
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
        encode_state_shape(program, state, &binders, &type_binders, &mut shape);
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
    let mut contract_binders = binders.clone();
    if let Some(state) = program.machine_states(machine).first() {
        contract_binders.extend(
            program
                .state_parameters(state)
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    (parameter.name.as_str().to_owned(), format!("$P{index}"))
                }),
        );
    }
    let contracts = encode_contract_set(
        program,
        program.machine_contracts(machine),
        &contract_binders,
    );
    for contract in contracts {
        bytes.extend(contract);
        bytes.push(0xfc);
    }
    match &machine.termination_plan.interface {
        language_semantics::TerminationInterface::InternalDerived => bytes.push(0),
        language_semantics::TerminationInterface::Published(
            language_semantics::TerminationGuarantee::NoGuarantee,
        ) => bytes.push(1),
        language_semantics::TerminationInterface::Published(
            language_semantics::TerminationGuarantee::Terminates { premises },
        ) => {
            bytes.push(2);
            let parameter_symbols = program
                .machine_states(machine)
                .first()
                .map(|state| {
                    program
                        .state_parameters(state)
                        .iter()
                        .map(|parameter| parameter.symbol)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            encode_progress_premises(premises, &parameter_symbols, &mut bytes);
        }
    }
    bytes.extend(validation::static_machine_template_reach_contract_bytes(
        program,
        reach_inference,
        machine,
    )?);
    Ok(bytes)
}

pub(super) fn fnv1a_report_fingerprint(bytes: &[u8]) -> u64 {
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
    strong.update(b"omega.machine-template.v1\0");
    strong.update(canonical_template_contract_bytes);
    typed_trees::typed_trees::MachineTemplateCommitment::from_digest(strong.finalize().into())
}

/// Deterministic identity of an authored generic machine declaration before
/// monomorphization consumes its binders. Trust receipts and separate-
/// compilation caches use this same identity; callers cannot substitute the
/// identity of one concrete instance for the universal template grant.
pub fn generic_machine_template_report_fingerprint(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
) -> Option<u64> {
    let machine_index = program
        .machines()
        .iter()
        .position(|machine| machine.symbol == machine_symbol)?;
    if program
        .machine_type_parameters(&program.machines()[machine_index])
        .is_empty()
    {
        return None;
    }
    let operational = validation::infer_operational_may(program);
    let service_reaches = validation::infer_service_reaches(program, &operational);
    let bytes = canonical_template_contract_bytes(program, machine_index, &service_reaches).ok()?;
    Some(fnv1a_report_fingerprint(&bytes))
}

/// Domain-separated strong commitment to the exact canonical universal
/// template captured before monomorphization consumes its binders.
pub fn generic_machine_template_commitment(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
) -> Option<typed_trees::typed_trees::MachineTemplateCommitment> {
    let machine_index = program
        .machines()
        .iter()
        .position(|machine| machine.symbol == machine_symbol)?;
    if program
        .machine_type_parameters(&program.machines()[machine_index])
        .is_empty()
    {
        return None;
    }
    let operational = validation::infer_operational_may(program);
    let service_reaches = validation::infer_service_reaches(program, &operational);
    let bytes = canonical_template_contract_bytes(program, machine_index, &service_reaches).ok()?;
    Some(machine_template_commitment(&bytes))
}

pub(super) fn accepted_template_commitment(
    program: &TypedTrees,
    machine_index: usize,
) -> Option<String> {
    let machine = &program.machines()[machine_index];
    (machine.supply_mode == language_semantics::MachineSupplyMode::AdmissionClaim)
        .then(|| machine.name.as_str().to_owned())
}

pub(crate) fn bind_specialization_contract_identities(
    program: &mut TypedTrees,
    contracts: &checked_trees::MachineContractPlans,
) -> Result<(), Vec<Diagnostic>> {
    let operational = validation::infer_operational_may(program);
    validation::validate_static_machine_call_contracts(program, &operational)
        .map_err(|diagnostic| vec![diagnostic])?;
    let updates: Result<Vec<_>, _> = program
        .machine_specializations
        .iter()
        .map(|specialization| {
            replay_machine_specialization_identity(program, contracts, specialization, &operational)
        })
        .collect();
    let updates = updates.map_err(|diagnostic| vec![diagnostic])?;
    for (specialization, replay) in program.machine_specializations.iter_mut().zip(updates) {
        if !specialization
            .machine_argument_contract_report_fingerprints
            .is_empty()
            && specialization.machine_argument_contract_report_fingerprints
                != replay.machine_contract_report_fingerprints
        {
            return Err(vec![Diagnostic::error(
                "generic specialization cache entry no longer matches its selected machine contract identities",
            )]);
        }
        if !specialization
            .machine_argument_contract_commitments
            .is_empty()
            && specialization.machine_argument_contract_commitments
                != replay.machine_contract_commitments
        {
            return Err(vec![Diagnostic::error(
                "generic specialization cache entry no longer matches its selected machine contract commitments",
            )]);
        }
        if !specialization
            .conformance_argument_report_fingerprints
            .is_empty()
            && specialization.conformance_argument_report_fingerprints
                != replay.conformance_report_fingerprints
        {
            return Err(vec![Diagnostic::error(
                "generic specialization cache entry no longer matches its selected conformance-map identities",
            )]);
        }
        if !specialization.commitment.is_zero() && specialization.commitment != replay.commitment {
            return Err(vec![Diagnostic::error(
                "generic specialization cache entry has a stale authoritative commitment",
            )]);
        }
        if specialization.report_fingerprint != 0
            && specialization.report_fingerprint != replay.report_fingerprint
        {
            return Err(vec![Diagnostic::error(
                "generic specialization cache entry has a stale aggregate report fingerprint",
            )]);
        }
        specialization.machine_argument_contract_report_fingerprints =
            replay.machine_contract_report_fingerprints;
        specialization.machine_argument_contract_commitments = replay.machine_contract_commitments;
        specialization.conformance_argument_report_fingerprints =
            replay.conformance_report_fingerprints;
        specialization.report_fingerprint = replay.report_fingerprint;
        specialization.commitment = replay.commitment;
    }
    Ok(())
}

struct ReplayedMachineSpecializationIdentity {
    machine_contract_report_fingerprints: Vec<u64>,
    machine_contract_commitments: Vec<[u8; 32]>,
    conformance_report_fingerprints: Vec<u64>,
    report_fingerprint: u64,
    commitment: typed_trees::typed_trees::MachineSpecializationCommitment,
}

/// Independently replay the authoritative commitment of one retained machine
/// specialization from its exact typed custody and checked contract plans.
pub fn recompute_machine_specialization_commitment(
    program: &TypedTrees,
    contracts: &checked_trees::MachineContractPlans,
    specialization: &typed_trees::typed_trees::MachineSpecialization,
) -> Result<typed_trees::typed_trees::MachineSpecializationCommitment, Diagnostic> {
    let operational = validation::infer_operational_may(program);
    validation::validate_static_machine_call_contracts(program, &operational)?;
    replay_machine_specialization_identity(program, contracts, specialization, &operational)
        .map(|replay| replay.commitment)
}

fn replay_machine_specialization_identity(
    program: &TypedTrees,
    contracts: &checked_trees::MachineContractPlans,
    specialization: &typed_trees::typed_trees::MachineSpecialization,
    operational: &flow_effects::OperationalPlan,
) -> Result<ReplayedMachineSpecializationIdentity, Diagnostic> {
    let template = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == specialization.template)
        .ok_or_else(|| Diagnostic::error("generic specialization lost its template machine"))?;
    match (
        template.supply_mode,
        &specialization.accepted_template_commitment,
    ) {
        (language_semantics::MachineSupplyMode::AdmissionClaim, None) => {
            return Err(Diagnostic::error(
                "accepted generic specialization lost its template trust commitment",
            ));
        }
        (language_semantics::MachineSupplyMode::AdmissionClaim, Some(commitment))
            if template.name.as_str() != commitment =>
        {
            return Err(Diagnostic::error(format!(
                "accepted generic specialization records template commitment `{commitment}`, but its template identity no longer matches"
            )));
        }
        (language_semantics::MachineSupplyMode::AdmissionClaim, Some(_)) => {}
        (mode, Some(commitment)) => {
            return Err(Diagnostic::error(format!(
                "generic specialization records accepted template commitment `{commitment}`, but the retained template supply mode is {mode:?}"
            )));
        }
        _ => {}
    }
    if specialization.template_contract_report_fingerprint == 0
        || specialization.canonical_template_contract_bytes.is_empty()
        || fnv1a_report_fingerprint(&specialization.canonical_template_contract_bytes)
            != specialization.template_contract_report_fingerprint
    {
        return Err(Diagnostic::error(
            "generic specialization is missing or mismatches its canonical pre-substitution template contract",
        ));
    }
    let expected_template_commitment =
        machine_template_commitment(&specialization.canonical_template_contract_bytes);
    if specialization.template_contract_commitment.is_zero()
        || specialization.template_contract_commitment != expected_template_commitment
    {
        return Err(Diagnostic::error(
            "generic specialization is missing or mismatches its authoritative template commitment",
        ));
    }
    if specialization.normalized_template_identity.is_empty() {
        return Err(Diagnostic::error(
            "generic specialization is missing its normalized template identity",
        ));
    }
    let instance = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == specialization.instance)
        .ok_or_else(|| Diagnostic::error("generic specialization lost its concrete instance"))?;
    let instance_identity = normalized_machine_identity(program, instance).ok_or_else(|| {
        Diagnostic::error("generic specialization instance has no normalized callable identity")
    })?;

    let mut machine_owner_identities = Vec::new();
    let mut machine_contract_report_fingerprints = Vec::new();
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
        if contract.commitment.is_zero() {
            return Err(Diagnostic::error(format!(
                "generic specialization selected `{}`, but its machine contract commitment is empty",
                owner.name
            )));
        }
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
        machine_contract_report_fingerprints.push(contract.report_fingerprint);
        machine_contract_commitments.push(contract.commitment.as_bytes());
    }

    let mut conformance_report_fingerprints = Vec::new();
    let mut conformance_commitments = Vec::new();
    for application in &specialization.conformance_applications {
        let conformance = program
            .conformances()
            .iter()
            .find(|conformance| conformance.symbol == application.declaration)
            .ok_or_else(|| Diagnostic::error(format!(
                "generic specialization references conformance symbol {:?}, but no package conformance exists",
                application.declaration
            )))?;
        if program.closed_conformance_rows(conformance).is_none() {
            return Err(Diagnostic::error(format!(
                "generic specialization selected `{}`, but it is not a closed conformance map",
                conformance
                    .alias
                    .as_ref()
                    .map(|name| name.as_str())
                    .unwrap_or("<unnamed-conformance>")
            )));
        }
        if application.commitment.is_zero() {
            return Err(Diagnostic::error(
                "generic specialization selected a closed conformance with an empty commitment",
            ));
        }
        conformance_report_fingerprints.push(application.report_fingerprint);
        conformance_commitments.push(application.commitment.as_bytes());
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
    bytes.extend((conformance_commitments.len() as u64).to_le_bytes());
    for commitment in &conformance_commitments {
        bytes.extend(commitment);
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
    let report_fingerprint = machine_specialization_report_fingerprint(&bytes);
    let mut strong = Sha256::new();
    strong.update(b"omega.machine-specialization.v2\0");
    strong.update(&bytes);
    Ok(ReplayedMachineSpecializationIdentity {
        machine_contract_report_fingerprints,
        machine_contract_commitments,
        conformance_report_fingerprints,
        report_fingerprint,
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

pub(super) fn machine_specialization_report_fingerprint(bytes: &[u8]) -> u64 {
    let mut report_bytes = b"omega.machine-specialization.report.v2\0".to_vec();
    report_bytes.extend(bytes);
    fnv1a_report_fingerprint(&report_bytes)
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

pub(super) fn encode_state_signature(
    program: &TypedTrees,
    signature: &typed_trees::signature::StateSignature,
    binders: &[(String, String)],
    type_binders: &[(SymbolHandle, String)],
    output: &mut Vec<u8>,
) {
    for parameter in program.state_signature_parameters(signature) {
        encode_parameter(program, parameter, binders, type_binders, output);
    }
    encode_normalized_text(
        program
            .type_identity(TypeIdentityRequest {
                binders: type_binders,
                ..TypeIdentityRequest::ordinary(signature.return_type)
            })
            .as_str(),
        binders,
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
    let mut contract_binders = binders.to_vec();
    contract_binders.extend(
        program
            .state_signature_parameters(signature)
            .iter()
            .enumerate()
            .map(|(index, parameter)| (parameter.name.as_str().to_owned(), format!("$P{index}"))),
    );
    let contracts = encode_contract_set(
        program,
        program.state_signature_contracts(signature),
        &contract_binders,
    );
    for contract in contracts {
        output.extend(contract);
        output.push(0xfc);
    }
    match &signature.termination_guarantee {
        language_semantics::TerminationGuarantee::NoGuarantee => output.push(0),
        language_semantics::TerminationGuarantee::Terminates { premises } => {
            output.push(1);
            let parameter_symbols = program
                .state_signature_parameters(signature)
                .iter()
                .map(|parameter| parameter.symbol)
                .collect::<Vec<_>>();
            encode_progress_premises(premises, &parameter_symbols, output);
        }
    }
}

pub(super) fn encode_progress_premises(
    premises: &[language_semantics::ProgressPremise],
    parameter_symbols: &[symbols::SymbolHandle],
    output: &mut Vec<u8>,
) {
    let mut encoded = premises
        .iter()
        .map(|premise| {
            let mut bytes = Vec::new();
            bytes.extend(premise.profile.0.to_le_bytes());
            if let Some(index) = parameter_symbols
                .iter()
                .position(|symbol| *symbol == premise.subject.root)
            {
                bytes.push(0);
                bytes.extend(index.to_le_bytes());
            } else {
                bytes.push(1);
                bytes.extend(premise.subject.root.arena_index().to_le_bytes());
            }
            for projection in &premise.subject.projections {
                bytes.extend(projection.arena_index().to_le_bytes());
            }
            bytes
        })
        .collect::<Vec<_>>();
    encoded.sort();
    for premise in encoded {
        output.extend(premise);
        output.push(0xfa);
    }
}

pub(crate) fn canonical_state_signature_bytes(
    program: &TypedTrees,
    signature: &typed_trees::signature::StateSignature,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    encode_state_signature(program, signature, &[], &[], &mut bytes);
    bytes
}

pub(super) fn encode_state_shape(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    binders: &[(String, String)],
    type_binders: &[(SymbolHandle, String)],
    output: &mut Vec<u8>,
) {
    for parameter in program.state_parameters(state) {
        encode_parameter(program, parameter, binders, type_binders, output);
    }
    encode_normalized_text(
        program
            .type_identity(TypeIdentityRequest {
                binders: type_binders,
                ..TypeIdentityRequest::ordinary(state.return_type)
            })
            .as_str(),
        binders,
        output,
    );
    let mut contract_binders = binders.to_vec();
    contract_binders.extend(
        program
            .state_parameters(state)
            .iter()
            .enumerate()
            .map(|(index, parameter)| (parameter.name.as_str().to_owned(), format!("$P{index}"))),
    );
    let contracts = encode_contract_set(program, program.state_contracts(state), &contract_binders);
    for contract in contracts {
        output.extend(contract);
        output.push(0xfc);
    }
}

pub(super) fn encode_parameter(
    program: &TypedTrees,
    parameter: &typed_trees::signature::StateParameter,
    binders: &[(String, String)],
    type_binders: &[(SymbolHandle, String)],
    output: &mut Vec<u8>,
) {
    output.push(u8::from(parameter.is_self));
    output.push(u8::from(parameter.is_mutable));
    output.push(u8::from(parameter.is_const));
    encode_normalized_text(
        program
            .type_identity(TypeIdentityRequest {
                binders: type_binders,
                ..TypeIdentityRequest::ordinary(parameter.type_reference)
            })
            .as_str(),
        binders,
        output,
    );
}

pub(super) fn encode_contract(
    program: &TypedTrees,
    contract: &typed_trees::signature::SignatureContract,
    binders: &[(String, String)],
    output: &mut Vec<u8>,
) {
    encode_contract_kind(&contract.kind, binders, output);
    let mut facts: Vec<String> = program
        .proof_facts
        .span_or_empty(contract.facts)
        .iter()
        .map(|fact| contract_fact_text(program, fact))
        .collect();
    facts.sort();
    for fact in facts {
        encode_normalized_text(&fact, binders, output);
    }
}

pub(super) fn encode_contract_kind(
    kind: &typed_trees::signature::SignatureContractKind,
    _binders: &[(String, String)],
    output: &mut Vec<u8>,
) {
    output.push(match kind {
        typed_trees::signature::SignatureContractKind::Requires => 1,
        typed_trees::signature::SignatureContractKind::Ensures => 2,
        typed_trees::signature::SignatureContractKind::EnsuresForResultCase { .. } => 3,
        typed_trees::signature::SignatureContractKind::Crashes { .. } => 4,
    });
    if let typed_trees::signature::SignatureContractKind::EnsuresForResultCase {
        result_data,
        result_case,
    } = kind
    {
        output.extend_from_slice(&result_data.arena_index().to_le_bytes());
        output.extend_from_slice(&result_case.arena_index().to_le_bytes());
    }
    if let typed_trees::signature::SignatureContractKind::Crashes { cause } = kind {
        output.push(match cause {
            typed_trees::signature::CrashCause::Trap => 1,
            typed_trees::signature::CrashCause::Abort => 2,
        });
    }
}

pub(super) fn contract_fact_text(
    program: &TypedTrees,
    fact: &typed_trees::domain::ProofFact,
) -> String {
    match fact {
        typed_trees::domain::ProofFact::Expression(expression) => {
            program.expression_table.display_name(*expression)
        }
        typed_trees::domain::ProofFact::Membership(membership) => format!(
            "{} in {}{}",
            program.expression_table.display_name(membership.value),
            program
                .domain_path_members(membership.domain)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::"),
            if membership.domain_arguments.is_empty() {
                String::new()
            } else {
                // Contract identity follows normalized argument contents, not
                // source spellings or the program-local semantic interner ID.
                format!(
                    "<{}>",
                    program
                        .type_reference_table
                        .type_reference_handles(membership.domain_arguments)
                        .iter()
                        .map(|argument| program.normalized_type_identity(*argument).to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            }
        ),
        typed_trees::domain::ProofFact::Proposition(application) => format!(
            "{}({})",
            application.name.as_str(),
            program
                .expression_table
                .expression_handles(application.arguments)
                .iter()
                .map(|argument| program.expression_table.display_name(*argument))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Template and specialization identities use the same crash-bucket algebra
/// as public contract plans: route clauses merge by cause, routes form a set,
/// and an unconditional route subsumes guarded alternatives.
pub(super) fn encode_contract_set(
    program: &TypedTrees,
    contracts: &[typed_trees::signature::SignatureContract],
    binders: &[(String, String)],
) -> Vec<Vec<u8>> {
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct CrashBucket {
        unconditional: bool,
        routes: Vec<Vec<u8>>,
    }

    let mut encoded = Vec::new();
    let mut crash_buckets = BTreeMap::<Vec<u8>, CrashBucket>::new();
    for contract in contracts {
        if matches!(
            contract.kind,
            typed_trees::signature::SignatureContractKind::Crashes { .. }
        ) {
            let mut header = Vec::new();
            encode_contract_kind(&contract.kind, binders, &mut header);
            let bucket = crash_buckets.entry(header).or_default();
            let facts = program.proof_facts.span_or_empty(contract.facts);
            if facts.is_empty()
                || facts.iter().any(|fact| {
                    matches!(
                        fact,
                        typed_trees::domain::ProofFact::Expression(expression)
                            if matches!(
                                program.expression_table.expression(*expression),
                                typed_trees::expression::ExpressionNode::Boolean(true)
                            )
                    )
                })
            {
                bucket.unconditional = true;
            } else {
                for fact in facts {
                    let mut route = Vec::new();
                    encode_normalized_text(&contract_fact_text(program, fact), binders, &mut route);
                    bucket.routes.push(route);
                }
            }
            continue;
        }

        let mut contract_bytes = Vec::new();
        encode_contract(program, contract, binders, &mut contract_bytes);
        encoded.push(contract_bytes);
    }

    for (header, mut bucket) in crash_buckets {
        if bucket.unconditional {
            let mut contract = header;
            contract.push(0);
            encoded.push(contract);
            continue;
        }
        bucket.routes.sort();
        bucket.routes.dedup();
        for route in bucket.routes {
            let mut contract = header.clone();
            contract.push(1);
            contract.extend(route);
            encoded.push(contract);
        }
    }
    encoded.sort();
    encoded
}

pub(super) fn encode_normalized_text(
    text: &str,
    binders: &[(String, String)],
    output: &mut Vec<u8>,
) {
    let mut word = String::new();
    let flush = |word: &mut String, output: &mut Vec<u8>| {
        if word.is_empty() {
            return;
        }
        if let Some((_, replacement)) = binders.iter().find(|(name, _)| name == word) {
            output.extend(replacement.as_bytes());
        } else {
            output.extend(word.as_bytes());
        }
        word.clear();
    };
    for character in text.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            word.push(character);
        } else {
            flush(&mut word, output);
            output.extend(character.to_string().as_bytes());
        }
    }
    flush(&mut word, output);
    output.push(0);
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
