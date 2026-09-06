use super::*;

const DECLARATIONS: &str = "data Value { number: u64; }
    machine forward(value: Value) -> Value { value }
    machine Main::consume(value: Value) {}
    machine Main::consume_pair(left: Value, right: Value) {}";

#[test]
fn nested_structural_call_has_a_real_temporary_result() {
    assert_nested(
        &format!(
            "{DECLARATIONS} machine Main::caller(value: Value) {{ Main::consume(forward(value)); }}"
        ),
        "Main::caller",
        &[(0, 1), (0, 0)],
        5,
    );
}

fn assert_nested(source: &str, name: &str, coordinates: &[(usize, usize)], fuel: usize) {
    let checked = checked(source);
    let lowered = lower_machine(&checked, name)
        .unwrap_or_else(|error| panic!("nested result lowers: {error:?}\n{source}"));
    assert_eq!(
        lowered
            .source_call_occurrences
            .iter()
            .map(|call| (call.statement_index, call.call_ordinal))
            .collect::<Vec<_>>(),
        coordinates
    );
    let caller = &lowered.semantic_module.machines[0];
    let operations = &caller.blocks[0].operations;
    let mut results = Vec::new();
    let mut moved = Vec::new();
    for operation in operations {
        let arguments = match &operation.kind {
            OperationKind::CallStructuralWithScalarArguments {
                structural_arguments,
                ..
            }
            | OperationKind::CallUnit {
                structural_arguments,
                ..
            } => structural_arguments,
            _ => panic!("call-only nested fixture"),
        };
        for argument in arguments {
            assert!(argument.path.is_empty());
            assert!(
                caller
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.place == argument.place)
                    || results.contains(&argument.place),
                "argument must already exist"
            );
            assert!(
                !moved.contains(&argument.place),
                "one move per input or temporary"
            );
            moved.push(argument.place);
        }
        if let OperationResult::Structural(result) = &operation.result {
            assert!(!results.contains(&result.place));
            assert!(
                !moved.contains(&result.place),
                "result cannot exist before producer returns"
            );
            results.push(result.place);
        }
    }
    assert!(
        results.iter().all(|place| moved.contains(place)),
        "every temporary/local is consumed by this fixture"
    );
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &caller.blocks[0].terminator
    else {
        panic!("whole-result exit")
    };
    assert!(trivial_affine_discards.is_empty());
    super::chains::assert_execution(&lowered.semantic_module, &lowered.proof_bundle, &[], fuel);
}

#[test]
fn deeper_calls_execute_in_postorder_without_renumbering_occurrences() {
    assert_nested(
        &format!(
            "{DECLARATIONS} machine Main::caller(value: Value) {{ Main::consume(forward(forward(value))); }}"
        ),
        "Main::caller",
        &[(0, 2), (0, 1), (0, 0)],
        7,
    );
}

#[test]
fn sibling_operands_execute_in_authored_argument_order() {
    assert_nested(
        &format!(
            "{DECLARATIONS} machine Main::caller(left: Value, right: Value) {{ Main::consume_pair(forward(left), forward(right)); }}"
        ),
        "Main::caller",
        &[(0, 1), (0, 2), (0, 0)],
        7,
    );
    assert_nested(
        &format!(
            "{DECLARATIONS} machine Main::caller(left: Value, right: Value) {{ Main::consume_pair(forward(forward(left)), forward(forward(right))); }}"
        ),
        "Main::caller",
        &[(0, 2), (0, 1), (0, 4), (0, 3), (0, 0)],
        11,
    );
}

#[test]
fn nested_results_in_unit_tails_use_the_same_expression_storage() {
    assert_nested(
        &format!(
            "{DECLARATIONS} machine Main::caller(value: Value) {{ Main::consume(forward(value)) }}"
        ),
        "Main::caller",
        &[(0, 1), (0, 0)],
        5,
    );
}

#[test]
fn later_initializers_keep_nested_call_coordinates() {
    assert_nested(
        &format!(
            "{DECLARATIONS} machine Main::tick() {{}} machine Main::caller(value: Value) {{ Main::tick(); let result: Value = forward(forward(value)); Main::consume(result); }}"
        ),
        "Main::caller",
        &[(0, 0), (1, 1), (1, 0), (2, 0)],
        9,
    );
}

