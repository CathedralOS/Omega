//! One machine's structural declarations: literals, locals, places, result
//! and parameters.

use super::super::structural_qualification_rosters::validate_projected_qualification_roster;
use super::super::{
    BTreeMap, BTreeSet, BlockId, MachineId, ModuleError, OperationKind, ServiceId,
    StructuralAccess, StructuralDomainId, StructuralMultiplicity, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeId, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator,
};
use super::{
    ServiceCeilingOwner, StructuralSignatureOwner, validate_attachment,
    validate_machine_entry_claims, validate_provider_attachment_specialization,
    validate_service_ceiling, validate_structural_signature,
};
use terminal_psi::{ServiceDeclaration, StructuralDomainDeclaration, StructuralTypeDeclaration};

/// One machine's structural foundation: its block declarations,
/// attachment, signature and provider-attachment specialization, then its
/// byte-sequence literals, trivial affine locals, place declarations,
/// result declaration and structural parameters, its service ceilings and
/// its entry claims.
pub(super) fn validate_machine_foundation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
    services: &BTreeMap<ServiceId, &ServiceDeclaration>,
) -> Result<(), ModuleError> {
    super::super::block_views::validate_declarations(module, machine)?;
    validate_attachment(machine.id, machine.attachment, types)?;
    validate_structural_signature(
        &machine.structural_parameters,
        machine.attachment,
        types,
        domains,
        StructuralSignatureOwner::Machine(machine.id),
    )?;
    validate_provider_attachment_specialization(module, machine, types)?;
    validate_byte_sequence_literals(machine, types)?;
    validate_trivial_affine_locals(machine, types)?;
    validate_place_declarations(machine, types, domains)?;
    validate_result_declaration(module, machine, machines, types, domains)?;
    validate_structural_parameters(machine)?;
    validate_service_ceiling(
        &machine.published_service_ceiling,
        services,
        ServiceCeilingOwner::Machine(machine.id),
    )?;
    validate_service_ceiling(
        &machine.declared_service_reach,
        services,
        ServiceCeilingOwner::MachineDeclared(machine.id),
    )?;
    if let Some(service) = machine
        .declared_service_reach
        .iter()
        .find(|service| !machine.published_service_ceiling.contains(service))
    {
        return Err(ModuleError::DeclaredServiceOutsidePublishedCeiling {
            machine: machine.id,
            service: *service,
        });
    }
    validate_machine_entry_claims(module, machine)?;
    Ok(())
}

/// One machine's byte-sequence literal places: ordinals are dense, each
/// literal's type is a known byte-sequence carrier, and the operations
/// establishing the literals match the places exactly.
pub(super) fn validate_byte_sequence_literals(
    machine: &TerminalMachine,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<(), ModuleError> {
    let mut byte_sequence_literals = machine
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                structural_type,
            } => Some((place.id, declaration_ordinal, structural_type)),
            _ => None,
        })
        .collect::<Vec<_>>();
    byte_sequence_literals.sort_by_key(|(_, declaration_ordinal, _)| *declaration_ordinal);
    if byte_sequence_literals
        .iter()
        .enumerate()
        .any(|(expected, (_, declaration_ordinal, _))| {
            u32::try_from(expected).ok() != Some(*declaration_ordinal)
        })
    {
        return Err(ModuleError::NonCanonicalByteSequenceLiterals(machine.id));
    }
    for (place, _, structural_type) in &byte_sequence_literals {
        let Some(declaration) = types.get(structural_type) else {
            return Err(ModuleError::UnknownStructuralType(*structural_type));
        };
        if !matches!(
            declaration.shape,
            StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView)
        ) {
            return Err(
                ModuleError::ByteSequenceLiteralDeclarationRequiresBorrowedView {
                    machine: machine.id,
                    place: *place,
                },
            );
        }
    }
    let mut literal_establishments = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::EstablishByteSequenceLiteral { destination, .. } => Some(destination),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut expected_literal_establishments = byte_sequence_literals
        .iter()
        .map(|(place, _, _)| *place)
        .collect::<Vec<_>>();
    let total_literal_establishments = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::EstablishByteSequenceLiteral { .. }
            )
        })
        .count();
    // Declarations remain canonical, but an immutable literal may be
    // established at its authored evaluation position. Full-graph view
    // dominance below checks every consuming use independently.
    literal_establishments.sort();
    expected_literal_establishments.sort();
    if literal_establishments != expected_literal_establishments
        || total_literal_establishments != byte_sequence_literals.len()
    {
        return Err(ModuleError::ByteSequenceLiteralEstablishmentMismatch(
            machine.id,
        ));
    }
    Ok(())
}

