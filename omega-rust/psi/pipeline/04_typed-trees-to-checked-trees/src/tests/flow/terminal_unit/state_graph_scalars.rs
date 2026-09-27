//! Scalar prefixes and successor expressions retain their authored fact coordinates.
use super::CheckedScalarExpressionRole;
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;
use checked_trees::CheckedComposedUnitControlTerminatorPlan;

const PREFIX: &str = r#"
    data Helper {}
    machine Helper::quiet(byte: u8) {}
    machine writer(byte: u8, flag: bool) {
        let previous: u8 = byte;
        let mut output: u8 = byte;
        output = previous;
        let ready: bool = flag;
        Helper::quiet(output);
        Helper::quiet(previous);
        transition ready {
            true -> next(output, previous + 0u8)
            false -> next(10u8, byte)
        }
        state next(byte: u8, other: u8) {
            let mut output: u8 = byte;
            Helper::quiet(output);
        }
    }
"#;

#[test]
fn general_scalar_prefix_and_successors_require_exact_facts_and_custody() {
    let checked = checked(PREFIX);
    let machine = machine_named(&checked, "writer");
    let state = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .expect("original graph")
        .states[0]
        .state;
    for (ordinal, role) in [
        (1, CheckedScalarExpressionRole::StorageInitializer),
        (2, CheckedScalarExpressionRole::AssignmentValue),
        (
            3,
            CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 1 },
        ),
        (
            6,
            CheckedScalarExpressionRole::TransitionArgument {
                argument_ordinal: 0,
            },
        ),
        (
            6,
            CheckedScalarExpressionRole::TransitionArgument {
                argument_ordinal: 1,
            },
        ),
        (
            7,
            CheckedScalarExpressionRole::TransitionArgument {
                argument_ordinal: 0,
            },
        ),
    ] {
        for corrupt_custody in [false, true] {
            let mut facts = checked.facts.clone();
            if corrupt_custody {
                let handle = facts
                    .values
                    .scalar_expressions
                    .source_bindings
                    .iter()
                    .find(|(_, binding)| {
                        binding.state == state
                            && binding.statement_ordinal == ordinal
                            && binding.role == role
                    })
                    .expect("source custody row")
                    .0;
                facts
                    .values
                    .scalar_expressions
                    .source_bindings
                    .get_mut(handle)
                    .destination = symbols::SymbolHandle::invalid();
            } else {
                facts
                    .values
                    .scalar_expressions
                    .expressions
                    .retain(|expression| {
                        expression.state != state
                            || expression.statement_ordinal != ordinal
                            || expression.role != role
                    });
            }
            let plans = crate::execution::terminal_unit::build_checked_unit_effect_plans(
                &checked.typed,
                &facts,
                crate::execution::terminal_unit::ScalarCalleePlans {
                    boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
                    structural_returns: &facts.flow.terminal_structural_scalar_returns,
                },
            );
            assert!(
                plans.composed_for_machine(machine).is_none(),
                "{ordinal} {role:?} custody={corrupt_custody}"
            );
        }
    }
}

