//! Structural Unit artifact and staging fixtures.

use crate::tests::*;

pub(crate) fn unit_return_artifact() -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(3_501).unwrap();
    let entry = BlockId::new(3_502).unwrap();
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
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
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine,
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
            entry,
            blocks: vec![Block {
                id: entry,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(3_503).unwrap(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(3_504).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    };
    (
        psi_terminal_codec::encode_module(&module).unwrap(),
        psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

pub(crate) fn port_write_unit_return_artifact() -> (Vec<u8>, Vec<u8>) {
    let (semantic, proof) = unit_return_artifact();
    let mut module = psi_terminal_codec::decode_module(&semantic).unwrap();
    let service = psi_core::ServiceId::new(3_505).unwrap();
    module.services.push(psi_terminal::ServiceDeclaration {
        id: service,
        identity: "test::DebugPort".to_owned(),
        parents: Vec::new(),
    });
    module.root_service_reach.concrete.push(service);
    module.machines[0].published_service_ceiling.push(service);
    module.machines[0].blocks[0].operations.push(Operation {
        id: OperationId::new(3_506).unwrap(),
        result: OperationResult::Unit,
        kind: OperationKind::PortWrite {
            service,
            port: 0x03f8,
            value: 0x41,
        },
    });
    (psi_terminal_codec::encode_module(&module).unwrap(), proof)
}

pub(crate) fn unit_call_return_artifact() -> (Vec<u8>, Vec<u8>) {
    let (semantic, proof) = unit_return_artifact();
    let mut module = psi_terminal_codec::decode_module(&semantic).unwrap();
    let callee = MachineId::new(3_507).unwrap();
    let mut callee_machine = module.machines[0].clone();
    callee_machine.id = callee;
    callee_machine.entry = BlockId::new(3_508).unwrap();
    callee_machine.blocks[0].id = callee_machine.entry;
    let Terminator::ReturnUnit { edge, .. } = &mut callee_machine.blocks[0].terminator else {
        unreachable!()
    };
    *edge = EdgeId::new(3_509).unwrap();
    callee_machine.contract.id = ContractId::new(3_510).unwrap();
    module.machines[0].blocks[0].operations.push(Operation {
        id: OperationId::new(3_511).unwrap(),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    module.machines.push(callee_machine);
    (psi_terminal_codec::encode_module(&module).unwrap(), proof)
}

pub(crate) fn byte_sequence_literal_unit_return_artifact() -> (Vec<u8>, Vec<u8>) {
    let (semantic, proof) = unit_return_artifact();
    let mut module = psi_terminal_codec::decode_module(&semantic).unwrap();
    let structural_type = StructuralTypeId::new(3_515).unwrap();
    let place = PlaceId::new(3_516).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "BorrowedBytes".into(),
        shape: StructuralTypeShape::ByteSequence(psi_terminal::ByteSequenceCarrier::BorrowedView),
    });
    module.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: 0,
                structural_type,
            },
        });
    module.machines[0].blocks[0].operations.push(Operation {
        id: OperationId::new(3_517).unwrap(),
        result: OperationResult::Unit,
        kind: OperationKind::EstablishByteSequenceLiteral {
            destination: place,
            bytes: vec![0x00, 0x4f, 0x6d, 0x65, 0x67, 0x61, 0xff],
        },
    });
    (psi_terminal_codec::encode_module(&module).unwrap(), proof)
}

pub(crate) fn ieee_float_literal_unit_return_artifact() -> (Vec<u8>, Vec<u8>) {
    let (semantic, proof) = unit_return_artifact();
    let mut module = psi_terminal_codec::decode_module(&semantic).unwrap();
    module.machines[0].blocks[0].operations.push(Operation {
        id: OperationId::new(3_518).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(3_519).unwrap(),
            scalar_type: ScalarType::IeeeFloat(psi_core::IeeeFloatFormat::Binary64),
        }),
        kind: OperationKind::IeeeFloatConstant {
            value: psi_core::IeeeFloatValue::Binary64(0x7ff8_1234_5678_9abc),
        },
    });
    (psi_terminal_codec::encode_module(&module).unwrap(), proof)
}

