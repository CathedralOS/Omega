//! Attached-Unit scalar-call production and independent corruption replay.

use crate::tests::fixtures::scalar_call_unit::scalar_call_unit_fixture;
use crate::{legalize_target_operations, validate_legalized_operations};

#[test]
fn register_calls_retain_the_target_abi_home_area() {
    let (abstract_plan, _, unit) = scalar_call_unit_fixture();
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &abstract_plan,
            native,
        )
        .unwrap();
        let legalized = legalize_target_operations(&target, &abstract_plan, &unit).unwrap();
        let expected_shadow = if native == target::NativeTarget::windows_x64() {
            32
        } else {
            0
        };
        let mut changed = legalized.plan().clone();
        let call = call_mut(&mut changed.scalar_functions[0], 0);
        assert_eq!(call.call_plan.shadow_bytes, expected_shadow);
        call.call_plan.shadow_bytes = if expected_shadow == 0 { 32 } else { 0 };
        assert!(validate_legalized_operations(&target, &abstract_plan, &unit, changed).is_err());
    }
    assert_ne!(
        crate::legalization_validator_identity(),
        optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.terminal-target-legalization-independent-replay.v25"
        )
    );
}

#[test]
fn one_call_and_equal_constant_operands_have_no_fixture_topology_requirement() {
    let (mut abstract_plan, _, _) = scalar_call_unit_fixture();
    let caller = &mut abstract_plan.functions[0];
    caller.operations.remove(4);
    caller.operations.remove(3);
    let abstract_operations::AbstractOperation::IntegerConstant { value, .. } =
        &mut caller.operations[1]
    else {
        unreachable!()
    };
    *value = semantic_vocabulary::IntegerValue::Unsigned(7);
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &abstract_plan,
        target::NativeTarget::linux_x64(),
    )
    .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &abstract_plan,
        semantic_vocabulary::FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit)
        .expect("one ordinary register call with equal constants");
    assert_eq!(
        legalized.plan().scalar_functions[0].blocks[0]
            .instructions
            .len(),
        3
    );
    validate_legalized_operations(&target, &abstract_plan, &unit, legalized.plan().clone())
        .expect("independent ordered replay");
}

#[test]
fn zero_call_proposal_and_forward_references_reject() {
    let (abstract_plan, target, unit) = scalar_call_unit_fixture();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit).unwrap();
    let mut no_calls = legalized.plan().clone();
    no_calls.scalar_functions[0].blocks[0]
        .instructions
        .retain(|operation| {
            matches!(
                operation.kind,
                legalized_operations::LegalizedScalarInstructionKind::Constant(_)
            )
        });
    assert!(validate_legalized_operations(&target, &abstract_plan, &unit, no_calls).is_err());

    let mut forward = target.clone();
    let target_operations::TargetOperation::UnitBody(body) = &mut forward.functions[0].operation
    else {
        unreachable!()
    };
    let target_operations::TargetUnitOperation::ScalarCall {
        result_home: future,
        ..
    } = body.operations[3]
    else {
        unreachable!()
    };
    let target_operations::TargetUnitOperation::ScalarCall { arguments, .. } =
        &mut body.operations[2]
    else {
        unreachable!()
    };
    arguments[0].source = target_operations::TargetUnitScalarArgumentSource::Home(future);
    assert!(legalize_target_operations(&forward, &abstract_plan, &unit).is_err());
    assert!(
        validate_legalized_operations(&forward, &abstract_plan, &unit, legalized.plan().clone())
            .is_err()
    );
}

#[test]
fn substituted_register_call_plan_and_memory_effectful_callee_reject() {
    let (abstract_plan, target, unit) = scalar_call_unit_fixture();
    let mut changed = target.clone();
    let target_operations::TargetOperation::UnitBody(body) = &mut changed.functions[0].operation
    else {
        unreachable!()
    };
    let target_operations::TargetUnitOperation::ScalarCall { call_plan, .. } =
        &mut body.operations[2]
    else {
        unreachable!()
    };
    call_plan.parameters.swap(0, 1);
    assert!(legalize_target_operations(&changed, &abstract_plan, &unit).is_err());

    let mut effectful = abstract_plan.clone();
    effectful.functions[1]
        .operations
        .push(abstract_operations::AbstractOperation::PortWrite {
            psi_operation: semantic_vocabulary::OperationId::new(900).unwrap(),
            service: semantic_vocabulary::ServiceId::new(901).unwrap(),
            port: 1,
            value: 2,
        });
    assert!(legalize_target_operations(&target, &effectful, &unit).is_err());
}

#[test]
fn ordered_call_custody_has_a_new_identity_and_validator() {
    let (abstract_plan, target, unit) = scalar_call_unit_fixture();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit).unwrap();
    assert_ne!(
        legalized_operations::legalized_operation_plan_identity(legalized.plan()),
        legalized_operations::legalized_operation_plan_identity_v22_legacy(legalized.plan())
    );
    assert_ne!(
        crate::legalization_validator_identity(),
        crate::legalization_validator_identity_v22_legacy()
    );
}

