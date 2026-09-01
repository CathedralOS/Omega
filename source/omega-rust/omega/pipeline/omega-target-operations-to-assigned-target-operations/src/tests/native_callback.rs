use crate::{AssignmentError, assign_registers, assign_registers_with_native_callbacks};
use omega_assigned_target_operations::{
    AssignedCallDestination, AssignedOperation, AssignedUnitOperation,
};
use omega_calling_conventions::{
    CallSignature, CallbackBinderRequirement, CallbackMaterialization,
    CallbackMaterializationContext, CallbackRequirementId, CallingPolicy, NativeCallbackDemand,
    NativeParameterApplication, NativeParameterId, NativePlace, StaticMachineBinderId, ValueShape,
};
use omega_function_identity::{MachineFunctionIdentity, StateKey};
use omega_target::{
    ForeignLocatorCandidate, NativeTarget, TargetProfile, normalize_foreign_locator,
};
use omega_target_operations::{
    NormalizedForeignCallBinding, ProviderExecutionBinding, ProviderPlanReportIdentity,
    TargetFunction, TargetNativeCallbackArgument, TargetOperation, TargetOperationPlan,
    TargetOperationPlanWithNativeCallbacks, TargetUnitBody, TargetUnitOperation,
    TargetUnitScalarArgumentSource, TargetUnitScalarCallArgument, TerminalPsiProvenance,
};
use psi_core::{
    BoundaryMachineId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId, ValueId,
};
use psi_symbols::SymbolHandle;
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

fn admitted_foreign_stack() -> omega_task_plans::AdmittedSameStackContribution {
    let commitment = omega_task_plans::SameStackProviderPlanCommitment::from_digest([0x73; 32]);
    omega_task_plans::admit_same_stack_contribution(
        omega_task_plans::SameStackContributionAdmissionCandidate {
            provider_plan_report_identity: 701,
            provider_plan_commitment: commitment,
            requirement_identity: "omega::test::callback_registrar()".into(),
            receipt:
                omega_task_plans::SameStackContributionAdmissionReceiptId::from_normalized_identity(
                    702,
                )
                .unwrap(),
            bytes: 64,
            alignment: 16,
        },
        701,
        commitment,
        "omega::test::callback_registrar()",
    )
    .unwrap()
}

fn fixture() -> TargetOperationPlanWithNativeCallbacks {
    let target = NativeTarget::linux_x64();
    let machine = MachineId::new(91).unwrap();
    let operation = OperationId::new(92).unwrap();
    let shape = ValueShape::integer(8, 8);
    let mut registrar = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape, shape, shape],
            result: None,
        },
    )
    .unwrap()
    .plan()
    .clone();
    let binder = StaticMachineBinderId::new(93).unwrap();
    let parameter = NativeParameterId::new(94).unwrap();
    let requirement = CallbackRequirementId::new(95).unwrap();
    let destination = NativePlace::Parameter(parameter);
    registrar.call.callback_materializations = vec![CallbackMaterialization {
        binder,
        destination: destination.clone(),
    }];
    let context = CallbackMaterializationContext {
        binders: vec![CallbackBinderRequirement {
            binder,
            requirement,
        }],
        demands: vec![NativeCallbackDemand {
            destination,
            requirement,
        }],
    };
    let first_operation = OperationId::new(96).unwrap();
    let second_operation = OperationId::new(97).unwrap();
    let first_value = ValueId::new(96).unwrap();
    let second_value = ValueId::new(97).unwrap();
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar_argument =
        |parameter_index, defining_operation, source_value, value| TargetUnitScalarCallArgument {
            parameter_index,
            source: TargetUnitScalarArgumentSource::IntegerImmediate {
                defining_operation,
                source_value,
                scalar_type,
                value: IntegerValue::Unsigned(value),
            },
            placement: registrar.call.parameters[usize::try_from(parameter_index).unwrap()].clone(),
        };
    let callback_function = MachineFunctionIdentity::callback_thunk(
        StateKey {
            machine: SymbolHandle::from_parts(1, 1),
            state: SymbolHandle::from_parts(2, 1),
            segment_index: 0,
        },
        0,
    )
    .unwrap();
    TargetOperationPlanWithNativeCallbacks {
        plan: TargetOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([0x92; 32]),
            },
            target,
            entry: machine,
            functions: vec![TargetFunction {
                machine,
                attachment: None,
                fixed_integer_scalar_abi: None,
                provenance: TerminalPsiProvenance::default(),
                operation: TargetOperation::UnitBody(TargetUnitBody {
                    structural_types: Vec::new(),
                    call_plan: omega_calling_conventions::evaluate_call_plan(
                        CallingPolicy::native_for_target(target),
                        &CallSignature::default(),
                    )
                    .unwrap(),
                    parameters: Vec::new(),
                    operations: vec![
                        TargetUnitOperation::IntegerConstant {
                            psi_operation: first_operation,
                            result: first_value,
                            scalar_type,
                            value: IntegerValue::Unsigned(11),
                        },
                        TargetUnitOperation::IntegerConstant {
                            psi_operation: second_operation,
                            result: second_value,
                            scalar_type,
                            value: IntegerValue::Unsigned(22),
                        },
                        TargetUnitOperation::NormalizedForeignCall {
                            psi_operation: operation,
                            boundary: BoundaryMachineId::new(98).unwrap(),
                            provider_execution: ProviderExecutionBinding::from_execution_record(
                                ProviderPlanReportIdentity::new(99).unwrap(),
                                100,
                                101,
                                102,
                                103,
                            )
                            .unwrap(),
                            binding: NormalizedForeignCallBinding {
                                locator: normalize_foreign_locator(
                                    ForeignLocatorCandidate::ElfVersioned {
                                        object: b"libcallback.so".to_vec(),
                                        symbol: b"register_callback".to_vec(),
                                        version: b"OMEGA_1".to_vec(),
                                    },
                                    TargetProfile::LinuxX64,
                                )
                                .unwrap(),
                                boundary_entry_plan: registrar.clone(),
                                same_stack_contribution: admitted_foreign_stack(),
                            },
                            scalar_arguments: vec![
                                scalar_argument(0, first_operation, first_value, 11),
                                scalar_argument(2, second_operation, second_value, 22),
                            ],
                            result_home: None,
                        },
                    ],
                }),
            }],
        },
        native_callback_arguments: vec![TargetNativeCallbackArgument {
            terminal_operation: operation,
            placement_index: 0,
            callback_function,
            application: NativeParameterApplication {
                parameter,
                native_ordinal: 1,
                shape,
                placement: registrar.call.parameters[1].clone(),
            },
            registrar_boundary_entry_plan: registrar,
            registrar_context: context,
            registrar_application_commitment: [0x66; 32],
        }],
    }
}