#[test]
fn named_scalar_arguments_do_not_need_reordered_computations() {
    let checked = checked(
        "data Value { number: u64; }
        machine forward(value: Value) -> Value { value }
        machine Main::consume(count: u32, value: Value) {}
        machine Main::caller(count: u32, value: Value) { Main::consume(count, forward(value)); }",
    );
    let lowered = lower_machine(&checked, "Main::caller")
        .expect("existing scalar value supplies the enclosing call");
    super::chains::assert_execution(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &[TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
            value: IntegerValue::Unsigned(4),
        }],
        5,
    );
}

#[test]
fn local_and_temporary_results_share_one_binding_namespace() {
    for (body, coordinates) in [
        (
            "let prior: Value = forward(value); Main::consume(forward(prior));",
            vec![(0, 0), (1, 1), (1, 0)],
        ),
        (
            "let result: Value = forward(forward(value)); Main::consume(result);",
            vec![(0, 1), (0, 0), (1, 0)],
        ),
    ] {
        assert_nested(
            &format!("{DECLARATIONS} machine Main::caller(value: Value) {{ {body} }}"),
            "Main::caller",
            &coordinates,
            7,
        );
    }
}

#[test]
fn free_callers_retain_nested_array_and_generic_temporaries() {
    for (declarations, identity) in [
        (
            "data Inner { number: u64; } data Outer { inner: Inner; count: u32; }",
            "Outer",
        ),
        ("data Entry { number: u64; }", "[Entry; 3]"),
        (
            "data Entry { number: u64; } data Buffer<T> { entries: [T; 3]; }",
            "Buffer<Entry>",
        ),
    ] {
        let source = format!(
            "{declarations} machine forward(value: {identity}) -> {identity} {{ value }} machine Main::consume(value: {identity}) {{}} machine caller(value: {identity}) {{ Main::consume(forward(value)); }}"
        );
        assert_nested(&source, "caller", &[(0, 1), (0, 0)], 5);
    }
}

#[test]
fn nested_structural_calls_reject_temporary_and_occurrence_drift() {
    let source = format!(
        "{DECLARATIONS} machine Main::caller(left: Value, right: Value) {{ Main::consume_pair(forward(left), forward(right)); }}"
    );
    for mutation in 0..6 {
        let mut checked = checked(&source);
        let caller = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| {
                plan.operations
                    .iter()
                    .filter(|operation| {
                        matches!(
                            operation,
                            CheckedUnitEffectOperationPlan::StructuralCall { .. }
                        )
                    })
                    .count()
                    == 2
            })
            .expect("nested plan");
        match mutation {
            0 => caller.operations.swap(0, 1),
            1 => {
                let CheckedUnitEffectOperationPlan::StructuralCall {
                    discard_result_on_return,
                    ..
                } = &mut caller.operations[0]
                else {
                    unreachable!()
                };
                *discard_result_on_return = true;
            }
            2 | 3 => {
                let CheckedUnitEffectOperationPlan::StructuralCall {
                    coordinate, result, ..
                } = &mut caller.operations[0]
                else {
                    unreachable!()
                };
                if mutation == 2 {
                    coordinate.call_ordinal = 0;
                } else {
                    result.binding_ordinal = 1;
                }
            }
            4 => {
                let CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } = &mut caller.operations[2]
                else {
                    unreachable!()
                };
                structural_arguments.swap(0, 1);
            }
            5 => {
                caller.operations.remove(0);
            }
            _ => unreachable!(),
        }
        assert!(
            lower_machine(&checked, "Main::caller").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn nested_argument_evaluation_does_not_hoist_scalar_computation() {
    let source = "data Value { number: u64; }
        machine forward(value: Value) -> Value { value }
        machine Main::consume(count: u32, value: Value) {}
        machine Main::caller(count: u32, value: Value) { Main::consume(count ^ 1u32, forward(value)); }";
    let checked = checked(source);
    assert!(
        lower_machine(&checked, "Main::caller").is_err(),
        "mixed operand computations need an argument-position evaluator"
    );
}

#[test]
fn a_reference_field_cannot_enter_the_owned_temporary_initializer_route() {
    let source = "data View { reference: &u64; }
        machine forward(value: View) -> View { value }
        machine Main::caller(value: View) { let result: View = forward(forward(value)); }";
    let diagnostics = typed_trees_to_checked_trees::lower_typed_trees(typed(source))
        .expect_err("stored references need a loan-bearing temporary plan");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("a value-call argument cannot itself be a machine call yet")),
        "{diagnostics:#?}"
    );
}