/// One machine's trivial affine local places: ordinals are dense, each
/// local's type is an empty record, construction elements are well formed,
/// and the establishing operations (including entry establishments) match
/// the locals exactly.
pub(super) fn validate_trivial_affine_locals(
    machine: &TerminalMachine,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<(), ModuleError> {
    let mut trivial_affine_locals = machine
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                structural_type,
                construction,
            } => Some((place.id, declaration_ordinal, structural_type, construction)),
            _ => None,
        })
        .collect::<Vec<_>>();
    trivial_affine_locals.sort_by_key(|(_, declaration_ordinal, _, _)| *declaration_ordinal);
    if trivial_affine_locals
        .iter()
        .enumerate()
        .any(|(expected, (_, declaration_ordinal, _, _))| {
            u32::try_from(expected).ok() != Some(*declaration_ordinal)
        })
    {
        return Err(ModuleError::NonCanonicalTrivialAffineLocals(machine.id));
    }
    for (place, _, structural_type, _) in &trivial_affine_locals {
        let Some(declaration) = types.get(structural_type) else {
            return Err(ModuleError::UnknownStructuralType(*structural_type));
        };
        if !matches!(declaration.shape, StructuralTypeShape::Record { ref fields } if fields.is_empty())
        {
            return Err(
                ModuleError::TrivialAffineLocalDeclarationRequiresEmptyRecord {
                    machine: machine.id,
                    place: *place,
                },
            );
        }
    }
    let construction_elements = trivial_affine_locals
        .iter()
        .filter_map(|(_, ordinal, structural_type, construction)| {
            construction.map(|construction| (*ordinal, *structural_type, construction))
        })
        .collect::<Vec<_>>();
    if !construction_elements.is_empty() {
        let expected_root_length = match construction_elements.len() {
            2 => 3,
            3 => 4,
            4 => 5,
            5 => 6,
            6 => 7,
            7 => 8,
            8 => 9,
            9 => 10,
            10 => 11,
            11 => 12,
            12 => 13,
            13 => 14,
            14 => 15,
            15 => 16,
            16 => 17,
            17 => 18,
            18 => 19,
            19 => 20,
            20 => 21,
            21 => 22,
            22 => 23,
            23 => 24,
            24 => 25,
            25 => 26,
            _ => 0,
        };
        let exact_prefix = expected_root_length != 0
            && construction_elements.len() == trivial_affine_locals.len()
            && construction_elements.iter().enumerate().all(
                |(index, (ordinal, _, construction))| {
                    usize::try_from(*ordinal) == Ok(index)
                        && usize::try_from(construction.index) == Ok(index)
                },
            );
        let exact_root = construction_elements
            .first()
            .is_some_and(|(_, element_type, first)| {
                construction_elements
                    .iter()
                    .all(|(_, candidate_type, candidate)| {
                        candidate_type == element_type
                            && candidate.root_structural_type == first.root_structural_type
                    })
                    && types.get(&first.root_structural_type).is_some_and(|root| {
                        matches!(
                            root.shape,
                            StructuralTypeShape::FixedArray { element, length }
                                if element == *element_type
                                    && length == expected_root_length
                        )
                    })
            });
        if !exact_prefix || !exact_root {
            return Err(ModuleError::NonCanonicalTrivialAffineLocals(machine.id));
        }
    }
    // Every establishment site and the local it establishes. A declared
    // local has exactly one site machine-wide.
    let establishments = machine
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .operations
                .iter()
                .filter_map(|operation| match operation.kind {
                    OperationKind::EstablishTrivialAffineLocal { destination } => {
                        Some((block.id, destination))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let expected_establishments = trivial_affine_locals
        .iter()
        .map(|(place, _, _, _)| *place)
        .collect::<Vec<_>>();
    // The supported local prefix is established once, in declaration
    // order, before control leaves entry. A local may instead carry its
    // single establishment site inside a cyclic member block — one
    // reachable from itself through terminator successors — where the
    // frontier replay proves the re-arm lifecycle and the cyclic
    // eligibility fence admits only this empty-declaration form.
    // Structural-frontier validation carries those live locals through
    // every edge and checks normal-exit cleanup; a crash does not run
    // cleanup. Operand evaluation may split the remaining body into
    // blocks.
    let entry_establishments = establishments
        .iter()
        .filter(|(block, _)| *block == machine.entry)
        .map(|(_, place)| *place)
        .collect::<Vec<_>>();
    let successors = machine
        .blocks
        .iter()
        .map(|block| {
            let targets = match &block.terminator {
                Terminator::Jump { target, .. } => vec![*target],
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => vec![when_true.target, when_false.target],
                Terminator::StructuralCase { cases, .. } => {
                    cases.iter().map(|case| case.target).collect()
                }
                _ => Vec::new(),
            };
            (block.id, targets)
        })
        .collect::<BTreeMap<BlockId, Vec<BlockId>>>();
    let cyclic_members = machine
        .blocks
        .iter()
        .map(|block| block.id)
        .filter(|start| {
            let mut frontier = successors.get(start).cloned().unwrap_or_default();
            let mut visited = BTreeSet::new();
            while let Some(next) = frontier.pop() {
                if next == *start {
                    return true;
                }
                if visited.insert(next)
                    && let Some(targets) = successors.get(&next)
                {
                    frontier.extend(targets.iter().copied());
                }
            }
            false
        })
        .collect::<BTreeSet<BlockId>>();
    if !trivial_affine_locals.is_empty()
        && (!expected_establishments.starts_with(&entry_establishments)
            || establishments.len() != expected_establishments.len()
            || expected_establishments
                .iter()
                .any(|expected| !establishments.iter().any(|(_, place)| place == expected))
            || establishments.iter().any(|(block, place)| {
                *block != machine.entry
                    && (!cyclic_members.contains(block)
                        || trivial_affine_locals
                            .iter()
                            .find(|(declared, _, _, _)| declared == place)
                            .is_some_and(|(_, _, _, construction)| construction.is_some()))
            })
            || machine.blocks.iter().any(|block| {
                !matches!(
                    block.terminator,
                    Terminator::Jump { .. }
                        | Terminator::Conditional { .. }
                        | Terminator::StructuralCase { .. }
                        | Terminator::Crash { .. }
                        | Terminator::Return { .. }
                        | Terminator::ReturnStructural { .. }
                        | Terminator::ReturnUnit { .. }
                )
            }))
    {
        return Err(ModuleError::TrivialAffineLocalEstablishmentMismatch(
            machine.id,
        ));
    }
    Ok(())
}

/// One machine's structural places name known types and domains.
pub(super) fn validate_place_declarations(
    machine: &TerminalMachine,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
) -> Result<(), ModuleError> {
    for place in &machine.structural_places {
        let StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } = place.kind
        else {
            continue;
        };
        let Some(operation) = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| operation.id == producer)
        else {
            return Err(ModuleError::StructuralCallResultPlaceMismatch(producer));
        };
        let Some(result) = operation.result.structural() else {
            return Err(ModuleError::StructuralCallResultPlaceMismatch(producer));
        };
        if result.place != place.id || result.structural_type != structural_type {
            return Err(ModuleError::StructuralCallResultPlaceMismatch(producer));
        }
        validate_projected_qualification_roster(
            result.place,
            result.structural_type,
            &result.projected_qualifications,
            types,
            domains,
        )?;
    }
    Ok(())
}

