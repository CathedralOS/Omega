//! Scalar-returning stores retain independent ABI, source, and physical custody.
use super::{
    AbstractOperation, AbstractOperationPlan, AbstractResult, BindingRelevance,
    FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, NativeTarget, OperationId,
    PsiOptimizationUnit, ScalarType, StructuralAccess, StructuralFieldDeclaration,
    StructuralFieldId, StructuralFieldType, StructuralTypeShape, TargetOperationPlan,
    TargetUnitOperation, TargetUnitWriteOnlyPrimitiveStoreSource, ValueId,
};
use crate::legalize_target_operations;
use crate::tests::legalization::primitive_stores::integer;
use crate::tests::legalization::primitive_stores::reject_target;
use crate::validate_legalized_operations;
use abstract_operations::AbstractFunctionResult;
use target_operations::{TargetControlGraph, TargetControlTerminator};

fn fixture(
    native: NativeTarget,
    runtime: bool,
    access: StructuralAccess,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    fixture_with_scalar(native, runtime, access, integer(IntegerSign::Unsigned, 64))
}

fn fixture_with_scalar(
    native: NativeTarget,
    runtime: bool,
    access: StructuralAccess,
    scalar: ScalarType,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let (mut source, _, _) = super::fixture(native, scalar, !runtime);
    let function = &mut source.functions[0];
    function.structural_parameters[0].access = access;
    for operation in &mut function.operations {
        if let AbstractOperation::WriteOnlyPrimitiveStore { destination, .. } = operation {
            destination.access = access;
        }
    }
    let AbstractOperation::ReturnUnit { psi_edge, .. } = function.operations.pop().unwrap() else {
        panic!("Unit fixture exit")
    };
    let returned = ValueId::new(7).unwrap();
    let result = ValueId::new(8).unwrap();
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: result,
        scalar_type: scalar,
    });
    function.operations.extend([
        if scalar == ScalarType::Boolean {
            AbstractOperation::BooleanConstant {
                psi_operation: OperationId::new(3).unwrap(),
                result: returned,
                value: true,
            }
        } else { AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(3).unwrap(),
            result: returned,
            scalar_type: scalar,
            value: if matches!(scalar, ScalarType::Integer(integer) if integer.sign() == IntegerSign::Signed) {
                IntegerValue::Signed(0)
            } else { IntegerValue::Unsigned(0) },
        } },
        AbstractOperation::Return {
            psi_edge,
            result,
            value: returned,
            scalar_type: scalar,
            cleanup_actions: Vec::new(),
        },
    ]);
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
    )
    .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (source, target, unit)
}

fn graph(target: &mut TargetOperationPlan) -> &mut TargetControlGraph {
    &mut target.functions[0].graph
}

#[test]
fn record_field_store_with_scalar_result_retains_borrow_access_custody() {
    let scalar = integer(IntegerSign::Signed, 32);
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (mut source, _, _) =
            fixture_with_scalar(native, true, StructuralAccess::MutableBorrow, scalar);
        let field = StructuralFieldId::new(1).unwrap();
        source.structural_types.make_mut()[0].shape = StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: field,
                identity: "value".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(scalar),
            }],
        };
        let store = source.functions[0]
            .operations
            .iter_mut()
            .find(|operation| {
                matches!(operation, AbstractOperation::WriteOnlyPrimitiveStore { .. })
            })
            .unwrap();
        let AbstractOperation::WriteOnlyPrimitiveStore {
            psi_operation,
            destination,
            value,
            ..
        } = store
        else {
            panic!("store")
        };
        *store = AbstractOperation::StructuralScalarFieldStore {
            range_obligation: None,
            psi_operation: *psi_operation,
            destination: destination.clone(),
            value: *value,
            path: Vec::new(),
            field,
        };
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .unwrap();
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legalized.plan().clone()).unwrap();
        for access in [StructuralAccess::SharedBorrow, StructuralAccess::Owned] {
            let mut changed = target.clone();
            let TargetUnitOperation::StructuralScalarFieldStore { destination, .. } =
                changed.functions[0].graph.blocks[0]
                    .operations
                    .iter_mut()
                    .find(|operation| {
                        matches!(
                            operation,
                            TargetUnitOperation::StructuralScalarFieldStore { .. }
                        )
                    })
                    .unwrap()
            else {
                panic!("field store")
            };
            destination.access = access;
            assert!(legalize_target_operations(&changed, &source, &unit).is_err());
            assert!(
                validate_legalized_operations(&changed, &source, &unit, legalized.plan().clone())
                    .is_err()
            );
        }
        let mut changed = source.clone();
        changed.functions[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
        let AbstractOperation::StructuralScalarFieldStore { destination, .. } = changed.functions
            [0]
        .operations
        .iter_mut()
        .find(|operation| {
            matches!(
                operation,
                AbstractOperation::StructuralScalarFieldStore { .. }
            )
        })
        .unwrap() else {
            panic!("source field store")
        };
        destination.access = StructuralAccess::SharedBorrow;
        if let Ok(changed_target) =
            abstract_operations_to_target_operations::lower_to_target_operations(
                &changed,
                abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
            )
        {
            let changed_unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &changed,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap();
            assert!(
                legalize_target_operations(&changed_target, &changed, &changed_unit).is_err(),
                "a scalar result must not authorize writing through a shared borrow"
            );
        }
    }
}

