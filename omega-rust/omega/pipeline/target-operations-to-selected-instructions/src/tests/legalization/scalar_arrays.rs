//! Target and legalized readers reconstruct leaves, storage and returned identity.
use crate::{legalize_target_operations, validate_legalized_operations};
use abstract_operations::{AbstractFunctionResult, AbstractOperation as O};
use calling_conventions::ValueShape;
use legalized_operations::LegalizedScalarInstructionKind as K;
use semantic_vocabulary::{
    EdgeId, FuelScheduleIdentity, OperationId, PlaceId, ScalarType, StructuralTypeId, ValueId,
};
use target_operations::{TargetOperation, TargetStructuralHomeLayout, TargetUnitOperation};
use terminal_psi::{StructuralMultiplicity, StructuralTypeShape};

mod floating_parameters;

fn fixture(
    length: u64,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let root = StructuralTypeId::new(1).unwrap();
    let leaf = StructuralTypeId::new(2).unwrap();
    source.structural_types = vec![
        terminal_psi::StructuralTypeDeclaration {
            id: root,
            identity: "test::array".into(),
            shape: StructuralTypeShape::FixedArray {
                element: leaf,
                length,
            },
        },
        terminal_psi::StructuralTypeDeclaration {
            id: leaf,
            identity: "test::boolean".into(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        },
    ];
    let value = ValueId::new(1).unwrap();
    let other = ValueId::new(2).unwrap();
    let place = PlaceId::new(1).unwrap();
    let function = &mut source.functions[0];
    function.result =
        AbstractFunctionResult::Structural(terminal_psi::StructuralResultDeclaration {
            place: PlaceId::new(2).unwrap(),
            structural_type: root,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    function.operations = vec![
        O::BooleanConstant {
            psi_operation: OperationId::new(1).unwrap(),
            result: value,
            value: true,
        },
        O::BooleanConstant {
            psi_operation: OperationId::new(2).unwrap(),
            result: other,
            value: false,
        },
        O::EstablishScalarArray {
            psi_operation: OperationId::new(3).unwrap(),
            result: terminal_psi::StructuralOperationResult {
                place,
                structural_type: root,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
            elements: if length == 0 {
                Vec::new()
            } else {
                vec![value, other, value]
            },
        },
        O::ReturnStructural {
            psi_edge: EdgeId::new(1).unwrap(),
            source: place,
            returned_claims: Vec::new(),
            trivial_affine_locals: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    ];
    if length == 0 {
        // Empty construction has no scalar operands; do not introduce unused
        // Boolean definitions whose materialization is outside this fixture.
        function.operations.drain(..2);
    }
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        target::NativeTarget::linux_x64(),
    )
    .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
    (source, target, unit)
}

#[test]
fn scalar_array_graph_returns_replay_exact_abi_without_optional_mirrors() {
    let (mut source, _, _) = fixture(0);
    let integer =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 16)
            .unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let result = ValueId::new(9).unwrap();
    let returned = ValueId::new(8).unwrap();
    source.functions[0].result =
        AbstractFunctionResult::Scalar(abstract_operations::AbstractResult {
            value: result,
            scalar_type,
        });
    source.functions[0].operations.pop();
    source.functions[0].operations.extend([
        O::IntegerConstant {
            psi_operation: OperationId::new(4).unwrap(),
            result: returned,
            scalar_type,
            value: semantic_vocabulary::IntegerValue::Unsigned(65535),
        },
        O::Return {
            psi_edge: EdgeId::new(1).unwrap(),
            result,
            value: returned,
            scalar_type,
            cleanup_actions: Vec::new(),
        },
    ]);
    let mut target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        target::NativeTarget::linux_x64(),
    )
    .unwrap();
    target.functions[0].scalar_abi = None;
    target.functions[0].mixed_structural_scalar_abi = None;
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
    validate_legalized_operations(&target, &source, &unit, legalized.plan().clone()).unwrap();
    for mutation in 0..4 {
        let mut changed = target.clone();
        let TargetOperation::ControlGraph(graph) = &mut changed.functions[0].operation else {
            panic!("array scalar graph");
        };
        match mutation {
            0 => graph.call_plan.result.as_mut().unwrap().shape = ValueShape::integer(4, 4),
            1 => {
                let target_operations::TargetControlTerminator::ReturnScalar { expression, .. } =
                    &mut graph.blocks[0].terminator
                else {
                    panic!("scalar return");
                };
                let target_operations::TargetScalarExpression::Integer { scalar_type, .. } =
                    expression
                else {
                    panic!("integer return");
                };
                *scalar_type = semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Signed,
                    16,
                )
                .unwrap();
            }
            2 => {
                let target_operations::TargetControlTerminator::ReturnScalar {
                    source_value, ..
                } = &mut graph.blocks[0].terminator
                else {
                    panic!("scalar return");
                };
                *source_value = result;
            }
            _ => {
                let calling_conventions::ValueLocation::Register { byte_size, .. } =
                    &mut graph.call_plan.result.as_mut().unwrap().locations[0]
                else {
                    panic!("register return");
                };
                *byte_size = 4;
            }
        }
        assert!(
            legalize_target_operations(&changed, &source, &unit).is_err(),
            "mutation {mutation}"
        );
        assert!(
            validate_legalized_operations(&changed, &source, &unit, legalized.plan().clone())
                .is_err(),
            "replay mutation {mutation}"
        );
    }
}

#[test]
fn array_target_replay_rejects_leaf_storage_and_producer_substitution() {
    let (source, target, unit) = fixture(3);
    legalize_target_operations(&target, &source, &unit).unwrap();
    for mutation in 0..5 {
        let mut changed = target.clone();
        let TargetOperation::ControlGraph(graph) = &mut changed.functions[0].operation else {
            panic!("graph")
        };
        let TargetUnitOperation::EstablishScalarArray {
            psi_operation,
            result_home,
            elements,
        } = &mut graph.blocks[0].operations[2]
        else {
            panic!("array")
        };
        match mutation {
            0 => elements.swap(0, 1),
            1 => {
                elements.pop();
            }
            2 => {
                result_home.layout =
                    TargetStructuralHomeLayout::Aggregate(ValueShape::integer(4, 1))
            }
            3 => result_home.result.place = PlaceId::new(99).unwrap(),
            4 => *psi_operation = OperationId::new(99).unwrap(),
            _ => unreachable!(),
        }
        assert!(
            legalize_target_operations(&changed, &source, &unit).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn array_legalized_replay_rejects_leaf_order_shape_and_result_substitution() {
    for length in [0, 3] {
        let (source, target, unit) = fixture(length);
        let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legalized.plan().clone()).unwrap();
        for mutation in 0..4 {
            let mut changed = legalized.plan().clone();
            let instruction = changed.scalar_functions[0].blocks[0]
                .instructions
                .iter_mut()
                .find(|instruction| matches!(instruction.kind, K::EstablishScalarArray { .. }))
                .unwrap();
            let K::EstablishScalarArray {
                result,
                elements,
                shape,
            } = &mut instruction.kind
            else {
                panic!("array")
            };
            match mutation {
                0 if length != 0 => elements.swap(0, 1),
                0 => elements.push(ValueId::new(1).unwrap()),
                1 => *shape = ValueShape::integer(8, 8),
                2 => result.place = PlaceId::new(99).unwrap(),
                3 => result.structural_type = StructuralTypeId::new(2).unwrap(),
                _ => unreachable!(),
            }
            assert!(
                validate_legalized_operations(&target, &source, &unit, changed).is_err(),
                "length {length}, mutation {mutation}"
            );
        }
    }
}

#[test]
fn incoming_array_identity_rejects_substituted_parameter_storage() {
    let (mut source, _, _) = fixture(0);
    let StructuralTypeShape::FixedArray { length, .. } = &mut source.structural_types[0].shape
    else {
        panic!("array")
    };
    *length = 1;
    let function = &mut source.functions[0];
    function.operations.remove(0);
    function
        .structural_parameters
        .push(terminal_psi::StructuralParameterDeclaration {
            place: PlaceId::new(1).unwrap(),
            position: 0,
            is_self: false,
            structural_type: StructuralTypeId::new(1).unwrap(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: terminal_psi::StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        target::NativeTarget::linux_x64(),
    )
    .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
    let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
    validate_legalized_operations(&target, &source, &unit, legalized.plan().clone()).unwrap();
    for mutation in 0..5 {
        let mut changed = target.clone();
        let TargetOperation::ControlGraph(graph) = &mut changed.functions[0].operation else {
            panic!("graph")
        };
        let target_operations::TargetControlTerminator::ReturnStructural {
            source: target_operations::TargetStructuralReturnSource::Parameter(parameter),
            ..
        } = &mut graph.blocks[0].terminator
        else {
            panic!("parameter return")
        };
        match mutation {
            0 => parameter.place = PlaceId::new(99).unwrap(),
            1 => parameter.structural_type = StructuralTypeId::new(2).unwrap(),
            2 => parameter.multiplicity = StructuralMultiplicity::Affine,
            3 => parameter.access = terminal_psi::StructuralAccess::SharedBorrow,
            4 => parameter.shape = ValueShape::integer(8, 8),
            _ => unreachable!(),
        }
        assert!(
            legalize_target_operations(&changed, &source, &unit).is_err(),
            "mutation {mutation}"
        );
    }
}