/// One machine's result declaration: a Unit, scalar or structural result
/// whose places, type, multiplicity, qualifications and claims agree with
/// the machine's declarations and the callers that consume it.
pub(super) fn validate_result_declaration(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
) -> Result<(), ModuleError> {
    match &machine.result {
        TerminalMachineResult::Unit => {
            if let Some(place) = machine
                .structural_places
                .iter()
                .find(|place| place.kind == StructuralPlaceKind::Result)
            {
                return Err(ModuleError::UnitMachineHasResultStructuralPlace {
                    machine: machine.id,
                    place: place.id,
                });
            }
        }
        TerminalMachineResult::Scalar(_) => {
            if let Some(place) = machine
                .structural_places
                .iter()
                .find(|place| place.kind == StructuralPlaceKind::Result)
            {
                return Err(ModuleError::ScalarMachineHasResultStructuralPlace {
                    machine: machine.id,
                    place: place.id,
                });
            }
        }
        TerminalMachineResult::Structural(result) => {
            let exact_unrestricted_payloadless_result =
                result.multiplicity == StructuralMultiplicity::Unrestricted
                    && super::super::structural_result_contracts::has_empty_qualification_rosters(
                        &result.qualifications,
                        &result.projected_qualifications,
                    )
                    && !machine.blocks.is_empty()
                    && machine.blocks.iter().all(|block| {
                        let Terminator::ReturnStructural {
                            source,
                            returned_claims,
                            ..
                        } = &block.terminator
                        else {
                            return true;
                        };
                        if !returned_claims.is_empty() {
                            return false;
                        }
                        if let Some(StructuralPlaceDeclaration {
                            kind: StructuralPlaceKind::Parameter { position, is_self },
                            ..
                        }) = machine
                            .structural_places
                            .iter()
                            .find(|place| place.id == *source)
                        {
                            return matches!(machine.structural_parameters.as_slice(), [parameter]
                                if parameter.place == *source
                                    && parameter.position == *position
                                    && parameter.is_self == *is_self
                                    && !parameter.is_self
                                    && parameter.structural_type == result.structural_type
                                    && parameter.multiplicity
                                        == StructuralMultiplicity::Unrestricted
                                    && parameter.access == StructuralAccess::Owned
                                    && parameter.qualifications.is_empty());
                        }
                        let Some(StructuralPlaceDeclaration {
                            kind:
                                StructuralPlaceKind::OperationResult {
                                    producer,
                                    structural_type,
                                },
                            ..
                        }) = machine
                            .structural_places
                            .iter()
                            .find(|place| place.id == *source)
                        else {
                            return false;
                        };
                        if *structural_type != result.structural_type {
                            return false;
                        }
                        machine
                            .blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                            .find(|operation| operation.id == *producer)
                            .is_some_and(|operation| {
                                (matches!(
                                    &operation.kind,
                                    OperationKind::EstablishScalarCase { fields, .. } if fields.is_empty()
                                ) || super::super::structural_operations::exact_payloadless_structural_call(
                                    module,
                                    operation,
                                    machines,
                                )) && operation.result.structural().is_some_and(
                                    |operation_result| {
                                        operation_result.place == *source
                                            && operation_result.structural_type
                                                == result.structural_type
                                            && operation_result.multiplicity
                                                == StructuralMultiplicity::Unrestricted
                                            && super::super::structural_result_contracts::has_empty_qualification_rosters(
                                                &operation_result.qualifications,
                                                &operation_result.projected_qualifications,
                                            )
                                            && operation_result.claims.is_empty()
                                    },
                                )
                            })
                    })
                    && machine.blocks.iter().any(|block| {
                        matches!(block.terminator, Terminator::ReturnStructural { .. })
                    })
                    && machine
                        .blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .all(|operation| {
                            !matches!(
                                operation.kind,
                                OperationKind::Call { .. }
                                    | OperationKind::CallUnit { .. }
                                    | OperationKind::CallStructuralScalar { .. }
                                    | OperationKind::CallDynamicScalar { .. }
                                    | OperationKind::CallDynamicParameterScalar { .. }
                                    | OperationKind::BoundaryCall { .. }
                            ) && (!matches!(operation.kind, OperationKind::CallStructural { .. })
                                || super::super::structural_operations::exact_payloadless_structural_call(
                                    module,
                                    operation,
                                    machines,
                                ))
                        });
            if result.multiplicity == StructuralMultiplicity::Unrestricted
                && !exact_unrestricted_payloadless_result
                && !(result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && terminal_semantics::scalar_array_leaf_shape(
                        module.structural_types.iter(),
                        result.structural_type,
                    )
                    .is_some()
                    && machine.blocks.iter().all(|block| match &block.terminator {
                        Terminator::ReturnStructural {
                            source,
                            returned_claims,
                            ..
                        } => {
                            returned_claims.is_empty()
                                && super::super::scalar_array::plain_return_source(
                                    module, machine, *source,
                                )
                        }
                        _ => true,
                    }))
                && !(result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && machine.blocks.iter().all(|block| match &block.terminator {
                        Terminator::ReturnStructural {
                            source,
                            returned_claims,
                            ..
                        } => {
                            returned_claims.is_empty()
                                && (((super::super::scalar_case::plain_type(
                                    module,
                                    result.structural_type,
                                ) || super::super::record::plain_type(
                                    module,
                                    result.structural_type,
                                )) && (super::super::scalar_case::plain_return_source(
                                    module, machine, *source,
                                ) || super::super::record::plain_return_source(
                                    module, machine, *source,
                                ) || super::super::block_views::plain_owned_return_source(
                                    module, machine, *source,
                                )))
                                    // A leaf copy is fresh owned storage by
                                    // construction: an `Unrestricted` result
                                    // may publish it whatever the leaf shape.
                                    || super::super::structural_leaf_copy::copied_return_source(
                                        machine, *source,
                                    ))
                        }
                        _ => true,
                    }))
            {
                return Err(ModuleError::StructuralResultMustBeOwned(machine.id));
            }
            if !types.contains_key(&result.structural_type) {
                return Err(ModuleError::UnknownStructuralType(result.structural_type));
            }
            let mut qualifications = BTreeSet::new();
            for qualification in &result.qualifications {
                if !qualifications.insert(*qualification) {
                    return Err(ModuleError::DuplicateStructuralQualification {
                        place: result.place,
                        domain: *qualification,
                    });
                }
                let Some(domain) = domains.get(qualification) else {
                    return Err(ModuleError::UnknownStructuralDomain(*qualification));
                };
                if domain.carrier != result.structural_type {
                    return Err(ModuleError::StructuralDomainCarrierMismatch {
                        domain: domain.id,
                        expected: result.structural_type,
                        actual: domain.carrier,
                    });
                }
            }
            if result
                .qualifications
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            {
                return Err(ModuleError::NonCanonicalStructuralQualifications(
                    result.place,
                ));
            }
            validate_projected_qualification_roster(
                result.place,
                result.structural_type,
                &result.projected_qualifications,
                types,
                domains,
            )?;
            if !machine
                .structural_places
                .iter()
                .any(|place| place.id == result.place && place.kind == StructuralPlaceKind::Result)
            {
                return Err(ModuleError::StructuralResultPlaceMismatch {
                    machine: machine.id,
                    place: result.place,
                });
            }
        }
    }
    Ok(())
}

/// One machine's structural parameters are declared consistently with
/// their places.
pub(super) fn validate_structural_parameters(machine: &TerminalMachine) -> Result<(), ModuleError> {
    if !machine.structural_parameters.is_empty() {
        for parameter in &machine.structural_parameters {
            let expected = StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: parameter.is_self,
            };
            if !machine
                .structural_places
                .iter()
                .any(|place| place.id == parameter.place && place.kind == expected)
            {
                return Err(ModuleError::StructuralParameterPlaceMismatch {
                    machine: machine.id,
                    place: parameter.place,
                });
            }
        }
        for place in &machine.structural_places {
            if matches!(place.kind, StructuralPlaceKind::Parameter { .. })
                && !machine
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.place == place.id)
            {
                return Err(ModuleError::StructuralPlaceHasNoParameter {
                    machine: machine.id,
                    place: place.id,
                });
            }
        }
    }
    Ok(())
}
