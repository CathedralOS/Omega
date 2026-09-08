//! Structural telescopes retain exact shared-view identities and availability.

use super::*;
use abstract_operations::AbstractStructuralBinding;
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
};

fn transfer_unit() -> PsiOptimizationUnit {
    let machine = id(1, MachineId::new);
    let entry = id(2, BlockId::new);
    let target = id(3, BlockId::new);
    let source = id(10, PlaceId::new);
    let destination = id(11, PlaceId::new);
    let structural_type = id(12, StructuralTypeId::new);
    let value = id(13, ValueId::new);
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
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
    let plan = AbstractOperationPlan {
        psi: unit().psi,
        entry: machine,
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::transferred-view".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
        }],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry,
            parameters: Vec::new(),
            structural_parameters: vec![parameter.clone()],
            result: AbstractFunctionResult::Scalar(AbstractResult { value, scalar_type }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![
                AbstractBlockEntry {
                    block: entry,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    operation_offset: 0,
                },
                AbstractBlockEntry {
                    block: target,
                    parameters: Vec::new(),
                    structural_parameters: vec![StructuralParameterDeclaration {
                        place: destination,
                        ..parameter
                    }],
                    operation_offset: 1,
                },
            ],
            operations: vec![
                AbstractOperation::Jump {
                    psi_edge: id(20, EdgeId::new),
                    target,
                    bindings: Vec::new(),
                    structural_bindings: vec![AbstractStructuralBinding {
                        parameter: destination,
                        argument: StructuralArgument {
                            place: source,
                            path: Vec::new(),
                            access: StructuralAccess::SharedBorrow,
                        },
                    }],
                    trivial_affine_discards: Vec::new(),
                    residual_affine_discards: Vec::new(),
                },
                AbstractOperation::ByteSequenceLength {
                    psi_operation: id(21, OperationId::new),
                    result: AbstractResult { value, scalar_type },
                    source: destination,
                },
                AbstractOperation::Return {
                    psi_edge: id(22, EdgeId::new),
                    result: value,
                    value,
                    scalar_type,
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    };
    reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap()).unwrap()
}

#[test]
fn shared_view_transfer_reconstructs_and_validates_exact_block_root() {
    let candidate = transfer_unit();
    validate_psi_optimization_unit(&candidate).unwrap();
    let function = &candidate.functions[0];
    assert!(
        function
            .structural_places
            .iter()
            .any(|place| place.id == id(11, PlaceId::new)
                && place.kind
                    == (StructuralPlaceKind::BlockParameter {
                        block: id(3, BlockId::new),
                        position: 0
                    }))
    );
    assert_eq!(
        function.blocks[0].nodes[0].successors[0].structural_bindings[0].parameter,
        id(11, PlaceId::new)
    );
}

#[test]
fn structural_binding_mutations_cannot_hide_in_rebuilt_metadata() {
    for mutation in 0..5 {
        let mut candidate = transfer_unit();
        let AbstractOperation::Jump {
            structural_bindings,
            ..
        } = &mut candidate.functions[0].blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        match mutation {
            0 => structural_bindings.clear(),
            1 => structural_bindings[0].parameter = id(10, PlaceId::new),
            2 => structural_bindings[0].argument.access = StructuralAccess::Owned,
            3 => structural_bindings[0].argument.place = id(11, PlaceId::new),
            4 => structural_bindings.push(structural_bindings[0].clone()),
            _ => unreachable!(),
        }
        refresh_node_derivatives(&mut candidate, 0, 0, 0);
        assert!(
            validate_psi_optimization_unit(&candidate).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn structural_block_parameter_identity_and_contract_are_not_inferred() {
    for mutation in 0..4 {
        let mut candidate = transfer_unit();
        let parameter = &mut candidate.functions[0].blocks[1].structural_parameters[0];
        match mutation {
            0 => parameter.position = 1,
            1 => parameter.structural_type = id(99, StructuralTypeId::new),
            2 => parameter.access = StructuralAccess::Owned,
            3 => parameter.place = id(10, PlaceId::new),
            _ => unreachable!(),
        }
        refresh_identity(&mut candidate);
        assert!(
            validate_psi_optimization_unit(&candidate).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn original_root_length_does_not_certify_a_transferred_view_read() {
    let mut candidate = transfer_unit();
    let block = &mut candidate.functions[0].blocks[1];
    let AbstractOperation::ByteSequenceLength { source, .. } = &mut block.nodes[0].operation else {
        unreachable!()
    };
    *source = id(10, PlaceId::new);
    let mut read = block.nodes[0].clone();
    read.operation = AbstractOperation::ByteSequenceRead {
        psi_operation: id(23, OperationId::new),
        result: AbstractResult {
            value: id(24, ValueId::new),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
        },
        source: id(11, PlaceId::new),
        index: id(13, ValueId::new),
        length: id(13, ValueId::new),
        obligation: id(25, semantic_vocabulary::ObligationId::new),
    };
    block.nodes.insert(1, read);
    for node in 0..3 {
        refresh_node_derivatives(&mut candidate, 0, 1, node);
    }
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::InvalidByteSequenceRead {
            machine: id(1, MachineId::new),
            block: id(3, BlockId::new),
            node: 1
        })
    );
}
