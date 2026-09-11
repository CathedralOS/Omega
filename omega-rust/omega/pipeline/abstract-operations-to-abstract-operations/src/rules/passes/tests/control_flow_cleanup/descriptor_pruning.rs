//! Scalar pruning does not remove a retained descriptor telescope.
use super::*;

#[test]
fn conditional_fold_does_not_propose_orphaning_descriptor_roots() {
    use semantic_vocabulary::{PlaceId, StructuralPlaceKind, StructuralTypeId};
    use terminal_psi::{
        ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
        StructuralTypeShape,
    };
    let mut unit = propagated_block_parameter_unit(true);
    let source = id(9001, PlaceId::new);
    let destination = id(9002, PlaceId::new);
    let structural_type = id(9003, StructuralTypeId::new);
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
    unit.structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::pruned-view".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
        });
    let function = &mut unit.functions[0];
    function.structural_parameters.push(parameter.clone());
    function.blocks[2]
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: destination,
            ..parameter
        });
    function.structural_places.extend([
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
                block: function.blocks[2].id,
                position: 0,
            },
        },
    ]);
    function.declared_places.extend([source, destination]);
    let node = &mut function.blocks[0].nodes[1];
    let AbstractOperation::Conditional { when_false, .. } = &mut node.operation else {
        unreachable!()
    };
    let binding = abstract_operations::AbstractStructuralBinding {
        parameter: destination,
        argument: StructuralArgument {
            place: source,
            path: Vec::new(),
            access: StructuralAccess::SharedBorrow,
        },
    };
    when_false.structural_bindings.push(binding.clone());
    node.successors[1].structural_bindings.push(binding);
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
    let contract = ConstantConditionalFoldRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        ConstantConditionalFoldRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );
}
