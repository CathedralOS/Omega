use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, ContractId, EdgeId, IeeeFloatFormat,
    IeeeFloatStructuralField, MachineId, OperationId, PlaceId, PsiSemanticId, StructuralFieldId,
    ValueId,
};
use terminal_psi::{
    Block, DirectBlockFloatParameter, DirectCallFloatResult, DirectMachineFloatResult,
    DirectOperationFloatResult, DirectStructuralFloatLeaf, FloatMeaningProjection,
    FloatMeaningProjectionOperation, FloatMeaningSource, MachineContract, ProofOnlyValueType,
    ProofValueDeclaration, ProofValueId, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, VocabularyMarker,
};

use super::{decode_module, encode_module};
use crate::module_wire::encode_legacy_result_path_raw;
use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, SuccessorEdge,
};

fn id<T: PsiSemanticId>(raw: u64) -> T {
    T::new(raw).expect("test ids are nonzero")
}

#[test]
fn legacy_result_path_format_cannot_transport_natural_ranking() {
    let mut module = unit_module();
    module.machines[0].ranked_scc = Some(terminal_psi::TerminalRankedScc::Natural(Vec::new()));
    assert_eq!(
        encode_legacy_result_path_raw(&module),
        Err(crate::CodecError::InvalidTag("TerminalRankedScc", 2)),
    );
}

fn unit_module() -> TerminalModule {
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: id::<MachineId>(1),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: id::<MachineId>(1),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: id::<BlockId>(1),
            blocks: vec![Block {
                structural_parameters: Vec::new(),
                id: id::<BlockId>(1),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: id::<EdgeId>(1),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: id::<ContractId>(1),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
                crash_routes: Vec::new(),
            },
        }],
    }
}

fn borrowed_parameter(place: u64, position: u32) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: id(place),
        position,
        is_self: false,
        structural_type: id(1),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn borrowed_argument(place: u64) -> StructuralArgument {
    StructuralArgument {
        place: id(place),
        path: Vec::new(),
        access: StructuralAccess::SharedBorrow,
    }
}

