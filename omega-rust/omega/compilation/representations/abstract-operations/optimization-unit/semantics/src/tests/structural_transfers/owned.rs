use super::{AbstractOperation, id};
use crate::tests::structural_transfers::transfer_unit;
use crate::tests::support::{refresh_identity, refresh_node_derivatives};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::AbstractStructuralBinding;
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{
    BlockId, IntegerSign, IntegerType, IntegerValue, OperationId, PlaceId, ScalarType,
    StructuralPlaceKind, ValueId,
};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralTypeShape,
    TerminalAffineCleanupAction,
};

pub(super) fn owned_unit() -> PsiOptimizationUnit {
    let mut candidate = transfer_unit();
    candidate.structural_types.make_mut()[0].shape =
        StructuralTypeShape::Record { fields: Vec::new() };
    let function = &mut candidate.functions[0];
    for parameter in function.structural_parameters.iter_mut().chain(
        function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.structural_parameters),
    ) {
        parameter.access = StructuralAccess::Owned;
        parameter.multiplicity = StructuralMultiplicity::Affine;
    }
    let mut source = function.structural_parameters[0].clone();
    source.place = id(30, PlaceId::new);
    source.position = 1;
    function.structural_parameters.push(source.clone());
    let mut destination = source;
    destination.place = id(31, PlaceId::new);
    function.blocks[1].structural_parameters.push(destination);
    function.structural_places.extend([
        terminal_psi::StructuralPlaceDeclaration {
            id: id(30, PlaceId::new),
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        },
        terminal_psi::StructuralPlaceDeclaration {
            id: id(31, PlaceId::new),
            kind: StructuralPlaceKind::BlockParameter {
                block: id(3, BlockId::new),
                position: 1,
            },
        },
    ]);
    function
        .declared_places
        .extend([id(30, PlaceId::new), id(31, PlaceId::new)]);
    let AbstractOperation::Jump {
        structural_bindings,
        ..
    } = &mut function.blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    structural_bindings[0].argument.access = StructuralAccess::Owned;
    structural_bindings.push(AbstractStructuralBinding {
        parameter: id(31, PlaceId::new),
        argument: StructuralArgument {
            place: id(30, PlaceId::new),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        },
    });
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    function.blocks[1].nodes[0].operation = AbstractOperation::IntegerConstant {
        psi_operation: id(21, OperationId::new),
        result: id(13, ValueId::new),
        scalar_type,
        value: IntegerValue::Unsigned(7),
    };
    let AbstractOperation::Return {
        cleanup_actions, ..
    } = &mut function.blocks[1].nodes[1].operation
    else {
        unreachable!()
    };
    *cleanup_actions = vec![
        TerminalAffineCleanupAction::DiscardRoot(id(31, PlaceId::new)),
        TerminalAffineCleanupAction::DiscardRoot(id(11, PlaceId::new)),
    ];
    refresh_node_derivatives(&mut candidate, 0, 0, 0);
    refresh_node_derivatives(&mut candidate, 0, 1, 0);
    refresh_node_derivatives(&mut candidate, 0, 1, 1);
    candidate
}

#[test]
fn owned_arrival_cannot_duplicate_one_source() {
    let mut candidate = owned_unit();
    let AbstractOperation::Jump {
        structural_bindings,
        ..
    } = &mut candidate.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    structural_bindings[1].argument.place = structural_bindings[0].argument.place;
    refresh_node_derivatives(&mut candidate, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));
}

#[test]
fn owned_arrival_signature_cannot_change_multiplicity_or_access() {
    for mutation in 0..3 {
        let mut candidate = owned_unit();
        let parameter = &mut candidate.functions[0].blocks[1].structural_parameters[0];
        match mutation {
            0 => parameter.multiplicity = StructuralMultiplicity::Unrestricted,
            1 => parameter.multiplicity = StructuralMultiplicity::Linear,
            2 => parameter.access = StructuralAccess::SharedBorrow,
            _ => unreachable!(),
        }
        refresh_identity(&mut candidate);
        assert!(
            validate_psi_optimization_unit(&candidate).is_err(),
            "mutation {mutation}"
        );
    }
}
