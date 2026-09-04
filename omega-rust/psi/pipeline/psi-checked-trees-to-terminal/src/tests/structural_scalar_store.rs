use super::*;

const SOURCE: &str = r#"
    data Pair { left: u8; right: u16; }
    data Inner { value: u8; }
    data Outer { inner: Inner; }
    data Sink {}

    machine Sink::direct(pair: &write Pair) {
        pair.left = 7;
    }

    machine Sink::nested(outer: &write Outer) {
        outer.inner.value = 9;
    }
"#;

const RESULT_SOURCE: &str = r#"
    data Scalar {}
    machine Scalar::identity(value: i32) -> i32
    requires value == value
    ensures result == value
    {
        transition { _ -> value }
    }

    data Pair { prefix: u8; target: i32; }
    data Root {}
    machine Root::enter(destination: &write Pair) {
        let replacement: i32 = Scalar::identity(23);
        destination.target = replacement;
    }
"#;

#[test]
fn lowers_direct_and_nested_write_only_record_field_stores() {
    let checked = checked_source(SOURCE);
    for (machine_name, expected_path_len, expected_value) in
        [("Sink::direct", 0_usize, 7_u128), ("Sink::nested", 1, 9)]
    {
        let lowered = lower_machine(&checked, machine_name).expect("field store lowers");
        psi_terminal_verifier::validate_module(&lowered.semantic_module)
            .expect("field store module verifies");
        let entry = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .expect("entry machine");
        assert_eq!(
            entry.structural_parameters[0].access,
            psi_terminal::StructuralAccess::WriteOnlyBorrow
        );
        assert!(matches!(
            entry.blocks[0].operations.as_slice(),
            [
                Operation {
                    kind: OperationKind::IntegerConstant { value },
                    ..
                },
                Operation {
                    kind: OperationKind::StructuralScalarFieldStore { path, .. },
                    ..
                },
            ] if path.len() == expected_path_len
                && matches!(value, IntegerValue::Unsigned(value)
                    if *value == expected_value)
        ));
    }
}

#[test]
fn rejects_checked_record_field_store_path_corruption() {
    let mut checked = checked_source(SOURCE);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Sink::nested")
        .expect("nested machine")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.machine == machine)
        .expect("nested Unit plan");
    let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) = &mut plan.operations[0]
    else {
        panic!("checked structural store")
    };
    store.carrier_path.clear();
    assert!(matches!(
        lower_machine(&checked, "Sink::nested"),
        Err(LoweringError::Unsupported(
            "structural scalar store field is absent or ambiguous"
        ))
    ));
}

#[test]
fn scalar_result_reaches_one_projected_store_and_local_drift_rejects() {
    let checked = checked_source(RESULT_SOURCE);
    let lowered = lower_machine(&checked, "Root::enter")
        .expect("scalar result reaches one projected field store");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let producer = entry.blocks[0]
        .operations
        .iter()
        .find(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .expect("ordinary scalar producer");
    let OperationResult::Scalar(result) = producer.result else {
        panic!("ordinary producer returns one scalar")
    };
    assert!(entry.blocks[0].operations.iter().any(|operation| matches!(
        operation.kind,
        OperationKind::StructuralScalarFieldStore { value, .. } if value == result.id
    )));

    let mut drifted = checked;
    let machine = drifted
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .expect("projected result-store machine")
        .symbol;
    let plan = drifted
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.machine == machine)
        .expect("projected result-store plan");
    let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) = &mut plan.operations[1]
    else {
        panic!("checked projected store")
    };
    let CheckedScalarExpression::Local { position, .. } = &mut store.value else {
        panic!("projected store reads the scalar result local")
    };
    *position = 1;
    assert!(matches!(
        lower_machine(&drifted, "Root::enter"),
        Err(LoweringError::Unsupported(
            "structural scalar store lost exact exclusive custody"
        ))
    ));
}
