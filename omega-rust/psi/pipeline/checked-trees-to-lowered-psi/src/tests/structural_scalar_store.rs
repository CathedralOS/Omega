use super::*;

#[test]
fn ieee_field_store_literals_require_their_exact_format() {
    for (value, primitive_type, wrong_type) in [
        (
            semantic_vocabulary::IeeeFloatValue::Binary32(0x8000_0000),
            PrimitiveType::F32,
            PrimitiveType::F64,
        ),
        (
            semantic_vocabulary::IeeeFloatValue::Binary64(0x7ff8_0000_0000_0042),
            PrimitiveType::F64,
            PrimitiveType::F32,
        ),
    ] {
        let expression = CheckedScalarExpression::IeeeFloatLiteral { value };
        assert!(
            crate::structural_scalar_store::checked_store_literal_matches(
                &expression,
                primitive_type
            )
        );
        for rejected in [wrong_type, PrimitiveType::U64, PrimitiveType::Bool] {
            assert!(
                !crate::structural_scalar_store::checked_store_literal_matches(
                    &expression,
                    rejected
                )
            );
        }
    }
}

#[test]
fn ieee_field_stores_retain_exact_parameters_and_literal_bits() {
    for primitive in ["f32", "f64"] {
        for access in ["write", "mut"] {
            for replacement in ["value", "1.25"] {
                let checked = checked_source(&format!(
                    "data Record [copy] {{ value: {primitive}; }}
                     machine Record::replace(&{access} self, value: {primitive}) {{
                         self.value = {replacement};
                     }}"
                ));
                let artifact = produce_terminal_artifact(&checked, "Record::replace")
                    .expect("IEEE field store publishes canonical Terminal");
                let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
                let entry = module
                    .machines
                    .iter()
                    .find(|machine| machine.id == module.entry)
                    .unwrap();
                let operations = &entry.blocks[0].operations;
                let OperationKind::StructuralScalarFieldStore { value, path, .. } =
                    &operations.last().unwrap().kind
                else {
                    panic!("last operation is a non-observing field store")
                };
                assert!(path.is_empty());
                if replacement == "value" {
                    assert_eq!(
                        operations.len(),
                        1,
                        "parameter forwarding introduces no load"
                    );
                    assert_eq!(*value, entry.parameters[0].id);
                } else {
                    assert_eq!(operations.len(), 2, "literal plus store introduces no load");
                    let OperationKind::IeeeFloatConstant { value: literal } = operations[0].kind
                    else {
                        panic!("IEEE literal retains raw bits")
                    };
                    let expected = if primitive == "f32" {
                        semantic_vocabulary::IeeeFloatValue::Binary32(1.25_f32.to_bits())
                    } else {
                        semantic_vocabulary::IeeeFloatValue::Binary64(1.25_f64.to_bits())
                    };
                    assert_eq!(literal, expected);
                }
            }
        }
    }
}

#[test]
fn ieee_field_store_receiving_rejects_type_source_access_and_field_drift() {
    for primitive in ["f32", "f64"] {
        let checked = checked_source(&format!(
            "data Record [copy] {{ value: {primitive}; other: {primitive}; }}
             machine Record::replace(&write self, value: {primitive}, other: {primitive}) {{
                 self.value = value;
             }}"
        ));
        lower_machine(&checked, "Record::replace").expect("untampered IEEE field store lowers");
        for corruption in 0..4 {
            let mut changed = checked.clone();
            let plan = changed
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter_mut()
                .find(|plan| {
                    plan.operations.iter().any(|operation| {
                        matches!(
                            operation,
                            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                        )
                    })
                })
                .expect("IEEE field store plan");
            if corruption == 2 {
                plan.structural_parameters[0].access =
                    checked_trees::CheckedStructuralAccess::SharedBorrow;
            } else {
                let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) =
                    &mut plan.operations[0]
                else {
                    panic!("IEEE field store")
                };
                match corruption {
                    0 => {
                        store.primitive_type = if primitive == "f32" {
                            PrimitiveType::F64
                        } else {
                            PrimitiveType::F32
                        }
                    }
                    1 => {
                        let CheckedScalarExpression::Parameter { position, .. } = &mut store.value
                        else {
                            panic!("runtime IEEE parameter")
                        };
                        *position = 1;
                    }
                    3 => store.field_identity = "Record::other".into(),
                    _ => unreachable!(),
                }
            }
            assert!(
                lower_machine(&changed, "Record::replace").is_err(),
                "IEEE store corruption {corruption} must reject for {primitive}"
            );
        }
    }
}

