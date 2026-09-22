//! The retained call plan must replay to the declared ABI's canonical
//! evaluation, not merely agree with the arguments that echo it.
use super::{
    CallSignature, CallingPolicy, IntegerSign, LegalizedScalarArgument, LegalizedScalarCall,
    LegalizedScalarFunction, ScalarType, ValueId, ValueLocation, ValueShape, evaluate_call_plan,
    validate,
};
use crate::selection::scalar_call_abi::unit_key;
use calling_conventions::RegisterSet;
use legalized_operations::LegalizedScalarParameter;
use optimization_unit::ValueDefinitionSite;
use semantic_vocabulary::{BlockId, IntegerType, MachineId, OperationId};

fn scalar_call(
    target: target::NativeTarget,
    arguments: usize,
) -> (LegalizedScalarFunction, LegalizedScalarCall) {
    let policy = CallingPolicy::native_for_target(target);
    let shape = ValueShape::integer(8, 8);
    let call_plan = evaluate_call_plan(
        policy,
        &CallSignature {
            parameters: vec![shape; arguments],
            result: Some(shape),
        },
    )
    .unwrap();
    let entry_plan = evaluate_call_plan(
        policy,
        &CallSignature {
            parameters: vec![shape; arguments],
            result: None,
        },
    )
    .unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let block = BlockId::new(1).unwrap();
    let source = LegalizedScalarFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        provenance: target_operations::TerminalPsiProvenance {
            operations: Vec::new(),
            edges: Vec::new(),
        },
        call_plan: entry_plan.clone(),
        parameters: entry_plan
            .parameters
            .iter()
            .enumerate()
            .map(|(index, placement)| LegalizedScalarParameter {
                value: ValueId::new(u64::try_from(index + 1).unwrap()).unwrap(),
                scalar_type,
                definition_site: ValueDefinitionSite::Node {
                    block,
                    node: u32::try_from(index).unwrap(),
                },
                placement: placement.clone(),
            })
            .collect(),
        structural: None,
        entry_block: block,
        blocks: Vec::new(),
    };
    let result_placement = call_plan.result.clone();
    let call = LegalizedScalarCall {
        structural_result: None,
        source: legalized_operations::NativeCallOrigin::Authored,
        callee: MachineId::new(2).unwrap(),
        arguments: call_plan
            .parameters
            .iter()
            .enumerate()
            .map(|(index, placement)| LegalizedScalarArgument::Scalar {
                source: source.parameters[index].value,
                placement: placement.clone(),
            })
            .collect(),
        call_plan,
        result_placement,
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    (source, call)
}

#[test]
fn admitted_scalar_calls_replay_the_declared_abi_plan() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        // Nine scalar arguments exceed every supported target's register bank,
        // so each canonical plan carries at least one stack argument.
        let (source, call) = scalar_call(target, 9);
        call.validate_shape().expect("coherent call");
        assert!(call.call_plan.parameters.iter().any(|placement| {
            placement
                .locations
                .iter()
                .any(|location| matches!(location, ValueLocation::Stack { .. }))
        }));
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let key = unit_key(&call, &environment).expect("ordinary call row");
        let row = environment.constraint(key).unwrap();
        validate(
            0,
            &source,
            &call,
            OperationId::new(1).unwrap(),
            key,
            row,
            &environment,
        )
        .expect("canonical call plan validates");
    }
}

#[test]
fn self_consistent_but_noncanonical_plans_reject() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (source, call) = scalar_call(target, 9);
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let key = unit_key(&call, &environment).unwrap();
        let row = environment.constraint(key).unwrap();
        let operation = OperationId::new(1).unwrap();
        let accepts = |call: &LegalizedScalarCall| {
            validate(0, &source, call, operation, key, row, &environment).is_ok()
        };
        assert!(accepts(&call));

        // Plan fields the operand checks never inspected were trusted from the
        // producer; each now rejects against the canonical evaluation.
        let mut clobbers = call.clone();
        clobbers.call_plan.ordinary_clobbers =
            RegisterSet::new(clobbers.call_plan.ordinary_clobbers.as_slice()[1..].to_vec());
        assert!(!accepts(&clobbers), "{target:?} dropped clobber");

        let mut alignment = call.clone();
        alignment.call_plan.stack_alignment *= 2;
        assert!(!accepts(&alignment), "{target:?} stack alignment");

        let mut shadow = call.clone();
        shadow.call_plan.shadow_bytes += 8;
        assert!(!accepts(&shadow), "{target:?} shadow bytes");

        let mut policy = call.clone();
        policy.call_plan.policy = match call.call_plan.policy {
            CallingPolicy::MicrosoftX64 => CallingPolicy::SystemVAMD64,
            _ => CallingPolicy::MicrosoftX64,
        };
        assert!(!accepts(&policy), "{target:?} relabeled policy");

        // A stack argument moved to a different aligned slot stays internally
        // consistent but no longer equals what the declared ABI assigns.
        let mut shifted = call.clone();
        let stack_index = shifted
            .call_plan
            .parameters
            .iter()
            .position(|placement| {
                placement
                    .locations
                    .iter()
                    .any(|location| matches!(location, ValueLocation::Stack { .. }))
            })
            .expect("stack argument");
        let LegalizedScalarArgument::Scalar {
            placement: argument_placement,
            ..
        } = &mut shifted.arguments[stack_index]
        else {
            panic!("scalar argument");
        };
        for placement in [
            &mut shifted.call_plan.parameters[stack_index],
            argument_placement,
        ] {
            for location in &mut placement.locations {
                let ValueLocation::Stack {
                    stack_byte_offset, ..
                } = location
                else {
                    continue;
                };
                *stack_byte_offset += 8;
            }
        }
        assert!(!accepts(&shifted), "{target:?} shifted stack slot");
    }
}
