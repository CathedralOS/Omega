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
fn unconditional_transition_does_not_require_an_unreachable_fallback() {
    let checked = checked(
        "data Root { count: u32 in Wrapping; }
         machine Root::choose(&mut self) {
             self.count = self.count + 1;
             transition { _ -> selected() _ -> skipped() }
             state selected(&mut self) { self.count = self.count + 1; }
             state skipped(&mut self) { self.count = self.count + 10; }
         }",
    );
    let machine = machine_named(&checked, "choose");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .expect("the first unconditional arm terminates the ordered transition tail");
    assert!(matches!(
        plan.states[0].terminator,
        CheckedComposedUnitControlTerminatorPlan::Jump { .. }
    ));
}

#[test]
fn canonical_float_comparison_cannot_bypass_a_present_machine_body() {
    let mut typed = crate::tests::front_end::typed_program(
        "boundary machine > Float::greater(left: f64, right: f64) -> bool;
         machine compare(left: f64, right: f64) -> bool { left > right }",
    );
    let machine = typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.spelling.is_some())
        .unwrap();
    machine.body_is_present = true;
    let symbol = machine.symbol;
    let bound = crate::operators::bind_selected_operator_calls(&mut typed)
        .expect("a body-bearing comparison must use ordinary call supply");
    assert_eq!(bound, 1);
    let machine = crate::lookup::machine_by_symbol(&typed, symbol).unwrap();
    let entry = typed.machine_states(machine)[0].symbol;
    assert!(typed.expression_table.iter_expressions().any(
        |(_, expression)| matches!(expression, typed_trees::expression::ExpressionNode::Call(call)
            if call.target_symbol == entry)
    ));
}

#[test]
fn selected_float_field_guards_retain_declared_and_literal_operands() {
    for declaration in ["boundary operator >", "boundary machine >"] {
        for operand in ["other", "211.5", "211.5f64"] {
            let source = format!(
                r#"
                {declaration} Float::greater(left: f64, right: f64) -> bool;
                data Main {{ result: f64; }}
                data Helper {{}}
                machine Helper::quiet(code: i32) {{}}
                machine Main::choose(&mut self, other: f64) {{
                    self.result = other;
                    transition self.result > {operand} {{
                        true -> above()
                        _ -> below()
                    }}
                    state above(&mut self) {{ Helper::quiet(70); }}
                    state below(&mut self) {{ Helper::quiet(71); }}
                }}
                "#,
            );
            let checked = checked(&source);
            let machine = machine_named(&checked, "choose");
            assert!(
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .composed_for_machine(machine)
                    .is_some(),
                "{declaration}, {operand}: omissions={:?}",
                checked.facts.flow.terminal_unit_effects.omissions,
            );
        }
    }
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

const NESTED_READS: &str = r#"
    data Grid { a: [[i32 in Wrapping; 3]; 3]; out: i32 in Wrapping; }
    machine Grid::walk(&mut self) {
        transition { _ -> dot(1, 0, 0) }
        state dot(&mut self, i: u64, k: u64, acc: i32 in Wrapping)
        requires i <= 2
        requires k <= 2 {
            let next_k: u64 = k + 1;
            transition next_k <= 2 {
                true -> dot(i, next_k, acc + self.a[i][k])
                false -> done(acc + self.a[i][k])
            }
        }
        state done(&mut self, acc: i32 in Wrapping) { self.out = acc; }
    }
"#;

/// `self.a[i][k]` is one read with two runtime selectors in application
/// order, and the Wrapping policy on the element survives into the
/// successor argument's arithmetic.
#[test]
fn nested_runtime_selectors_read_in_order_under_the_element_policy() {
    use checked_trees::{CheckedIntegerBinaryKind, CheckedScalarExpression, CheckedStorageRoot};
    let checked = checked(NESTED_READS);
    let machine = machine_named(&checked, "walk");
    let graph = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .expect("nested successor reads keep the state graph");
    let dot = graph.states[1].state;
    let arguments = checked
        .facts
        .values
        .scalar_expressions
        .expressions
        .iter()
        .filter(|expression| {
            // Ordinals count the receiver: `dot(i, next_k, acc + ..)` passes
            // the accumulator at 3, `done(acc + ..)` at 1.
            expression.state == dot
                && matches!(
                    expression.role,
                    CheckedScalarExpressionRole::TransitionArgument {
                        argument_ordinal: 1 | 3,
                    }
                )
                && matches!(
                    expression.expression,
                    CheckedScalarExpression::IntegerBinary { .. }
                )
        })
        .map(|expression| &expression.expression)
        .collect::<Vec<_>>();
    assert_eq!(arguments.len(), 2, "one accumulator argument per arm");
    for argument in arguments {
        let CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::WrappingAdd,
            right,
            ..
        } = argument
        else {
            panic!("the element policy selects wrapping addition: {argument:?}");
        };
        let CheckedScalarExpression::StructuralParameterIndexedRead {
            root: CheckedStorageRoot::Parameter { index: 0 },
            path,
            index,
            inner_indexes,
            element_path,
            primitive_type: typed_trees::types::PrimitiveType::I32,
        } = right.as_ref()
        else {
            panic!("a nested read of the self field: {right:?}");
        };
        assert_eq!(
            path,
            &[checked_trees::CheckedStructuralPredicatePathSegment::Field(
                "a".to_owned()
            )]
        );
        assert!(element_path.is_empty());
        assert!(matches!(
            index.as_ref(),
            CheckedScalarExpression::Parameter { position: 0, .. }
        ));
        assert!(matches!(
            inner_indexes.as_slice(),
            [CheckedScalarExpression::Parameter { position: 1, .. }]
        ));
    }
}

/// A member between the two runtime selectors (`self.rows[i].data[k]`) is a
/// shape checking admits but the nested read does not spell: its inner
/// array is reached through a field of the selected element, not directly.
/// It keeps declining, while the directly nested array read is admitted.
#[test]
fn a_member_between_runtime_selectors_keeps_declining() {
    for (read, admitted) in [("self.a[i][k]", true), ("self.rows[i].data[k]", false)] {
        let source = format!(
            r#"
            data Row {{ data: [i32 in Wrapping; 2]; }}
            data Grid {{
                a: [[i32 in Wrapping; 2]; 2];
                rows: [Row; 2];
                out: i32 in Wrapping;
            }}
            machine Grid::walk(&mut self) {{
                transition {{ _ -> pick(1, 0) }}
                state pick(&mut self, i: u64, k: u64) requires i <= 1 requires k <= 1 {{
                    transition {{ _ -> done({read}) }}
                }}
                state done(&mut self, value: i32 in Wrapping) {{ self.out = value; }}
            }}
            "#
        );
        let checked = checked(&source);
        let machine = machine_named(&checked, "walk");
        assert_eq!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .composed_for_machine(machine)
                .is_some(),
            admitted,
            "{read}"
        );
    }
}
