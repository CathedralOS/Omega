//! Primitive effects and scalar exits share the ordinary target control graph.
use super::super::{TargetLoweringRequest, lower_to_target_operations};
use super::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult, BlockId, EdgeId, IntegerSign,
    IntegerType, IntegerValue, MachineId, NativeTarget, OperationId, PlaceId, ScalarType,
    StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, TerminalAffineCleanupAction,
    ValueId, ValueShape, identity,
};
use target_operations::ScalarFunctionAbi;

fn fixture(runtime: bool) -> AbstractOperationPlan {
    let machine = MachineId::new(1).unwrap();
    let block = BlockId::new(1).unwrap();
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let input = ValueId::new(1).unwrap();
    let returned = ValueId::new(2).unwrap();
    let result = AbstractResult {
        value: ValueId::new(3).unwrap(),
        scalar_type: scalar,
    };
    let destination = StructuralParameterDeclaration {
        place: PlaceId::new(1).unwrap(),
        position: 0,
        is_self: false,
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let mut operations = Vec::new();
    if !runtime {
        operations.push(AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(1).unwrap(),
            result: input,
            scalar_type: scalar,
            value: IntegerValue::Unsigned(0),
        });
    }
    operations.extend([
        AbstractOperation::WriteOnlyPrimitiveStore {
            path: Vec::new(),
            psi_operation: OperationId::new(2).unwrap(),
            destination: destination.clone(),
            value: AbstractResult {
                value: input,
                scalar_type: scalar,
            },
        },
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(3).unwrap(),
            result: returned,
            scalar_type: scalar,
            value: IntegerValue::Unsigned(0),
        },
        AbstractOperation::Return {
            psi_edge: EdgeId::new(1).unwrap(),
            result: result.value,
            value: returned,
            scalar_type: scalar,
            cleanup_actions: Vec::new(),
        },
    ]);
    AbstractOperationPlan {
        psi: identity(),
        entry: machine,
        structural_types: vec![StructuralTypeDeclaration {
            id: destination.structural_type,
            identity: "u64".into(),
            shape: StructuralTypeShape::PrimitiveScalar(scalar),
        }]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: if runtime {
                vec![AbstractParameter {
                    value: input,
                    scalar_type: scalar,
                }]
            } else {
                Vec::new()
            },
            structural_parameters: vec![destination],
            result: AbstractFunctionResult::Scalar(result),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations,
        }],
    }
}

#[test]
fn scalar_primitive_store_rejects_unsupported_access_types_order_and_exits() {
    for mutation in [
        "shared",
        "owned",
        "affine",
        "referent",
        "store value",
        "late definition",
        "return identity",
        "return type",
        "cleanup",
    ] {
        let mut source = fixture(false);
        let function = &mut source.functions[0];
        match mutation {
            "shared" | "owned" | "affine" => {
                let parameter = &mut function.structural_parameters[0];
                match mutation {
                    "shared" => parameter.access = StructuralAccess::SharedBorrow,
                    "owned" => parameter.access = StructuralAccess::Owned,
                    _ => parameter.multiplicity = StructuralMultiplicity::Affine,
                }
                let AbstractOperation::WriteOnlyPrimitiveStore { destination, .. } =
                    &mut function.operations[1]
                else {
                    panic!("store")
                };
                *destination = parameter.clone();
            }
            "referent" => {
                source.structural_types.make_mut()[0].shape =
                    StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)
            }
            "store value" => {
                let AbstractOperation::WriteOnlyPrimitiveStore { value, .. } =
                    &mut function.operations[1]
                else {
                    panic!("store")
                };
                value.value = ValueId::new(2).unwrap();
            }
            "late definition" => function.operations.swap(0, 1),
            _ => {
                let AbstractOperation::Return {
                    result,
                    scalar_type,
                    cleanup_actions,
                    ..
                } = &mut function.operations[3]
                else {
                    panic!("return")
                };
                match mutation {
                    "return identity" => *result = ValueId::new(1).unwrap(),
                    "return type" => *scalar_type = ScalarType::Boolean,
                    _ => cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(
                        PlaceId::new(1).unwrap(),
                    )),
                }
            }
        }
        assert!(
            lower_to_target_operations(
                &source,
                TargetLoweringRequest::new(NativeTarget::macos_arm64())
            )
            .is_err(),
            "accepted {mutation}"
        );
    }
}

