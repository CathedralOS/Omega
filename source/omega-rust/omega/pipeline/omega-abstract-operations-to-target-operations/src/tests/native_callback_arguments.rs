use super::*;
use omega_abstract_operations::AbstractFunctionResult;
use omega_calling_conventions::{
    CallbackBinderRequirement, CallbackMaterialization, CallbackMaterializationContext,
    CallbackRequirementId, NativeCallbackDemand, NativeParameterApplication, NativeParameterId,
    NativePlace, StaticMachineBinderId,
};
use omega_function_identity::{MachineFunctionIdentity, StateKey};
use psi_symbols::SymbolHandle;

fn callback_admission(operation: OperationId) -> crate::AdmittedNativeCallbackArgument {
    let target = NativeTarget::linux_x64();
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
    let binder = StaticMachineBinderId::new(81).unwrap();
    let parameter = NativeParameterId::new(82).unwrap();
    let requirement = CallbackRequirementId::new(83).unwrap();
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
    let continuation = StateKey {
        machine: SymbolHandle::from_parts(1, 1),
        state: SymbolHandle::from_parts(2, 1),
        segment_index: 0,
    };
    crate::AdmittedNativeCallbackArgument {
        terminal_operation: operation,
        placement_index: 0,
        callback_function: MachineFunctionIdentity::callback_thunk(continuation, 0).unwrap(),
        application: NativeParameterApplication {
            parameter,
            native_ordinal: 1,
            shape,
            placement: registrar.call.parameters[1].clone(),
        },
        registrar_boundary_entry_plan: registrar,
        registrar_context: context,
        registrar_application_commitment: [0x66; 32],
    }
}

fn abstract_plan(operation: OperationId) -> AbstractOperationPlan {
    let machine = MachineId::new(84).unwrap();
    let boundary = BoundaryMachineId::new(85).unwrap();
    AbstractOperationPlan {
        psi: identity(),
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: BlockId::new(84).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block: BlockId::new(84).unwrap(),
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![AbstractOperation::BoundaryCall {
                psi_operation: operation,
                result: omega_abstract_operations::AbstractBoundaryResult::Unit,
                boundary,
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                completion_claim_sources: Vec::new(),
                completion_receipts: Vec::new(),
            }],
        }],
    }
}

#[test]
fn native_callback_admission_is_unique_exact_and_transactionally_consumed() {
    let operation = OperationId::new(86).unwrap();
    let plan = abstract_plan(operation);
    let admission = callback_admission(operation);
    let bound = bind_native_callback_arguments(
        &plan,
        NativeTarget::linux_x64(),
        std::slice::from_ref(&admission),
    )
    .expect("one exact callback admission");
    assert_eq!(bound[&operation].application.native_ordinal, 1);
    assert_eq!(
        bound[&operation].callback_function,
        admission.callback_function
    );

    assert_eq!(
        bind_native_callback_arguments(
            &plan,
            NativeTarget::linux_x64(),
            &[admission.clone(), admission.clone()],
        ),
        Err(crate::LoweringError::DuplicateNativeCallbackArgument(
            operation
        )),
    );

    let second = callback_admission(OperationId::new(88).unwrap());
    assert_eq!(
        bind_native_callback_arguments(
            &plan,
            NativeTarget::linux_x64(),
            &[admission.clone(), second],
        ),
        Err(crate::LoweringError::MultipleNativeCallbackArguments),
    );

    let mut unknown = admission.clone();
    unknown.terminal_operation = OperationId::new(87).unwrap();
    assert_eq!(
        bind_native_callback_arguments(&plan, NativeTarget::linux_x64(), &[unknown]),
        Err(crate::LoweringError::UnknownNativeCallbackArgument(
            OperationId::new(87).unwrap(),
        )),
    );

    for mutate in [
        |row: &mut crate::AdmittedNativeCallbackArgument| row.application.native_ordinal = 2,
        |row: &mut crate::AdmittedNativeCallbackArgument| {
            row.application.parameter = NativeParameterId::new(99).unwrap()
        },
        |row: &mut crate::AdmittedNativeCallbackArgument| {
            row.registrar_application_commitment = [0; 32]
        },
        |row: &mut crate::AdmittedNativeCallbackArgument| {
            row.registrar_context
                .demands
                .push(row.registrar_context.demands[0].clone())
        },
    ] {
        let mut changed = admission.clone();
        mutate(&mut changed);
        assert_eq!(
            bind_native_callback_arguments(&plan, NativeTarget::linux_x64(), &[changed]),
            Err(crate::LoweringError::InvalidNativeCallbackArgument(
                operation
            )),
        );
    }

    let empty_target = omega_target_operations::TargetOperationPlan {
        psi: plan.psi,
        target: NativeTarget::linux_x64(),
        entry: plan.entry,
        functions: Vec::new(),
    };
    assert_eq!(
        validate_native_callback_target_rows(&empty_target, &bound),
        Err(crate::LoweringError::UnusedNativeCallbackArgument(
            operation
        )),
    );
}