fn store(target: &mut TargetOperationPlan) -> &mut TargetUnitOperation {
    graph(target).blocks[0]
        .operations
        .iter_mut()
        .find(|operation| {
            matches!(
                operation,
                TargetUnitOperation::WriteOnlyPrimitiveStore { .. }
            )
        })
        .unwrap()
}

#[test]
fn scalar_primitive_store_target_rejects_source_and_return_substitutions() {
    for runtime in [false, true] {
        let (source, target, unit) = fixture(
            NativeTarget::macos_arm64(),
            runtime,
            StructuralAccess::MutableBorrow,
        );
        let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
        for mutation in [
            "access",
            "referent",
            "placement",
            "block storage",
            "source value",
            "source type",
            "store order",
            "return operand",
            "return edge",
            "result identity",
            "result type",
            "result placement",
            "graph result",
            "graph parameter",
            "graph scalar",
        ] {
            let mut changed = target.clone();
            match mutation {
                "access" | "referent" | "placement" | "block storage" | "source value"
                | "source type" => {
                    let TargetUnitOperation::WriteOnlyPrimitiveStore {
                        destination,
                        destination_type,
                        destination_storage,
                        source: stored,
                        ..
                    } = store(&mut changed)
                    else {
                        panic!("store")
                    };
                    match mutation {
                        "access" => destination.access = StructuralAccess::SharedBorrow,
                        "referent" => {
                            destination_type.shape =
                                StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)
                        }
                        "placement" => {
                            let target_operations::TargetStructuralArgumentSource::Placement(
                                placement,
                            ) = destination_storage
                            else {
                                panic!("entry placement");
                            };
                            placement.shape.byte_size = 4;
                        }
                        "block storage" => {
                            *destination_storage =
                                target_operations::TargetStructuralArgumentSource::BlockParameter {
                                    block: source.functions[0].entry,
                                    place: destination.place,
                                };
                        }
                        "source value" | "source type" => match stored {
                            TargetUnitWriteOnlyPrimitiveStoreSource::Parameter {
                                source_value,
                                scalar_type,
                                ..
                            } => {
                                if mutation == "source value" {
                                    *source_value = ValueId::new(6).unwrap();
                                } else {
                                    *scalar_type = ScalarType::Boolean;
                                }
                            }
                            TargetUnitWriteOnlyPrimitiveStoreSource::IntegerImmediate {
                                source_value,
                                scalar_type,
                                ..
                            } => {
                                if mutation == "source value" {
                                    *source_value = ValueId::new(7).unwrap();
                                } else {
                                    *scalar_type =
                                        IntegerType::new(IntegerSign::Signed, 64).unwrap();
                                }
                            }
                            _ => panic!("parameter or literal"),
                        },
                        _ => unreachable!(),
                    }
                }
                "store order" => graph(&mut changed).blocks[0].operations.swap(0, 1),
                "graph result" => graph(&mut changed).call_plan.result = None,
                "graph parameter" => {
                    graph(&mut changed).parameters[0].access = StructuralAccess::Owned
                }
                "graph scalar" => graph(&mut changed).scalar_parameters.clear(),
                "return operand" | "return edge" => {
                    let TargetControlTerminator::ReturnScalar {
                        source_value,
                        psi_edge,
                        ..
                    } = &mut graph(&mut changed).blocks[0].terminator
                    else {
                        panic!("return")
                    };
                    if mutation == "return operand" {
                        *source_value = ValueId::new(5).unwrap();
                    } else {
                        *psi_edge = semantic_vocabulary::EdgeId::new(99).unwrap();
                    }
                }
                _ => {
                    let abi = changed.functions[0]
                        .mixed_structural_scalar_abi
                        .as_mut()
                        .unwrap();
                    match mutation {
                        "result identity" => abi.result.value = ValueId::new(7).unwrap(),
                        "result type" => abi.result.scalar_type = ScalarType::Boolean,
                        _ => {
                            abi.result.placement.shape =
                                calling_conventions::ValueShape::integer(4, 4)
                        }
                    }
                }
            }
            if mutation == "graph scalar" && !runtime {
                continue;
            }
            reject_target(&source, &changed, &unit, legalized.plan());
        }
    }
}
