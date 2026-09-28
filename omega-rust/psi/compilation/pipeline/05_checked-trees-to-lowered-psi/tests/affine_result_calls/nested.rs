use super::{
    CheckedUnitEffectOperationPlan, OperationKind, OperationResult, Terminator, checked,
    lower_machine,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
const DECLARATIONS: &str = "data Value { number: u64; }
    machine forward(value: Value) -> Value { value }
    machine Main::consume(value: Value) {}
    machine Main::consume_pair(left: Value, right: Value) {}";

fn assert_nested(source: &str, name: &str, coordinates: &[(usize, usize)], fuel: usize) {
    let checked = checked(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name(name))
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
            lower_machine(&checked, TerminalMachineSelection::Name("Main::caller")).is_err(),
            "mutation {mutation}"
        );
    }
}
