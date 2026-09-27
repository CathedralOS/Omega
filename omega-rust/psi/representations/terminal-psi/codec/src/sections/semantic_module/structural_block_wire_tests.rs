//! Current structural successor bindings preserve exact ordered arguments.

mod fixed_array_views;
mod owned_record_stores;
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
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
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
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
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
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
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
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
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
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
        }),
    });
    let machine = &mut module.machines[0];
    machine.structural_parameters = vec![borrowed_parameter(1, 0), borrowed_parameter(2, 1)];
    machine.parameters.push(terminal_psi::ValueDeclaration {
        qualifications: Default::default(),
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
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: vec![borrowed_argument(2), borrowed_argument(1)],
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    let successor = |edge, places: [u64; 2]| SuccessorEdge {
        edge: id(edge),
        target: id(3),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: places.into_iter().map(borrowed_argument).collect(),
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks.push(Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
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
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
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
        let malformed = crate::sections::semantic_module::module_wire::encode_raw(&module).unwrap();
        assert!(decode_module(&malformed).is_err());
    }
}

fn unit_byte_field_module() -> TerminalModule {
    let mut module = unit_module();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: id(1),
            identity: "BytesView".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            }),
        },
        StructuralTypeDeclaration {
            id: id(2),
            identity: "ByteOwner".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![terminal_psi::StructuralFieldDeclaration {
                    id: id(31),
                    identity: "bytes".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::ByteSequence(
                        ByteSequenceCarrier::BoundedOwned { capacity: 3 },
                    ),
                }],
            },
        },
    ];
    let mut root = borrowed_parameter(1, 0);
    root.access = StructuralAccess::MutableBorrow;
    root.structural_type = id(2);
    module.machines[0].structural_parameters = vec![root];
    module.machines[0].structural_places = vec![StructuralPlaceDeclaration {
        id: id(1),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    let mut callee = module.machines[0].clone();
    callee.id = id(2);
    callee.contract.id = id(2);
    callee.structural_parameters[0].structural_type = id(1);
    callee.structural_parameters[0].place = id(2);
    callee.structural_places[0].id = id(2);
    callee.entry = id(2);
    callee.blocks[0].id = id(2);
    callee.blocks[0].terminator = Terminator::ReturnUnit {
        edge: id(2),
        trivial_affine_discards: Vec::new(),
    };
    module.machines[0].blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(1),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            callee: callee.id,
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: id(1),
                path: vec![terminal_psi::StructuralPathSegment::Field("bytes".into())],
                access: StructuralAccess::MutableBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    }];
    module.machines.push(callee);
    module
}

#[test]
fn unit_mutable_byte_field_presentation_rejects_wrong_field_access_and_view_type() {
    for mutation in 0..5 {
        let mut module = unit_byte_field_module();
        let baseline = encode_module(&module).expect("hostile control starts lawful");
        assert_eq!(decode_module(&baseline), Ok(module.clone()));
        match mutation {
            0 | 1 => {
                let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape
                else {
                    unreachable!()
                };
                if mutation == 0 {
                    fields[0].field_type = terminal_psi::StructuralFieldType::Scalar(
                        semantic_vocabulary::ScalarType::Boolean,
                    );
                } else {
                    fields[0].relevance = terminal_psi::BindingRelevance::Erased;
                }
            }
            2 => {
                module.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow
            }
            3 => {
                module.structural_types[0].shape =
                    StructuralTypeShape::Record { fields: Vec::new() }
            }
            _ => {
                let OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } = &mut module.machines[0].blocks[0].operations[0].kind
                else {
                    unreachable!()
                };
                structural_arguments[0].path =
                    vec![terminal_psi::StructuralPathSegment::Field("missing".into())];
            }
        }
        assert!(
            encode_module(&module).is_err(),
            "encoder mutation {mutation}"
        );
        let bytes = crate::sections::semantic_module::module_wire::encode_raw(&module).unwrap();
        assert!(
            decode_module(&bytes).is_err(),
            "decoder mutation {mutation}"
        );
    }
}
