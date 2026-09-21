//! Indirect requirement invocation through a borrowed descriptor parameter
//! legalizes to `DynamicParameterCall` retaining the descriptor ABI row, the
//! requirement slot, the erased call plan, and the table offset; replay
//! re-derives each from the roster and target ABI.

use crate::{legalize_target_operations, select_instructions, validate_legalized_operations};
use abstract_operations::{AbstractOperation, AbstractParameterDynamicDispatch, AbstractResult};
use legalized_operations::{LegalizedScalarInstructionKind, LegalizedScalarTerminator};
use semantic_vocabulary::{
    EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, OperationId, ScalarType,
    ValueId,
};
use target::NativeTarget;
use target_operations::TargetUnitOperation;
use terminal_psi::{
    ClosedConformanceCallableResult, StructuralAccess, TerminalDynamicDescriptorParameter,
    TerminalDynamicRequirement, TerminalParameterDynamicDispatch,
};

fn i32_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
}

fn descriptor_parameter(
    machine: MachineId,
    requirements: Vec<TerminalDynamicRequirement>,
) -> TerminalDynamicDescriptorParameter {
    TerminalDynamicDescriptorParameter {
        owner: machine,
        ordinal: 0,
        source_position: 0,
        trait_identity: "test::Shape".into(),
        access: StructuralAccess::SharedBorrow,
        requirements,
    }
}

