//! Current structural successor bindings preserve exact ordered arguments.
use super::{decode_module, encode_module};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, MachineId, PsiSemanticId, StructuralPlaceKind,
};
use terminal_psi::{
    Block, ByteSequenceCarrier, MachineContract, Operation, OperationKind, OperationResult,
    StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, SuccessorEdge, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, ValueDeclaration, VocabularyMarker,
};

fn id<T: PsiSemanticId>(raw: u64) -> T {
    T::new(raw).expect("test ids are nonzero")
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

#[test]
fn primitive_local_operations_round_trip_with_exact_result_and_operand_identities() {
    use semantic_vocabulary::{IeeeFloatFormat, IntegerSign, IntegerType, ScalarType};

    for scalar_type in [
        ScalarType::Boolean,
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap()),
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
    ] {
        let mut module = unit_module();
        module.structural_types.push(StructuralTypeDeclaration {
            id: id(7),
            identity: "PrimitiveLocal".into(),
            shape: StructuralTypeShape::PrimitiveScalar(scalar_type),
        });
        let machine = &mut module.machines[0];
        machine.parameters.push(ValueDeclaration {
            id: id(11),
            scalar_type,
        });
        machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
            id: id(59),
            scalar_type,
        });
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: id(23),
            kind: StructuralPlaceKind::OperationResult {
                producer: id(31),
                structural_type: id(7),
            },
        });
        machine.blocks[0].operations = vec![
            Operation {
                id: id(31),
                result: OperationResult::Structural(StructuralOperationResult {
                    place: id(23),
                    structural_type: id(7),
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: Vec::new(),
                }),
                kind: OperationKind::EstablishPrimitiveLocal { value: id(11) },
            },
            Operation {
                id: id(32),
                result: OperationResult::Scalar(ValueDeclaration {
                    id: id(47),
                    scalar_type,
                }),
                kind: OperationKind::PrimitiveScalarRead { source: id(23) },
            },
        ];
        machine.blocks[0].terminator = Terminator::Return {
            edge: id(1),
            value: id(47),
            cleanup_actions: Vec::new(),
        };

        let bytes = encode_module(&module).expect("primitive local module encodes");
        assert_eq!(&bytes[8..12], &[82, 0, 88, 0]);
        let decoded = decode_module(&bytes).expect("primitive local module decodes");
        assert_eq!(decoded, module);
        assert_eq!(encode_module(&decoded).unwrap(), bytes);

        let mut stale = bytes;
        stale[8..10].copy_from_slice(&81_u16.to_le_bytes());
        assert_eq!(
            decode_module(&stale),
            Err(super::CodecError::UnsupportedFormatMarker(81))
        );
        stale[8..10].copy_from_slice(&82_u16.to_le_bytes());
        stale[10..12].copy_from_slice(&87_u16.to_le_bytes());
        assert_eq!(
            decode_module(&stale),
            Err(super::CodecError::UnsupportedVocabularyMarker(87))
        );
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
    assert_eq!(&bytes[8..12], &[82, 0, 88, 0]);
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
    for (offset, marker) in [(8, 81_u16), (8, 83), (10, 87), (10, 89)] {
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