pub(crate) fn ieee_float_literal_sequence_unit_return_artifact() -> (Vec<u8>, Vec<u8>) {
    let (semantic, proof) = unit_return_artifact();
    let mut module = psi_terminal_codec::decode_module(&semantic).unwrap();
    for (operation, result, scalar_type, value) in [
        (
            3_520,
            3_521,
            ScalarType::IeeeFloat(psi_core::IeeeFloatFormat::Binary32),
            psi_core::IeeeFloatValue::Binary32(0x8000_0000),
        ),
        (
            3_522,
            3_523,
            ScalarType::IeeeFloat(psi_core::IeeeFloatFormat::Binary32),
            psi_core::IeeeFloatValue::Binary32(0x7fc1_2345),
        ),
        (
            3_524,
            3_525,
            ScalarType::IeeeFloat(psi_core::IeeeFloatFormat::Binary64),
            psi_core::IeeeFloatValue::Binary64(0x7ff8_1234_5678_9abc),
        ),
    ] {
        module.machines[0].blocks[0].operations.push(Operation {
            id: OperationId::new(operation).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(result).unwrap(),
                scalar_type,
            }),
            kind: OperationKind::IeeeFloatConstant { value },
        });
    }
    (psi_terminal_codec::encode_module(&module).unwrap(), proof)
}

pub(crate) fn nearest_ieee_float_fused_multiply_add_unit_return_artifact(
    format: psi_core::IeeeFloatFormat,
) -> (Vec<u8>, Vec<u8>) {
    let (semantic, proof) = unit_return_artifact();
    let mut module = psi_terminal_codec::decode_module(&semantic).unwrap();
    let values = match format {
        psi_core::IeeeFloatFormat::Binary32 => [
            psi_core::IeeeFloatValue::Binary32(0x8000_0000),
            psi_core::IeeeFloatValue::Binary32(0x7fc1_2345),
            psi_core::IeeeFloatValue::Binary32(0x3f80_0001),
        ],
        psi_core::IeeeFloatFormat::Binary64 => [
            psi_core::IeeeFloatValue::Binary64(0x8000_0000_0000_0000),
            psi_core::IeeeFloatValue::Binary64(0x7ff8_1234_5678_9abc),
            psi_core::IeeeFloatValue::Binary64(0x3ff0_0000_0000_0001),
        ],
    };
    for (position, value) in values.into_iter().enumerate() {
        module.machines[0].blocks[0].operations.push(Operation {
            id: OperationId::new(3_530 + position as u64 * 2).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(3_531 + position as u64 * 2).unwrap(),
                scalar_type: ScalarType::IeeeFloat(format),
            }),
            kind: OperationKind::IeeeFloatConstant { value },
        });
    }
    module.machines[0].blocks[0].operations.push(Operation {
        id: OperationId::new(3_536).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(3_537).unwrap(),
            scalar_type: ScalarType::IeeeFloat(format),
        }),
        kind: OperationKind::NearestIeeeFloatFusedMultiplyAdd {
            left: ValueId::new(3_531).unwrap(),
            right: ValueId::new(3_533).unwrap(),
            addend: ValueId::new(3_535).unwrap(),
        },
    });
    (psi_terminal_codec::encode_module(&module).unwrap(), proof)
}

pub(crate) fn trivial_affine_local_unit_return_artifact() -> (Vec<u8>, Vec<u8>) {
    let (semantic, proof) = unit_return_artifact();
    let mut module = psi_terminal_codec::decode_module(&semantic).unwrap();
    let structural_type = StructuralTypeId::new(3_512).unwrap();
    let place = PlaceId::new(3_513).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "TrivialAffineToken".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    module.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal: 0,
                structural_type,
                construction: None,
            },
        });
    module.machines[0].blocks[0].operations.push(Operation {
        id: OperationId::new(3_514).unwrap(),
        result: OperationResult::Unit,
        kind: OperationKind::EstablishTrivialAffineLocal { destination: place },
    });
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    *trivial_affine_discards = vec![place];
    (psi_terminal_codec::encode_module(&module).unwrap(), proof)
}

pub(crate) fn staged_unit_return(
    target: NativeTarget,
) -> (Vec<u8>, Vec<u8>, StagedOptimizedSelectedInstructions) {
    let (semantic, proof) = unit_return_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
    )
    .unwrap();
    let target = lower_optimized_to_target_operations(optimized, target).unwrap();
    (
        semantic,
        proof,
        stage_optimized_instruction_selection(target).unwrap(),
    )
}