/// The admitted scalar shape: a non-entry helper receives one borrowed
/// two-word descriptor, invokes its `code() -> i32` requirement once, and
/// returns the call result.
pub(crate) fn scalar_fixture(
    native: NativeTarget,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let machine = MachineId::new(1).unwrap();
    let scalar_type = i32_type();
    let operation = OperationId::new(1).unwrap();
    let result = AbstractResult {
        value: ValueId::new(1).unwrap(),
        scalar_type,
    };
    let parameter = descriptor_parameter(
        machine,
        vec![TerminalDynamicRequirement {
            slot: 0,
            declaring_trait_identity: "test::Shape".into(),
            public_requirement_identity: "test::Shape::code".into(),
            family_tuple: Vec::new(),
            result: ClosedConformanceCallableResult::I32,
        }],
    );
    source.functions[0].result = abstract_operations::AbstractFunctionResult::Scalar(result);
    source.functions[0].operations = vec![
        AbstractOperation::DynamicDescriptorParameter {
            parameter: parameter.clone(),
        },
        AbstractOperation::CallDynamicParameterScalar {
            psi_operation: operation,
            result,
            dynamic_dispatch: AbstractParameterDynamicDispatch {
                parameter,
                dispatch: TerminalParameterDynamicDispatch {
                    owner: machine,
                    operation,
                    parameter_ordinal: 0,
                    requirement_slot: 0,
                },
            },
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
        AbstractOperation::Return {
            psi_edge: EdgeId::new(1).unwrap(),
            result: result.value,
            value: result.value,
            scalar_type,
            cleanup_actions: Vec::new(),
        },
    ];
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

/// The Unit-result form invokes a `act()` requirement and discards into a
/// plain unit return.
pub(crate) fn unit_fixture(
    native: NativeTarget,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let machine = MachineId::new(1).unwrap();
    let operation = OperationId::new(1).unwrap();
    let parameter = descriptor_parameter(
        machine,
        vec![TerminalDynamicRequirement {
            slot: 0,
            declaring_trait_identity: "test::Action".into(),
            public_requirement_identity: "test::Action::act".into(),
            family_tuple: Vec::new(),
            result: ClosedConformanceCallableResult::Unit,
        }],
    );
    source.functions[0].operations = vec![
        AbstractOperation::DynamicDescriptorParameter {
            parameter: parameter.clone(),
        },
        AbstractOperation::CallDynamicParameterUnit {
            psi_operation: operation,
            dynamic_dispatch: AbstractParameterDynamicDispatch {
                parameter,
                dispatch: TerminalParameterDynamicDispatch {
                    owner: machine,
                    operation,
                    parameter_ordinal: 0,
                    requirement_slot: 0,
                },
            },
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
        AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(1).unwrap(),
            cleanup_actions: Vec::new(),
        },
    ];
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

#[test]
fn parameter_dynamic_scalar_call_legalizes_with_replayed_contract() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (source, target, unit) = scalar_fixture(native);
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        let function = &legal.plan().scalar_functions[0];
        let row = &function.blocks[0].instructions[0];
        let LegalizedScalarInstructionKind::DynamicParameterCall(call) = &row.kind else {
            panic!("descriptor parameter call kind")
        };
        call.validate_shape().unwrap();
        assert_eq!(row.result.unwrap().value, ValueId::new(1).unwrap());
        assert_eq!(
            call.table_slot_byte_offset, 0,
            "slot zero sits at the table base"
        );
        let LegalizedScalarTerminator::Return(returned) = &function.blocks[0].terminator else {
            panic!("scalar return")
        };
        assert!(
            matches!(returned.value, legalized_operations::LegalizedScalarReturnValue::Value { value, .. } if value == ValueId::new(1).unwrap())
        );
    }
}

#[test]
fn parameter_dynamic_unit_call_legalizes_without_result_home() {
    let (source, target, unit) = unit_fixture(NativeTarget::linux_x64());
    let legal = legalize_target_operations(&target, &source, &unit).unwrap();
    let function = &legal.plan().scalar_functions[0];
    let row = &function.blocks[0].instructions[0];
    let LegalizedScalarInstructionKind::DynamicParameterCall(call) = &row.kind else {
        panic!("descriptor parameter call kind")
    };
    call.validate_shape().unwrap();
    assert!(row.result.is_none());
    assert!(call.result_home.is_none());
}

#[test]
fn parameter_dynamic_call_replay_rejects_contract_substitution() {
    let (source, target, unit) = scalar_fixture(NativeTarget::linux_x64());
    let legal = legalize_target_operations(&target, &source, &unit).unwrap();
    let identity = legalized_operations::legalized_operation_plan_identity(legal.plan());
    for mutation in 0..6 {
        let mut changed = legal.plan().clone();
        let row = &mut changed.scalar_functions[0].blocks[0].instructions[0];
        let LegalizedScalarInstructionKind::DynamicParameterCall(call) = &mut row.kind else {
            unreachable!()
        };
        match mutation {
            0 => call.table_slot_byte_offset = 8,
            1 => call.requirement.slot = 1,
            2 => call.parameter_abi.parameter.ordinal = 1,
            3 => call.dynamic_dispatch.dispatch.requirement_slot = 1,
            4 => {
                call.result_home = call.result_home.map(|mut home| {
                    home.scalar_type = ScalarType::Boolean;
                    home
                })
            }
            5 => call.requirement.result = ClosedConformanceCallableResult::Unit,
            _ => unreachable!(),
        }
        assert_ne!(
            legalized_operations::legalized_operation_plan_identity(&changed),
            identity
        );
        assert!(
            validate_legalized_operations(&target, &source, &unit, changed).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn parameter_dynamic_call_target_replay_rejects_stored_field_substitution() {
    let (source, target, unit) = scalar_fixture(NativeTarget::linux_x64());
    for mutation in 0..4 {
        let mut changed = target.clone();
        let function_index = changed
            .functions
            .iter()
            .position(|function| {
                function
                    .graph
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .any(|operation| {
                        matches!(
                            operation,
                            TargetUnitOperation::DynamicParameterScalarCall { .. }
                        )
                    })
            })
            .unwrap();
        let operation = changed.functions[function_index]
            .graph
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find(|operation| {
                matches!(
                    operation,
                    TargetUnitOperation::DynamicParameterScalarCall { .. }
                )
            })
            .unwrap();
        let TargetUnitOperation::DynamicParameterScalarCall {
            requirement,
            table_slot_byte_offset,
            parameter_abi,
            result_home,
            ..
        } = operation
        else {
            unreachable!()
        };
        match mutation {
            0 => *table_slot_byte_offset = 8,
            1 => requirement.slot = 1,
            2 => parameter_abi.parameter.ordinal = 1,
            3 => result_home.scalar_type = ScalarType::Boolean,
            _ => unreachable!(),
        }
        assert!(
            legalize_target_operations(&changed, &source, &unit).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn parameter_dynamic_call_selection_still_rejects_the_new_kind() {
    // The frontier: legalization admits and replays the descriptor-parameter
    // call, but selection names no `SelectedInstructionKind` for it yet, so
    // the restored lane ends at the selected-instruction boundary.
    let (source, target, unit) = scalar_fixture(NativeTarget::linux_x64());
    let legal = legalize_target_operations(&target, &source, &unit).unwrap();
    let environment =
        register_environment::baseline_target_register_environment(NativeTarget::linux_x64())
            .unwrap();
    let constraints = crate::selection_constraints(&legal, &environment);
    assert!(
        select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .is_err()
    );
}