#[test]
fn state_graph_guard_names_the_selected_comparison_computation_root() {
    // A selected IEEE comparison has no pure Boolean form: its meaning is
    // the exact selected requirement, retained as a scalar computation. The
    // conditional pair names that Guard coordinate's unique root rather than
    // omitting the whole machine.
    let checked = checked(
        r#"
        boundary operator < Float::less(left: f64, right: f64) -> bool;
        data Helper {}
        machine Helper::quiet(code: i32) {}
        machine choose(left: f64, right: f64) {
            transition left < right {
                true -> below()
                _ -> above()
            }
            state below() { Helper::quiet(21); }
            state above() { Helper::quiet(22); }
        }
        "#,
    );
    let machine = machine_named(&checked, "choose");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "the selected float guard composes: {:?}",
                checked.facts.flow.terminal_unit_effects.omissions
            )
        });
    let entry = &plan.states[0];
    let CheckedComposedUnitControlTerminatorPlan::Conditional {
        guard: checked_trees::CheckedCallScalarArgument::Computation(guard),
        when_true,
        ..
    } = &entry.terminator
    else {
        panic!(
            "the selected float guard is a computed conditional: {:?}",
            entry.terminator
        )
    };
    let computations = &checked.facts.values.scalar_computations;
    let root = computations
        .root_at(
            entry.state,
            when_true.statement_ordinal,
            CheckedScalarExpressionRole::Guard,
        )
        .expect("the guard coordinate owns one computation root");
    assert_eq!(root.root, *guard);
    assert!(
        checked
            .facts
            .values
            .scalar_expressions
            .expression_at(
                entry.state,
                when_true.statement_ordinal,
                CheckedScalarExpressionRole::Guard
            )
            .is_none(),
        "a selected comparison has no competing pure guard"
    );
    // The authored `transition left < right { true -> .. }` subject is the
    // Boolean equality of the selected comparison with `true`.
    let mut pending = vec![*guard];
    let mut selected = 0;
    while let Some(node) = pending.pop() {
        match &computations.nodes.get(node).kind {
            checked_trees::CheckedScalarComputationKind::SelectedComparison { .. } => {
                selected += 1;
            }
            checked_trees::CheckedScalarComputationKind::Apply { operands, .. } => {
                pending.extend(
                    computations
                        .operands
                        .span_or_empty(*operands)
                        .iter()
                        .copied(),
                );
            }
            _ => {}
        }
    }
    assert_eq!(selected, 1, "one selected comparison feeds the guard");
}

#[test]
fn conditional_guard_retains_a_call_computation_with_byte_view_actuals() {
    // A call inside a conditional guard lives as a scalar computation, so its
    // `&[u8]` actuals need their own structural planning: the literal mints a
    // `ByteSequenceLiteral` source and the view parameter a `Parameter` source.
    let checked = checked(
        r#"
        machine ascii_eq(text: &[u8], spelled: &[u8]) -> bool {
            transition text.len == spelled.len {
                true -> (spelled.len > 0)
                false -> (false)
            }
        }
        machine pick(text: &[u8]) {
            transition ascii_eq(text, ("auto")) {
                true -> yes()
                false -> no()
            }
            state yes() {}
            state no() {}
        }
        "#,
    );
    let machine = machine_named(&checked, "pick");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "the call guard composes: {:?}",
                checked.facts.flow.terminal_unit_effects.omissions
            )
        });
    let entry = &plan.states[0];
    let CheckedComposedUnitControlTerminatorPlan::Conditional {
        guard: checked_trees::CheckedCallScalarArgument::Computation(guard),
        when_true,
        ..
    } = &entry.terminator
    else {
        panic!(
            "the call guard is a computed conditional: {:?}",
            entry.terminator
        )
    };
    let computations = &checked.facts.values.scalar_computations;
    let root = computations
        .root_at(
            entry.state,
            when_true.statement_ordinal,
            CheckedScalarExpressionRole::Guard,
        )
        .expect("the call guard owns one computation root");
    assert_eq!(root.root, *guard);
    let mut pending = vec![*guard];
    let mut calls = 0;
    while let Some(node) = pending.pop() {
        match &computations.nodes.get(node).kind {
            checked_trees::CheckedScalarComputationKind::Call {
                structural_arguments,
                arguments,
                ..
            } => {
                calls += 1;
                pending.extend(
                    computations
                        .operands
                        .span_or_empty(*arguments)
                        .iter()
                        .copied(),
                );
                let arguments = computations
                    .structural_arguments
                    .span_or_empty(*structural_arguments);
                assert_eq!(arguments.len(), 2);
                let sources = arguments
                    .iter()
                    .map(|argument| match argument {
                        checked_trees::CheckedScalarComputationStructuralArgument::Place(
                            argument,
                        ) => &argument.source,
                        _ => panic!("the byte-view actuals are place arguments"),
                    })
                    .collect::<Vec<_>>();
                assert!(matches!(
                    sources[0],
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
                ));
                assert!(matches!(
                    sources[1],
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral {
                        bytes
                    } if bytes.as_slice() == b"auto"
                ));
            }
            checked_trees::CheckedScalarComputationKind::Apply { operands, .. } => {
                pending.extend(
                    computations
                        .operands
                        .span_or_empty(*operands)
                        .iter()
                        .copied(),
                );
            }
            _ => {}
        }
    }
    assert_eq!(calls, 1, "one call computation feeds the guard");
}