#[test]
fn boolean_primitive_store_publishes_exact_borrow_and_scalar_return_abi() {
    let mut source = fixture(true);
    source.structural_types.make_mut()[0].identity = "bool".into();
    source.structural_types.make_mut()[0].shape =
        StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean);
    source.functions[0].parameters[0].scalar_type = ScalarType::Boolean;
    let AbstractOperation::WriteOnlyPrimitiveStore { value, .. } =
        &mut source.functions[0].operations[0]
    else {
        panic!("primitive store");
    };
    value.scalar_type = ScalarType::Boolean;
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let target =
            lower_to_target_operations(&source, TargetLoweringRequest::new(native)).unwrap();
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let function = &target.functions[0];
        let abi = function.mixed_structural_scalar_abi.as_ref().unwrap();
        let graph = &function.graph;
        assert_eq!(graph.call_plan, abi.call_plan);
        assert_eq!(graph.scalar_parameters, abi.scalar_parameters);
        assert_eq!(graph.parameters, abi.structural_parameters);
        assert_eq!(abi.scalar_parameters[0].scalar_type, ScalarType::Boolean);
        assert_eq!(
            abi.scalar_parameters[0].placement.shape,
            ValueShape::integer(1, 1)
        );
        assert_eq!(
            abi.structural_parameters[0].shape,
            ValueShape::borrowed_reference(1, 1)
        );
        assert_eq!(abi.result.placement.shape, ValueShape::integer(8, 8));
        for mutation in 0..2 {
            let mut changed = target.clone();
            let published = changed.functions[0]
                .mixed_structural_scalar_abi
                .as_mut()
                .unwrap();
            match mutation {
                0 => published.structural_parameters[0].shape = ValueShape::integer(1, 1),
                _ => published.structural_parameters[0].access = StructuralAccess::Owned,
            }
            assert!(
                crate::validate_abstract_to_target_translation(&source, native, &changed).is_err(),
                "Boolean ABI mutation {mutation}"
            );
        }
    }
}

/// A standalone receiving entrance — the graph's own scalar rows or the
/// published mixed ABI — must bind each scalar and result row to the declared
/// value, type, and derived placement. A coherently planned but substituted
/// row, or a scalar-only ABI on a structural function, is a substituted
/// entrance even when its physical shape matches the honest one.
#[test]
fn standalone_receiving_entrances_reject_substituted_scalar_rows() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = fixture(true);
        let target =
            lower_to_target_operations(&source, TargetLoweringRequest::new(native)).unwrap();
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        for mutation in 0..9 {
            let mut changed = target.clone();
            let function = &mut changed.functions[0];
            match mutation {
                0 => function.graph.scalar_parameters[0].value = ValueId::new(99).unwrap(),
                1 => function.graph.scalar_parameters[0].scalar_type = ScalarType::Boolean,
                2 => {
                    function.graph.scalar_parameters[0].placement =
                        function.graph.call_plan.parameters[1].clone();
                }
                3 => function.graph.scalar_parameters.clear(),
                4 => {
                    function
                        .mixed_structural_scalar_abi
                        .as_mut()
                        .unwrap()
                        .scalar_parameters[0]
                        .value = ValueId::new(99).unwrap();
                }
                5 => {
                    let abi = function.mixed_structural_scalar_abi.as_mut().unwrap();
                    abi.scalar_parameters[0].placement = abi.call_plan.parameters[1].clone();
                }
                6 => {
                    function
                        .mixed_structural_scalar_abi
                        .as_mut()
                        .unwrap()
                        .result
                        .value = ValueId::new(99).unwrap();
                }
                7 => {
                    function
                        .mixed_structural_scalar_abi
                        .as_mut()
                        .unwrap()
                        .result
                        .scalar_type = ScalarType::Boolean;
                }
                _ => {
                    let abi = function.mixed_structural_scalar_abi.as_ref().unwrap();
                    function.scalar_abi = Some(ScalarFunctionAbi {
                        call_plan: abi.call_plan.clone(),
                        parameters: abi.scalar_parameters.clone(),
                        result: abi.result.clone(),
                    });
                }
            }
            assert!(
                crate::validate_abstract_to_target_translation(&source, native, &changed).is_err(),
                "standalone entrance substitution {mutation}"
            );
        }
    }
}