#[test]
fn direct_callback_assignment_preserves_native_slot_without_inventing_a_scalar() {
    let fixture = fixture();
    let assigned = assign_registers_with_native_callbacks(&fixture).unwrap();
    let [callback] = assigned.native_callback_arguments.as_slice() else {
        panic!("one callback assignment")
    };
    assert_eq!(
        callback.destination,
        AssignedCallDestination::Register(omega_target_operations::MachineRegister::X86Rsi)
    );
    assert_eq!(callback.target, fixture.native_callback_arguments[0]);

    let AssignedOperation::UnitBody(body) = &assigned.plan.functions[0].operation else {
        panic!("unit registrar")
    };
    let AssignedUnitOperation::NormalizedForeignCall {
        scalar_arguments, ..
    } = &body.operations[2]
    else {
        panic!("normalized registrar call")
    };
    assert_eq!(
        scalar_arguments
            .iter()
            .map(|argument| argument.parameter_index)
            .collect::<Vec<_>>(),
        vec![0, 2]
    );
    assert!(assign_registers(&fixture.plan).is_err());
}

#[test]
fn callback_assignment_rejects_multiple_or_drifted_sidecars() {
    let mut multiple = fixture();
    multiple
        .native_callback_arguments
        .push(multiple.native_callback_arguments[0].clone());
    assert_eq!(
        assign_registers_with_native_callbacks(&multiple),
        Err(AssignmentError::MultipleNativeCallbackArguments)
    );

    let mut drifted = fixture();
    drifted.native_callback_arguments[0]
        .application
        .native_ordinal = 2;
    assert_eq!(
        assign_registers_with_native_callbacks(&drifted),
        Err(AssignmentError::InvalidNativeCallbackArgument(
            OperationId::new(92).unwrap()
        ))
    );

    let mut wrong_operation = fixture();
    wrong_operation.native_callback_arguments[0].terminal_operation =
        OperationId::new(199).unwrap();
    assert_eq!(
        assign_registers_with_native_callbacks(&wrong_operation),
        Err(AssignmentError::UnknownNativeCallbackArgument(
            OperationId::new(199).unwrap()
        ))
    );

    let mut wrong_identity = fixture();
    let continuation = wrong_identity.native_callback_arguments[0]
        .callback_function
        .associated_source_continuation();
    wrong_identity.native_callback_arguments[0].callback_function =
        MachineFunctionIdentity::callback_thunk(continuation, 1).unwrap();
    assert_eq!(
        assign_registers_with_native_callbacks(&wrong_identity),
        Err(AssignmentError::InvalidNativeCallbackArgument(
            OperationId::new(92).unwrap()
        ))
    );

    let mut scalar_slot_drift = fixture();
    let TargetOperation::UnitBody(body) = &mut scalar_slot_drift.plan.functions[0].operation else {
        unreachable!()
    };
    let TargetUnitOperation::NormalizedForeignCall {
        scalar_arguments, ..
    } = &mut body.operations[2]
    else {
        unreachable!()
    };
    scalar_arguments[0].parameter_index = 2;
    assert_eq!(
        assign_registers_with_native_callbacks(&scalar_slot_drift),
        Err(AssignmentError::InvalidNativeCallbackArgument(
            OperationId::new(92).unwrap()
        ))
    );
}
