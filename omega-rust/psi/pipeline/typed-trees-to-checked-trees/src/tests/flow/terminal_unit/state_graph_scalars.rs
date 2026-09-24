//! Scalar prefixes and successor expressions retain their authored fact coordinates.
use super::{
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan,
    PrimitiveType,
};
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;
use checked_trees::{
    CheckedComposedUnitControlTerminatorPlan, CheckedScalarBindingDestination,
    CheckedScalarBindingValue, CheckedStructuralScalarArgumentSourcePlan,
};

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
fn console_authored_mutable_byte_prefix_retains_its_checked_cast_call() {
    let console = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../source/library/std/console.omg"
    ));
    let body = console
        .split("state emit_newline(byte: u8) {")
        .nth(1)
        .expect("Console newline state")
        .split('}')
        .next()
        .expect("Console newline body");
    assert!(body.contains("let mut output: u8 = byte;"));
    let checked = checked(&format!(
        r#"
        data ConsoleNativeProvider {{}}
        machine ConsoleNativeProvider::write_byte(byte: i32) {{}}
        machine writer(byte: u8) {{
            transition {{ _ -> emit_newline(byte) }}
            state emit_newline(byte: u8) {{ {body} }}
        }}
    "#
    ));
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "writer"))
        .expect("authored Console prefix and call retained");
    let emit = &plan.states[1];
    assert!(matches!(emit.bindings.as_slice(), [binding]
        if binding.primitive_type == PrimitiveType::U8
            && matches!(binding.destination, CheckedScalarBindingDestination::StorageInitialize { .. })));
    assert!(matches!(emit.operations.as_slice(),
        [CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. }]
            if coordinate.statement_index == 1 && coordinate.call_ordinal == 0));
    assert!(
        matches!(checked.facts.values.scalar_expressions.expression_at(
        emit.state, 1, CheckedScalarExpressionRole::UnitCallArgument { call_ordinal: 0, argument_ordinal: 0 }),
        Some(CheckedScalarExpression::IntegerWiden { primitive_type: PrimitiveType::I32, operand })
            if matches!(operand.as_ref(), CheckedScalarExpression::StorageRead { primitive_type: PrimitiveType::U8, .. }))
    );
}

#[test]
fn general_state_graph_retains_mutable_scalar_prefix_and_ordered_calls() {
    let checked = checked(PREFIX);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "writer"))
        .expect("scalar prefixes retain the callable graph");
    let [entry, next] = plan.states.as_slice() else {
        panic!("both authored states survive");
    };
    let [previous, initialize, assign, ready] = entry.bindings.as_slice() else {
        panic!("all four prefix statements survive");
    };
    assert_eq!(
        previous.destination,
        CheckedScalarBindingDestination::Immutable
    );
    assert_eq!(
        ready.destination,
        CheckedScalarBindingDestination::Immutable
    );
    let CheckedScalarBindingDestination::StorageInitialize { symbol } = initialize.destination
    else {
        panic!("Console's mutable byte declaration retains storage identity");
    };
    assert!(symbol.is_valid());
    assert_eq!(
        assign.destination,
        CheckedScalarBindingDestination::StorageAssign { symbol }
    );
    for (ordinal, binding) in entry.bindings.iter().enumerate() {
        assert_eq!(binding.statement_ordinal, ordinal as u32);
        assert_eq!(binding.value, CheckedScalarBindingValue::Expression);
    }
    assert_eq!(entry.binding_initializers.len(), 4);
    for (binding, role) in entry.bindings.iter().zip([
        CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
        CheckedScalarExpressionRole::StorageInitializer,
        CheckedScalarExpressionRole::AssignmentValue,
        CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 1 },
    ]) {
        let (_, expression) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(entry.state, binding.statement_ordinal, role)
            .expect("exact initializer fact and source custody");
        assert_eq!(
            &entry.binding_initializers[binding.statement_ordinal as usize],
            expression
        );
    }
    assert!(matches!(
        &entry.binding_initializers[2],
        CheckedScalarExpression::Local {
            position: 2,
            primitive_type: PrimitiveType::U8
        }
    ));
    assert_eq!(entry.operations.len(), 2);
    for (ordinal, operation) in entry.operations.iter().enumerate() {
        assert!(matches!(operation,
            CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. }
                if coordinate.statement_index == (ordinal + 4) as u32 && coordinate.call_ordinal == 0));
    }
    assert!(matches!(next.bindings.as_slice(), [binding]
        if binding.statement_ordinal == 0
            && matches!(binding.destination, CheckedScalarBindingDestination::StorageInitialize { .. })));
    assert!(matches!(next.operations.as_slice(),
        [CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. }]
            if coordinate.statement_index == 1 && coordinate.call_ordinal == 0));
}