fn structural_block_module() -> TerminalModule {
    let mut module = unit_module();
    module.structural_types.push(StructuralTypeDeclaration {
        id: id(1),
        identity: "Bytes".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
    });
    let machine = &mut module.machines[0];
    machine.structural_parameters = vec![borrowed_parameter(1, 0), borrowed_parameter(2, 1)];
    machine.parameters.push(terminal_psi::ValueDeclaration {
        id: id(1),
        scalar_type: semantic_vocabulary::ScalarType::Boolean,
    });
    machine.structural_places = (1..=6)
        .map(|place| StructuralPlaceDeclaration {
            id: id(place),
            kind: if place <= 2 {
                StructuralPlaceKind::Parameter {
                    position: (place - 1) as u32,
                    is_self: false,
                }
            } else {
                StructuralPlaceKind::BlockParameter {
                    block: id(if place <= 4 { 2 } else { 3 }),
                    position: ((place - 3) % 2) as u32,
                }
            },
        })
        .collect();
    machine.blocks[0].terminator = Terminator::Jump {
        edge: id(1),
        target: id(2),
        arguments: Vec::new(),
        structural_arguments: vec![borrowed_argument(2), borrowed_argument(1)],
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    let successor = |edge, places: [u64; 2]| SuccessorEdge {
        edge: id(edge),
        target: id(3),
        arguments: Vec::new(),
        structural_arguments: places.into_iter().map(borrowed_argument).collect(),
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks.push(Block {
        id: id(2),
        parameters: Vec::new(),
        structural_parameters: vec![borrowed_parameter(3, 0), borrowed_parameter(4, 1)],
        operations: Vec::new(),
        terminator: Terminator::Conditional {
            condition: id(1),
            when_true: successor(2, [3, 4]),
            when_false: successor(3, [4, 3]),
        },
    });
    machine.blocks.push(Block {
        id: id(3),
        parameters: Vec::new(),
        structural_parameters: vec![borrowed_parameter(5, 0), borrowed_parameter(6, 1)],
        operations: Vec::new(),
        terminator: Terminator::ReturnUnit {
            edge: id(4),
            trivial_affine_discards: Vec::new(),
        },
    });
    module
}

#[test]
fn structural_block_bindings_round_trip_and_bind_each_argument_order() {
    let module = structural_block_module();
    let bytes = encode_module(&module).expect("borrowed block bindings encode");
    assert_eq!(&bytes[8..12], &[79, 0, 85, 0]);
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(
        encode_module(&decode_module(&bytes).unwrap()),
        Ok(bytes.clone())
    );
    for edge in 0..3 {
        let mut changed = module.clone();
        let machine = &mut changed.machines[0];
        let arguments = if edge == 0 {
            let Terminator::Jump {
                structural_arguments,
                ..
            } = &mut machine.blocks[0].terminator
            else {
                unreachable!()
            };
            structural_arguments
        } else {
            let Terminator::Conditional {
                when_true,
                when_false,
                ..
            } = &mut machine.blocks[1].terminator
            else {
                unreachable!()
            };
            if edge == 1 {
                &mut when_true.structural_arguments
            } else {
                &mut when_false.structural_arguments
            }
        };
        arguments.reverse();
        assert_ne!(encode_module(&changed).unwrap(), bytes);
        assert_ne!(
            super::semantic_fingerprint(&changed).unwrap(),
            super::semantic_fingerprint(&module).unwrap()
        );
    }
    for (offset, marker) in [(8, 78_u16), (8, 80), (10, 84), (10, 86)] {
        let mut stale = bytes.clone();
        stale[offset..offset + 2].copy_from_slice(&marker.to_le_bytes());
        assert!(decode_module(&stale).is_err());
    }
    for length in [12, bytes.len() - 1] {
        assert!(decode_module(&bytes[..length]).is_err());
    }
}

#[test]
fn structural_block_bindings_reject_malformed_parameter_coordinates() {
    let original = structural_block_module();
    for mutation in 0..5 {
        let mut module = original.clone();
        let machine = &mut module.machines[0];
        match mutation {
            0 => machine.blocks[1].structural_parameters[0].position = 1,
            1 => machine.blocks[1].structural_parameters[0].is_self = true,
            2 => machine.blocks[0]
                .structural_parameters
                .push(borrowed_parameter(3, 0)),
            3 => {
                machine.structural_places[2].kind = StructuralPlaceKind::BlockParameter {
                    block: id(99),
                    position: 0,
                }
            }
            4 => machine.blocks[1].structural_parameters[0].place = id(4),
            _ => unreachable!(),
        }
        assert!(encode_module(&module).is_err());
        let malformed = crate::module_wire::encode_raw(&module).unwrap();
        assert!(decode_module(&malformed).is_err());
    }
}

#[test]
fn legacy_format_rejects_each_new_structural_binding_payload() {
    for payload in 0..4 {
        let mut module = unit_module();
        let machine = &mut module.machines[0];
        match payload {
            0 => machine.blocks[0]
                .structural_parameters
                .push(borrowed_parameter(1, 0)),
            1 => machine.structural_places.push(StructuralPlaceDeclaration {
                id: id(1),
                kind: StructuralPlaceKind::BlockParameter {
                    block: id(1),
                    position: 0,
                },
            }),
            2 => {
                machine.blocks[0].terminator = Terminator::Jump {
                    edge: id(1),
                    target: id(1),
                    arguments: Vec::new(),
                    structural_arguments: vec![borrowed_argument(1)],
                    trivial_affine_discards: Vec::new(),
                    residual_affine_discards: Vec::new(),
                }
            }
            3 => {
                let successor = SuccessorEdge {
                    edge: id(1),
                    target: id(1),
                    arguments: Vec::new(),
                    structural_arguments: vec![borrowed_argument(1)],
                    trivial_affine_discards: Vec::new(),
                };
                machine.blocks[0].terminator = Terminator::Conditional {
                    condition: id(1),
                    when_true: successor.clone(),
                    when_false: successor,
                };
            }
            _ => unreachable!(),
        }
        assert!(
            encode_legacy_result_path_raw(&module).is_err(),
            "payload {payload}"
        );
    }
}

#[test]
fn legacy_machine_decoder_rejects_block_parameter_place_tag() {
    let module = structural_block_module();
    let mut writer = crate::wire::Writer::default();
    crate::machine_wire::encode_machine_for_result_paths(
        &mut writer,
        &module.machines[0],
        crate::structural_result_wire::ResultPathWireFormat::Current,
    )
    .unwrap();
    let bytes = writer.finish();
    assert_eq!(
        crate::machine_wire::decode_machine_for_result_paths(
            &mut crate::wire::Reader::new(&bytes),
            crate::structural_result_wire::ResultPathWireFormat::LegacyWithoutResultPaths,
        ),
        Err(super::CodecError::InvalidTag("StructuralPlaceKind", 8))
    );
}

#[test]
fn v56_v59_reconstructs_absent_result_path_rosters_as_current_empty_rows() {
    let module = unit_module();
    let legacy = encode_legacy_result_path_raw(&module).expect("legacy compatibility bytes");
    assert_eq!(&legacy[8..10], &56_u16.to_le_bytes());
    assert_eq!(&legacy[10..12], &59_u16.to_le_bytes());
    assert_eq!(decode_module(&legacy), Ok(module.clone()));

    let current = encode_module(&module).expect("current result-path bytes");
    assert_eq!(&current[8..10], &79_u16.to_le_bytes());
    assert_eq!(&current[10..12], &85_u16.to_le_bytes());

    let mut crossed_pair = legacy;
    crossed_pair[10..12].copy_from_slice(&72_u16.to_le_bytes());
    assert!(decode_module(&crossed_pair).is_err());
}

#[test]
fn v56_v59_rejects_current_only_direct_float_sources() {
    let operation = numerics::float_projection::FloatProjectionOperation::Meaning32;
    let contract = operation.contract_identity();
    let contract = terminal_psi::FloatProjectionContractIdentity {
        format: contract.format,
        operation: contract.operation,
        declaration: contract.declaration,
        catalog_version: contract.catalog_version,
        commitment: contract.commitment,
    };
    let mut module = unit_module();
    module.float_meaning_projections = vec![FloatMeaningProjection {
        result: ProofValueDeclaration {
            id: ProofValueId(0),
            value_type: ProofOnlyValueType::FloatMeaning,
        },
        source: FloatMeaningSource::DirectMachineResult(DirectMachineFloatResult {
            owner: id::<MachineId>(1),
            result: id::<ValueId>(1),
            format: IeeeFloatFormat::Binary32,
        }),
        operation: FloatMeaningProjectionOperation::Meaning32,
        contract,
    }];
    let mut legacy = encode_legacy_result_path_raw(&module).expect("legacy direct result bytes");
    let source_prefix = [0, 0, 0, 0, 1, 5];
    let source_offset = legacy
        .windows(source_prefix.len())
        .position(|window| window == source_prefix)
        .expect("legacy direct result source is unique");
    legacy[source_offset + 5] = 6;
    assert_eq!(
        decode_module(&legacy),
        Err(super::CodecError::InvalidTag("FloatMeaningSource", 6))
    );
    legacy[source_offset + 5] = 7;
    assert_eq!(
        decode_module(&legacy),
        Err(super::CodecError::InvalidTag("FloatMeaningSource", 7))
    );
    legacy[source_offset + 5] = 8;
    assert_eq!(
        decode_module(&legacy),
        Err(super::CodecError::InvalidTag("FloatMeaningSource", 8))
    );
    legacy[source_offset + 5] = 9;
    assert_eq!(
        decode_module(&legacy),
        Err(super::CodecError::InvalidTag("FloatMeaningSource", 9))
    );

    module.float_meaning_projections[0].source =
        FloatMeaningSource::DirectOperationResult(DirectOperationFloatResult {
            owner: id::<MachineId>(1),
            producer: id::<OperationId>(1),
            result: id::<ValueId>(1),
            format: IeeeFloatFormat::Binary32,
        });
    assert_eq!(
        encode_legacy_result_path_raw(&module),
        Err(super::CodecError::InvalidTag(
            "legacy FloatMeaningSource",
            6
        ))
    );

    module.float_meaning_projections[0].source =
        FloatMeaningSource::DirectBlockParameter(DirectBlockFloatParameter {
            owner: id::<MachineId>(1),
            block: id::<BlockId>(1),
            parameter: id::<ValueId>(1),
            format: IeeeFloatFormat::Binary32,
        });
    assert_eq!(
        encode_legacy_result_path_raw(&module),
        Err(super::CodecError::InvalidTag(
            "legacy FloatMeaningSource",
            7
        ))
    );

    module.float_meaning_projections[0].source =
        FloatMeaningSource::DirectCallResult(DirectCallFloatResult {
            owner: id::<MachineId>(1),
            producer: id::<OperationId>(1),
            result: id::<ValueId>(1),
            format: IeeeFloatFormat::Binary32,
        });
    assert_eq!(
        encode_legacy_result_path_raw(&module),
        Err(super::CodecError::InvalidTag(
            "legacy FloatMeaningSource",
            8
        ))
    );

    module.float_meaning_projections[0].source =
        FloatMeaningSource::DirectStructuralLeaf(DirectStructuralFloatLeaf {
            owner: id::<MachineId>(1),
            field: IeeeFloatStructuralField::new(
                id::<PlaceId>(1),
                vec![CanonicalStructuralPathSegment::Field(
                    id::<StructuralFieldId>(1),
                )],
            )
            .expect("nonempty structural path"),
            format: IeeeFloatFormat::Binary32,
        });
    assert_eq!(
        encode_legacy_result_path_raw(&module),
        Err(super::CodecError::InvalidTag(
            "legacy FloatMeaningSource",
            9
        ))
    );
}