#[test]
fn exact_u64_equality_three_call_chain_is_produced_and_replayed() {
    let (abstract_plan, target, unit) = scalar_call_unit_fixture();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit)
        .expect("exact attached-Unit scalar call chain legalizes");
    assert_eq!(legalized.plan().scalar_functions.len(), 1);
    assert_eq!(legalized.plan().functions.len(), 1);
    assert_eq!(legalized.receipt().function_count(), 2);
    let function = &legalized.plan().scalar_functions[0];
    let calls = function.blocks[0]
        .instructions
        .iter()
        .filter_map(|operation| match &operation.kind {
            legalized_operations::LegalizedScalarInstructionKind::Call(call) => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(function.blocks[0].instructions.len(), 5);
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0].arguments, calls[1].arguments);
    assert_eq!(
        calls[2].arguments[0].source,
        function.blocks[0].instructions[2].result
    );
    assert_eq!(
        calls[2].arguments[1].source,
        function.blocks[0].instructions[3].result
    );

    let mut corruptions = Vec::new();
    let mut corrupted = legalized.plan().clone();
    corrupted.scalar_functions[0].blocks[0]
        .instructions
        .swap(0, 1);
    corruptions.push(corrupted);
    let mut corrupted = legalized.plan().clone();
    call_mut(&mut corrupted.scalar_functions[0], 0)
        .arguments
        .swap(0, 1);
    corruptions.push(corrupted);
    let mut corrupted = legalized.plan().clone();
    corrupted.scalar_functions[0].blocks[0].instructions[4].result =
        corrupted.scalar_functions[0].blocks[0].instructions[3].result;
    corruptions.push(corrupted);
    let mut corrupted = legalized.plan().clone();
    corrupted.scalar_functions[0].blocks[0].instructions[3].fuel[0].units += 1;
    corruptions.push(corrupted);
    let mut corrupted = legalized.plan().clone();
    corrupted.scalar_functions.clear();
    corruptions.push(corrupted);
    for corrupted in corruptions {
        assert!(validate_legalized_operations(&target, &abstract_plan, &unit, corrupted).is_err());
    }
}

fn call_mut(
    function: &mut legalized_operations::LegalizedScalarFunction,
    index: usize,
) -> &mut legalized_operations::LegalizedScalarCall {
    function.blocks[0]
        .instructions
        .iter_mut()
        .filter_map(|operation| match &mut operation.kind {
            legalized_operations::LegalizedScalarInstructionKind::Call(call) => Some(call),
            _ => None,
        })
        .nth(index)
        .expect("call fixture")
}

#[test]
fn publication_classification_uses_the_existing_unit_grammar() {
    let (abstracted, targeted, unit) = scalar_call_unit_fixture();
    assert!(crate::legalization::accepts_fragment_publication_input(
        &targeted,
        &abstracted,
        &unit
    ));
    let mut changed = abstracted.clone();
    changed.functions[0].operations.pop();
    assert!(!crate::legalization::accepts_fragment_publication_input(
        &targeted, &changed, &unit
    ));
}
fn register_arity_source(arity: usize) -> abstract_operations::AbstractOperationPlan {
    use abstract_operations::{AbstractBlockEntry, AbstractOperation, AbstractParameter};
    use semantic_vocabulary::{
        EdgeId, IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
    };
    let (mut plan, _, _) = scalar_call_unit_fixture();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    for operation in &mut plan.functions[0].operations {
        if let AbstractOperation::Call { arguments, .. } = operation {
            let original = arguments.clone();
            *arguments = (0..arity)
                .map(|index| original[index % original.len()])
                .collect();
        }
    }
    let callee = &mut plan.functions[1];
    callee.parameters = (0..arity)
        .map(|index| AbstractParameter {
            value: ValueId::new(1000 + index as u64).unwrap(),
            scalar_type,
        })
        .collect();
    callee.block_entries = vec![AbstractBlockEntry {
        block: callee.entry,
        parameters: Vec::new(),
        operation_offset: 0,
    }];
    let abstract_operations::AbstractFunctionResult::Scalar(result) = callee.result else {
        unreachable!()
    };
    let source = callee
        .parameters
        .first()
        .map_or(ValueId::new(2000).unwrap(), |parameter| parameter.value);
    callee.operations = if arity == 0 {
        vec![AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(2001).unwrap(),
            result: source,
            scalar_type,
            value: IntegerValue::Unsigned(7),
        }]
    } else {
        Vec::new()
    };
    callee.operations.push(AbstractOperation::Return {
        psi_edge: EdgeId::new(2002).unwrap(),
        result: result.value,
        value: source,
        scalar_type,
        cleanup_actions: Vec::new(),
    });
    plan
}

#[test]
fn every_register_arity_uses_one_input_contract_and_independent_replay() {
    for (native, capacity) in [
        (target::NativeTarget::linux_x64(), 6),
        (target::NativeTarget::linux_arm64(), 8),
        (target::NativeTarget::windows_x64(), 4),
        (target::NativeTarget::macos_arm64(), 8),
    ] {
        for arity in 0..=capacity {
            let source = register_arity_source(arity);
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &source, native,
            )
            .unwrap();
            let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                semantic_vocabulary::FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap();
            assert!(crate::legalization::accepts_fragment_publication_input(
                &target, &source, &unit
            ));
            let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
            for operation in &legalized.plan().scalar_functions[0].blocks[0].instructions {
                if let legalized_operations::LegalizedScalarInstructionKind::Call(call) =
                    &operation.kind
                {
                    assert_eq!(call.arguments.len(), arity);
                }
            }
            validate_legalized_operations(&target, &source, &unit, legalized.plan().clone())
                .unwrap();
            let mut omitted = legalized.plan().clone();
            let call = call_mut(&mut omitted.scalar_functions[0], 0);
            if call.arguments.pop().is_some() {
                assert!(validate_legalized_operations(&target, &source, &unit, omitted).is_err());
            }
        }
        let source = register_arity_source(capacity + 1);
        let target =
            abstract_operations_to_target_operations::lower_to_target_operations(&source, native)
                .unwrap();
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            semantic_vocabulary::FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        assert!(!crate::legalization::accepts_fragment_publication_input(
            &target, &source, &unit
        ));
        assert!(legalize_target_operations(&target, &source, &unit).is_err());
    }
}