pub(crate) fn structurally_parameterized_unit_return_artifact() -> (Vec<u8>, Vec<u8>) {
    let (semantic, proof) = unit_return_artifact();
    let mut module = psi_terminal_codec::decode_module(&semantic).unwrap();
    let structural_type = StructuralTypeId::new(3_505).unwrap();
    let place = PlaceId::new(3_506).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "UnitStructuralInput".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: StructuralFieldId::new(1).unwrap(),
                identity: "value".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                )),
            }],
        },
    });
    let entry = module.machines.first_mut().unwrap();
    entry.structural_parameters = vec![StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    entry.structural_places = vec![StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    (psi_terminal_codec::encode_module(&module).unwrap(), proof)
}

pub(crate) fn structural_extent_call_unit_artifact() -> (Vec<u8>, Vec<u8>) {
    let caller = MachineId::new(3_601).unwrap();
    let callee = MachineId::new(3_602).unwrap();
    let extent = StructuralTypeId::new(3_603).unwrap();
    let granted = StructuralDomainId::new(3_604).unwrap();
    let caller_places = [PlaceId::new(3_605).unwrap(), PlaceId::new(3_606).unwrap()];
    let callee_places = [PlaceId::new(3_607).unwrap(), PlaceId::new(3_608).unwrap()];
    let parameter = |place, position| StructuralParameterDeclaration {
        place,
        position,
        is_self: false,
        structural_type: extent,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::Owned,
        qualifications: vec![granted],
        projected_qualifications: Vec::new(),
    };
    let places = |roots: [PlaceId; 2]| {
        roots
            .into_iter()
            .enumerate()
            .map(|(position, id)| StructuralPlaceDeclaration {
                id,
                kind: StructuralPlaceKind::Parameter {
                    position: u32::try_from(position).unwrap(),
                    is_self: false,
                },
            })
            .collect::<Vec<_>>()
    };
    let parameters = |roots: [PlaceId; 2]| {
        roots
            .into_iter()
            .enumerate()
            .map(|(position, place)| parameter(place, u32::try_from(position).unwrap()))
            .collect::<Vec<_>>()
    };
    let contract = |id| MachineContract {
        id,
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    };
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: caller,
        structural_types: vec![StructuralTypeDeclaration {
            id: extent,
            identity: "named(name(Extent))".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "base".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                            IntegerType::address(64).unwrap(),
                        )),
                    },
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(2).unwrap(),
                        identity: "length".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                        )),
                    },
                ],
            },
        }],
        structural_domains: vec![StructuralDomainDeclaration {
            id: granted,
            semantic_domain: DomainSemanticId::new(3_609).unwrap(),
            identity: "Extent::Granted".into(),
            carrier: extent,
            content_projection: None,
        }],
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
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![
            TerminalMachine {
                id: caller,
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: parameters(caller_places),
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: places(caller_places),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: BlockId::new(3_610).unwrap(),
                blocks: vec![Block {
                    id: BlockId::new(3_610).unwrap(),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(3_611).unwrap(),
                        result: OperationResult::Unit,
                        kind: OperationKind::CallUnit {
                            callee,
                            structural_arguments: caller_places
                                .into_iter()
                                .map(|place| psi_terminal::StructuralArgument {
                                    place,
                                    path: Vec::new(),
                                    access: StructuralAccess::Owned,
                                })
                                .collect(),
                            claim_transfers: Vec::new(),
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                        },
                    }],
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(3_612).unwrap(),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: contract(ContractId::new(3_613).unwrap()),
            },
            TerminalMachine {
                id: callee,
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: parameters(callee_places),
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: places(callee_places),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: BlockId::new(3_614).unwrap(),
                blocks: vec![Block {
                    id: BlockId::new(3_614).unwrap(),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(3_615).unwrap(),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: contract(ContractId::new(3_616).unwrap()),
            },
        ],
    };
    let proof = ProofBundle::default();
    (
        psi_terminal_codec::encode_module(&module).unwrap(),
        psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

pub(crate) fn structural_extent_unit_leaf_artifact() -> (Vec<u8>, Vec<u8>) {
    let (semantic, _) = structural_extent_call_unit_artifact();
    let mut module = psi_terminal_codec::decode_module(&semantic).unwrap();
    let leaf = module
        .machines
        .pop()
        .expect("the structural call fixture has one exact Unit leaf");
    module.entry = leaf.id;
    module.machines = vec![leaf];
    let proof = ProofBundle::default();
    (
        psi_terminal_codec::encode_module(&module).unwrap(),
        psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

pub(crate) fn statically_attached_unit_return_artifact() -> (Vec<u8>, Vec<u8>, StructuralTypeId) {
    let (semantic, proof) = unit_return_artifact();
    let mut module = psi_terminal_codec::decode_module(&semantic).unwrap();
    let attachment = StructuralTypeId::new(3_507).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: attachment,
        identity: "UnitStaticAttachment".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    module.machines.first_mut().unwrap().attachment = Some(attachment);
    (
        psi_terminal_codec::encode_module(&module).unwrap(),
        proof,
        attachment,
    )
}