const SOURCE: &str = r#"
    data Pair { left: u8; right: u16; }
    data Inner { value: u8; }
    data Outer { inner: Inner; }
    data Cell [copy] { prefix: u8; value: u16; }
    data Matrix { prefix: u8; cells: [Cell; 3]; }
    data Sink {}

    machine Sink::direct(pair: &write Pair) {
        pair.left = 7;
    }

    machine Sink::nested(outer: &write Outer) {
        outer.inner.value = 9;
    }

    machine Sink::indexed(matrix: &write Matrix) {
        matrix.cells[2].value = 13;
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
fn source_indexed_shared_call_reaches_serialized_interpretation() {
    let checked = checked_source(
        r#"
        data Cell [copy] { value: u16; }
        data Matrix [copy] { cells: [Cell; 3]; }
        data Sink {}
        machine Sink::inspect(cell: &Cell) {}
        data Root {}
        machine Root::forward(matrix: &Matrix) {
            Sink::inspect(&matrix.cells[2]);
        }
    "#,
    );
    let artifact = produce_terminal_artifact(&checked, "Root::forward")
        .expect("source indexed shared call produces canonical Terminal");
    drop(checked);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &entry.blocks[0].operations[0].kind
    else {
        panic!("source forwarding retains a Unit call")
    };
    assert!(matches!(structural_arguments.as_slice(), [argument]
        if argument.access == StructuralAccess::SharedBorrow
            && matches!(argument.path.as_slice(), [StructuralPathSegment::Field(_), StructuralPathSegment::FixedIndex(2)])));
    let argument = terminal_interpreter::TerminalStructuralValue {
        opaque_identity: 1,
        structural_type: entry.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution =
        terminal_interpreter::TerminalExecution::start_artifact_with_structural_arguments(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
            &[argument],
        )
        .expect("source indexed shared call reconstructs for interpretation");
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(3);
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        )
    );
}

#[test]
fn lowers_direct_and_nested_write_only_record_field_stores() {
    let checked = checked_source(SOURCE);
    for (machine_name, expected_path_len, expected_value) in [
        ("Sink::direct", 0_usize, 7_u128),
        ("Sink::nested", 1, 9),
        ("Sink::indexed", 2, 13),
    ] {
        let lowered = lower_machine(&checked, machine_name).expect("field store lowers");
        terminal_verifier::validate_module(&lowered.semantic_module)
            .expect("field store module verifies");
        let entry = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .expect("entry machine");
        assert_eq!(
            entry.structural_parameters[0].access,
            terminal_psi::StructuralAccess::WriteOnlyBorrow
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
            "structural scalar store destination drifted from its authored place"
        ))
    ));
}

#[test]
fn rejects_checked_literal_indexed_store_bound_corruption() {
    let mut checked = checked_source(SOURCE);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Sink::indexed")
        .expect("literal-indexed machine")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.machine == machine)
        .expect("literal-indexed Unit plan");
    let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) = &mut plan.operations[0]
    else {
        panic!("checked literal-indexed structural store")
    };
    let CheckedUnitStructuralPathSegment::FixedIndex(index) = &mut store.carrier_path[1] else {
        panic!("checked literal index")
    };
    *index = 3;
    assert!(matches!(
        lower_machine(&checked, "Sink::indexed"),
        Err(LoweringError::Unsupported(
            "structural scalar store destination drifted from its authored place"
        ))
    ));
}

#[test]
fn rejects_checked_indexed_store_without_its_record_owner() {
    let checked = checked_source(SOURCE);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Sink::indexed")
        .unwrap();
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == machine.symbol)
        .unwrap();
    let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) = &plan.operations[0]
    else {
        panic!("indexed store plan")
    };
    let mut changed = store.clone();
    changed.carrier_path.remove(0);
    let lowered = lower_machine(&checked, "Sink::indexed").unwrap();
    let module = &lowered.semantic_module;
    let mut parameter = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap()
        .structural_parameters[0]
        .clone();
    parameter.structural_type = module
        .structural_types
        .iter()
        .find(|declaration| matches!(declaration.shape, StructuralTypeShape::FixedArray { .. }))
        .unwrap()
        .id;
    assert!(matches!(
        crate::structural_scalar_store::lower_structural_scalar_store_destination(
            &changed,
            changed.statement_index,
            &parameter,
            &module.structural_types,
            &[],
            &[],
            crate::structural_scalar_store::StoreAccessPolicy::Exclusive,
        ),
        Err(LoweringError::Unsupported(
            "structural scalar store carrier path is unsupported"
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
            "structural scalar store RHS drifted from its selected expression"
        ))
    ));
}
