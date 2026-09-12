use super::*;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, MachineId, PlaceId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, ByteSequenceCarrier, MachineContract, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TerminalMachine,
    TerminalMachineResult, VocabularyMarker,
};

fn fixture() -> TerminalModule {
    let machine = MachineId::new(1).unwrap();
    let entry = BlockId::new(1).unwrap();
    let target = BlockId::new(2).unwrap();
    let source = PlaceId::new(1).unwrap();
    let destination = PlaceId::new(2).unwrap();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let parameter = StructuralParameterDeclaration {
        place: source,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_range_invariants: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::Bytes".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
        }],
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
            id: machine,
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: vec![parameter.clone()],
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: vec![
                StructuralPlaceDeclaration {
                    id: source,
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                },
                StructuralPlaceDeclaration {
                    id: destination,
                    kind: StructuralPlaceKind::BlockParameter {
                        block: target,
                        position: 0,
                    },
                },
            ],
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks: vec![
                Block {
                    id: entry,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(1).unwrap(),
                        target,
                        arguments: Vec::new(),
                        structural_arguments: vec![StructuralArgument {
                            place: source,
                            path: Vec::new(),
                            access: StructuralAccess::SharedBorrow,
                        }],
                        trivial_affine_discards: Vec::new(),
                        residual_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: target,
                    parameters: Vec::new(),
                    structural_parameters: vec![StructuralParameterDeclaration {
                        place: destination,
                        ..parameter
                    }],
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(2).unwrap(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(1).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

#[test]
fn shared_view_block_parameters_retain_their_declarations() {
    let mut module = fixture();
    assert_eq!(validate_structural_block_bindings(&module), Ok(()));
    module.machines[0].blocks[1].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    assert_eq!(validate_structural_block_bindings(&module), Ok(()));
    module.machines[0].blocks[1].structural_parameters[0].access =
        StructuralAccess::WriteOnlyBorrow;
    module.machines[0].blocks.swap(0, 1);
    assert_eq!(
        validate_structural_block_bindings(&module),
        Err(LoweringError::UnsupportedStructuralBlockParameters {
            machine: module.entry,
            block: BlockId::new(2).unwrap(),
        })
    );
}

#[test]
fn each_conditional_arm_is_checked_even_without_byte_operations() {
    for selected_arm in 0..2 {
        let mut module = fixture();
        let successor = |ordinal| SuccessorEdge {
            edge: EdgeId::new(ordinal).unwrap(),
            target: BlockId::new(2).unwrap(),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        };
        let mut successors = [successor(3), successor(4)];
        successors[selected_arm]
            .structural_arguments
            .push(StructuralArgument {
                place: PlaceId::new(1).unwrap(),
                path: Vec::new(),
                access: StructuralAccess::WriteOnlyBorrow,
            });
        let expected_edge = successors[selected_arm].edge;
        let [when_true, when_false] = successors;
        module.machines[0].blocks[0].terminator = Terminator::Conditional {
            condition: ValueId::new(1).unwrap(),
            when_true,
            when_false,
        };
        // Deliberately inspect the consumer gate directly: neither a missing
        // condition value nor the target declaration can conceal an arm.
        assert_eq!(
            validate_structural_block_bindings(&module),
            Err(LoweringError::UnsupportedStructuralSuccessorArguments {
                machine: module.entry,
                edge: expected_edge,
            })
        );
    }
}

#[test]
fn empty_block_bindings_preserve_the_existing_gate() {
    let mut module = fixture();
    module.machines[0].blocks[1].structural_parameters.clear();
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    structural_arguments.clear();
    assert_eq!(validate_structural_block_bindings(&module), Ok(()));
}

#[test]
fn common_native_and_optimizer_lowering_cannot_drop_descriptor_only_transfer() {
    let module = fixture();
    let plan = crate::lowering::lower_decoded_module(&module).unwrap();
    let function = &plan.functions[0];
    assert_eq!(
        function.block_entries[1].structural_parameters,
        module.machines[0].blocks[1].structural_parameters
    );
    let abstract_operations::AbstractOperation::Jump {
        structural_bindings,
        ..
    } = &function.operations[0]
    else {
        panic!("expected exact jump");
    };
    assert_eq!(structural_bindings.len(), 1);
    assert_eq!(structural_bindings[0].parameter, PlaceId::new(2).unwrap());
    assert_eq!(
        structural_bindings[0].argument,
        StructuralArgument {
            place: PlaceId::new(1).unwrap(),
            path: Vec::new(),
            access: StructuralAccess::SharedBorrow
        }
    );
}