#[test]
fn general_scalar_successors_rejoin_locals_literals_and_arithmetic() {
    let checked = checked(PREFIX);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "writer"))
        .expect("checked expression successors retain the graph");
    let entry = &plan.states[0];
    let CheckedComposedUnitControlTerminatorPlan::Conditional {
        when_true,
        when_false,
        ..
    } = &entry.terminator
    else {
        panic!("authored conditional survives");
    };
    assert_eq!(when_true.statement_ordinal, 6);
    assert_eq!(when_false.statement_ordinal, 7);
    for (edge, sources) in [
        (
            when_true,
            [CheckedStructuralScalarArgumentSourcePlan::Expression; 2],
        ),
        (
            when_false,
            [
                CheckedStructuralScalarArgumentSourcePlan::Expression,
                CheckedStructuralScalarArgumentSourcePlan::Parameter { index: 0 },
            ],
        ),
    ] {
        for (ordinal, (argument, source)) in edge.scalar_arguments.iter().zip(sources).enumerate() {
            assert_eq!(argument.source, source);
            assert_eq!(argument.argument_ordinal, ordinal as u32);
            assert_eq!(argument.target_scalar_parameter_index, ordinal as u32);
            assert_eq!(argument.primitive_type, PrimitiveType::U8);
            assert!(
                checked
                    .facts
                    .values
                    .scalar_expressions
                    .bound_expression_at(
                        entry.state,
                        edge.statement_ordinal,
                        CheckedScalarExpressionRole::TransitionArgument {
                            argument_ordinal: argument.argument_ordinal
                        },
                    )
                    .is_some()
            );
        }
    }
    assert!(matches!(
        checked.facts.values.scalar_expressions.expression_at(
            entry.state,
            6,
            CheckedScalarExpressionRole::TransitionArgument {
                argument_ordinal: 0
            }
        ),
        Some(CheckedScalarExpression::StorageRead { .. })
    ));
    assert!(matches!(
        checked.facts.values.scalar_expressions.expression_at(
            entry.state,
            6,
            CheckedScalarExpressionRole::TransitionArgument {
                argument_ordinal: 1
            }
        ),
        Some(CheckedScalarExpression::IntegerBinary { .. })
    ));
    assert!(matches!(
        checked.facts.values.scalar_expressions.expression_at(
            entry.state,
            7,
            CheckedScalarExpressionRole::TransitionArgument {
                argument_ordinal: 0
            }
        ),
        Some(CheckedScalarExpression::IntegerLiteral { .. })
    ));
}

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
                &[],
                &[],
            );
            assert!(
                plans.composed_for_machine(machine).is_none(),
                "{ordinal} {role:?} custody={corrupt_custody}"
            );
        }
    }
}

#[test]
fn general_state_graph_retains_interleaved_immutable_scalar_local() {
    let checked = checked(&PREFIX.replace(
        "Helper::quiet(output);\n        Helper::quiet(previous);",
        "Helper::quiet(output); let later: u8 = byte; Helper::quiet(previous);",
    ));
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "writer"))
        .expect("immutable local composes after call");
    assert!(matches!(plan.states[0].operations.as_slice(), [
        CheckedUnitEffectOperationPlan::CallUnit { coordinate: first, .. },
        CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, .. },
        CheckedUnitEffectOperationPlan::CallUnit { coordinate: last, .. },
    ] if first.statement_index == 4 && result.statement_index == 5
        && result.binding_ordinal == 2 && last.statement_index == 6));
}

#[test]
fn general_state_graph_retains_interleaved_scalar_storage_write() {
    // A non-observing store into mutable scalar local storage between two
    // calls is ordinary statement sequencing: since 6abba05945 the composed
    // state graph retains the sequencer's WriteOnlyPrimitiveStore at its
    // authored coordinate instead of declining the machine.
    let checked = checked(&PREFIX.replace(
        "Helper::quiet(output);\n        Helper::quiet(previous);",
        "Helper::quiet(output); output = byte; Helper::quiet(previous);",
    ));
    let machine = machine_named(&checked, "writer");
    let declaration = checked
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == machine)
        .unwrap();
    let typed_trees::statement::StatementNode::LocalData(output) = &checked
        .statement_table
        .statements(checked.machine_states(declaration)[0].statement_nodes)[1]
    else {
        panic!("`output` is the second local")
    };
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .expect("mutable scalar local store composes between calls");
    assert!(matches!(plan.states[0].operations.as_slice(), [
        CheckedUnitEffectOperationPlan::CallUnit { coordinate: first, .. },
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index: 5,
            destination: checked_trees::CheckedPrimitiveStoreDestination::Local { symbol },
            path,
            ..
        },
        CheckedUnitEffectOperationPlan::CallUnit { coordinate: last, .. },
    ] if first.statement_index == 4 && *symbol == output.symbol && path.is_empty()
        && last.statement_index == 6));
}

#[test]
fn state_graph_composes_a_field_equality_guard_inside_a_named_state() {
    // `self.<field> == <literal>` inside a declared `state` block: the
    // receiver formal is authored on that state, and the pure Boolean
    // equality lane mints the conditional pair.
    let checked = checked(
        r#"
        pub data Root { flag: bool; hit: i32; }
        machine Root::run(&mut self) {
            state open(&mut self) {
                transition self.flag == true {
                    true -> have()
                    _ -> stop()
                }
            }
            state have(&mut self) { self.hit = 70; }
            state stop(&mut self) { self.hit = 71; }
        }
        "#,
    );
    let machine = machine_named(&checked, "run");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "the named-state equality guard composes: {:?}",
                checked.facts.flow.terminal_unit_effects.omissions
            )
        });
    let [entry, open, have, stop] = plan.states.as_slice() else {
        panic!(
            "the named-state equality pair keeps the entry, its state, and both leaves: {:?}",
            plan.states.len()
        )
    };
    let CheckedComposedUnitControlTerminatorPlan::Conditional {
        guard,
        when_true,
        when_false,
        ..
    } = &open.terminator
    else {
        panic!(
            "the named-state equality guard mints a conditional terminator: {:?}",
            open.terminator
        )
    };
    assert_eq!(when_true.target_state, have.state);
    assert_eq!(when_false.target_state, stop.state);
    let CheckedScalarExpression::Boolean(boolean) = guard else {
        panic!("the equality guard is a Boolean scalar: {guard:?}")
    };
    let checked_trees::CheckedBooleanExpression::Equal { left, right } = boolean.as_ref() else {
        panic!("the equality guard keeps the authored Equal: {boolean:?}")
    };
    assert!(matches!(
        left.as_ref(),
        checked_trees::CheckedBooleanExpression::StructuralParameterField {
            parameter_position: 0,
            path,
        } if path.as_slice() == [
            checked_trees::CheckedStructuralPredicatePathSegment::Field("flag".to_owned())
        ]
    ));
    assert!(matches!(
        right.as_ref(),
        checked_trees::CheckedBooleanExpression::Constant(true)
    ));
    let _ = entry;
}

#[test]
fn state_graph_composes_a_constant_indexed_member_guard_inside_a_named_state() {
    // `self.<array-field>[<literal>] <cmp> <literal>` inside a declared `state`
    // block: the constant-indexed member read stays a live checked dependency
    // and the comparison lane mints the conditional pair.
    let checked = checked(
        r#"
        pub data Root { control: [u8; 64]; hit: i32; }
        machine Root::run(&mut self) {
            state open(&mut self) {
                transition self.control[3] >= 48 {
                    true -> have()
                    _ -> stop()
                }
            }
            state have(&mut self) { self.hit = 70; }
            state stop(&mut self) { self.hit = 71; }
        }
        "#,
    );
    let machine = machine_named(&checked, "run");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "the named-state indexed member guard composes: {:?}",
                checked.facts.flow.terminal_unit_effects.omissions
            )
        });
    let [entry, open, have, stop] = plan.states.as_slice() else {
        panic!(
            "the named-state indexed guard pair keeps the entry, its state, and both leaves: {:?}",
            plan.states.len()
        )
    };
    let CheckedComposedUnitControlTerminatorPlan::Conditional {
        guard,
        when_true,
        when_false,
        ..
    } = &open.terminator
    else {
        panic!(
            "the indexed member guard mints a conditional terminator: {:?}",
            open.terminator
        )
    };
    assert_eq!(when_true.target_state, have.state);
    assert_eq!(when_false.target_state, stop.state);
    let CheckedScalarExpression::Boolean(boolean) = guard else {
        panic!("the indexed guard is a Boolean scalar: {guard:?}")
    };
    let checked_trees::CheckedBooleanExpression::IntegerComparison { kind, left, right } =
        boolean.as_ref()
    else {
        panic!("the indexed guard keeps the authored comparison: {boolean:?}")
    };
    assert_eq!(
        *kind,
        checked_trees::CheckedIntegerComparisonKind::LessOrEqual
    );
    assert!(matches!(
        left.as_ref(),
        CheckedScalarExpression::IntegerLiteral { literal }
            if literal.value_u64() == Some(48)
    ));
    assert!(matches!(
        right.as_ref(),
        CheckedScalarExpression::StructuralParameterField {
            parameter_position: 0,
            path,
            primitive_type: PrimitiveType::U8,
        } if path.as_slice() == [
            checked_trees::CheckedStructuralPredicatePathSegment::Field("control".to_owned()),
            checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(3),
        ]
    ));
    let _ = entry;
}
