use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;
use typed_trees::statement::StatementNode;

fn literal_move_program(body: &str, scalar: &str) -> typed_trees::TypedTrees {
    let unknown_machine = if body.contains("write_unknown") {
        "machine write_unknown(mut outer: Outer) { outer.inner.absent = 1; }"
    } else {
        ""
    };
    let source = format!(
        r#"
        data View {{ body: &mut {scalar}; tag: u64; }}
        data OtherView {{ body: &mut {scalar}; tag: u64; }}
        data Outer {{ inner: View; }}
        data Pair {{ left: View; right: View; }}
        data Choice {{ case Selected(view: View); case Empty; }}
        data OuterChoice {{ inner: Choice; }}
        data Main {{ value: {scalar}; other: {scalar}; audit: {scalar}; }}
        machine write_outer(mut outer: Outer) {{ outer.inner.body = 255; }}
        machine write_pair(mut pair: Pair) {{ pair.left.body = 255; pair.right.body = 255; }}
        machine write_array(mut views: [View; 2]) {{ views[0].body = 255; }}
        machine write_choice(mut choice: Choice) {{ choice.view.body = 255; }}
        machine write_outer_choice(mut outer: OuterChoice) {{ outer.inner.view.body = 255; }}
        machine write_owned(mut outer: Outer) {{ outer.inner.tag = 1; }}
        machine write_outer_value(mut outer: Outer) -> u64 {{ outer.inner.body = 255; 0 }}
        {unknown_machine}
        machine audited<'value, 'audit>(value: &'value mut {scalar}, audit: &'audit mut {scalar}) -> &'value mut {scalar} {{ audit = 1; value }}
        machine Main::run(&mut self, mut input: View, outer: Outer, values: [View; 2], choice: Choice, index: u64) {{ {body} }}
        machine foreign(input: View) {{ let local: View = input; }}
    "#
    );
    typed_program(&source)
}

#[test]
fn immediate_literal_moves_preserve_complete_caller_reference_frames() {
    let cases = [
        (
            "record",
            "let local: View = View { body: &mut self.value }; write_outer(Outer { inner: local });",
            Some(vec!["self.value"]),
        ),
        (
            "explicit_move",
            "let local: View = View { body: &mut self.value }; write_outer(Outer { inner: move local });",
            Some(vec!["self.value"]),
        ),
        (
            "array",
            "let local: View = View { body: &mut self.value }; let second: View = View { body: &mut self.other }; write_array([local, second]);",
            Some(vec!["self.other", "self.value"]),
        ),
        (
            "selected_case",
            "let local: View = View { body: &mut self.value }; write_choice(Choice::Selected { view: local });",
            Some(vec!["self.value"]),
        ),
        (
            "local_field",
            "let local: Outer = Outer { inner: View { body: &mut self.value } }; write_outer(Outer { inner: local.inner });",
            Some(vec!["self.value"]),
        ),
        (
            "local_fixed_element",
            "let local: [View; 2] = [View { body: &mut self.value }, View { body: &mut self.other }]; write_outer(Outer { inner: local[0] });",
            Some(vec!["self.other", "self.value"]),
        ),
        (
            "local_runtime_element",
            "let local: [View; 2] = [View { body: &mut self.value }, View { body: &mut self.other }]; write_outer(Outer { inner: local[index] });",
            Some(vec!["self.other", "self.value"]),
        ),
        (
            "frozen_alias",
            "let mut alias: &mut u64 = &mut self.value; let local: View = View { body: alias }; alias = &mut self.other; write_outer(Outer { inner: local });",
            Some(vec!["self.value"]),
        ),
        (
            "owned_field",
            "let local: View = View { body: &mut self.value }; write_owned(Outer { inner: local });",
            Some(vec![]),
        ),
        (
            "empty_whole_case",
            "let local: Choice = Choice::Empty {}; write_outer_choice(OuterChoice { inner: local });",
            Some(vec![]),
        ),
        (
            "unknown_field_demand",
            "let local: View = View { body: &mut self.value }; write_unknown(Outer { inner: local });",
            None,
        ),
        (
            "parameter",
            "write_outer(Outer { inner: input });",
            Some(vec!["input.body"]),
        ),
        (
            "parameter_field",
            "write_outer(Outer { inner: outer.inner });",
            Some(vec!["outer.inner.body"]),
        ),
        (
            "parameter_fixed_element",
            "write_outer(Outer { inner: values[0] });",
            Some(vec!["values"]),
        ),
        (
            "parameter_runtime_element",
            "write_outer(Outer { inner: values[index] });",
            Some(vec!["values"]),
        ),
        (
            "parameter_whole_case",
            "write_outer_choice(OuterChoice { inner: choice });",
            Some(vec!["choice.view.body"]),
        ),
        (
            "missing_source",
            "write_outer(Outer { inner: absent });",
            None,
        ),
        (
            "missing_leaf",
            "let local: View = View {}; write_outer(Outer { inner: local });",
            None,
        ),
        (
            "opaque_leaf",
            "let local: View = View { body: unknown(&mut self.value) }; write_outer(Outer { inner: local });",
            None,
        ),
        (
            "wrong_nominal",
            "let local: OtherView = OtherView { body: &mut self.value }; write_outer(Outer { inner: local });",
            None,
        ),
        (
            "absent_payload",
            "let local: Choice = Choice::Empty {}; write_outer(Outer { inner: local.view });",
            None,
        ),
        (
            "selected_payload_with_prefix_evidence",
            "let local: Choice = Choice::Selected { view: View { body: &mut self.value } }; write_outer(Outer { inner: local.view });",
            Some(vec!["self.value"]),
        ),
        (
            // The parameter's Selected payload may be absent, which traps the
            // access; when it is present the write reaches its declared leaf.
            "unknown_parameter_payload",
            "write_outer(Outer { inner: choice.view });",
            Some(vec!["choice.view.body"]),
        ),
        (
            // The rebind writes the parameter's leaf slot, and the moved
            // carrier's payload write follows the replacement's own origin.
            "parameter_slot_replacement",
            "input.body = &mut self.other; write_outer(Outer { inner: input });",
            Some(vec!["input.body", "self.other"]),
        ),
        (
            "parameter_carrier_borrow",
            "unknown(&mut input); write_outer(Outer { inner: input });",
            None,
        ),
        (
            // The leaf-slot rebind claims the overwritten leaf while the
            // moved carrier's payload write follows the replacement's origin.
            "local_slot_replacement",
            "let mut local: View = View { body: &mut self.value }; local.body = &mut self.other; write_outer(Outer { inner: local });",
            Some(vec!["self.other", "self.value"]),
        ),
        (
            "unknown_sibling",
            "let local: View = View { body: &mut self.value }; write_array([local, absent]);",
            None,
        ),
        (
            "out_of_bounds",
            "write_outer(Outer { inner: values[2] });",
            None,
        ),
        (
            "negative_index",
            "write_outer(Outer { inner: values[-1] });",
            None,
        ),
    ];
    let mut failures = Vec::new();
    for (name, body, expected) in cases {
        let program = literal_move_program(body, "u64");
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .expect("caller");
        let state = &program.machine_states(machine)[0];
        let StatementNode::Call(call) = program
            .statement_table
            .statements(state.statement_nodes)
            .last()
            .expect("call")
        else {
            panic!("call");
        };
        let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
        for (query, paths) in [
            (
                "state",
                resolver
                    .inferred_state_write_frame(machine, state)
                    .into_complete_paths(),
            ),
            ("public call", resolver.may_write_paths(machine, call)),
        ] {
            let actual = paths.map(|paths| {
                let mut visible = Vec::new();
                for mut path in paths {
                    for (index, parameter) in program
                        .state_parameters(state)
                        .iter()
                        .filter(|parameter| !parameter.is_self)
                        .enumerate()
                    {
                        if let Some(suffix) = path.strip_prefix(&format!("$P{index}")) {
                            path = format!("{}{suffix}", parameter.name.as_str());
                            break;
                        }
                    }
                    let root = path.split('.').next().expect("root");
                    if root == "self"
                        || program
                            .state_parameters(state)
                            .iter()
                            .any(|parameter| parameter.name.as_str() == root)
                    {
                        visible.push(path);
                    }
                }
                visible.sort();
                visible.dedup();
                visible
            });
            let expected = expected.as_ref().map(|paths| {
                let mut paths: Vec<String> = paths.iter().map(|path| (*path).to_owned()).collect();
                if name == "local_slot_replacement" && query == "public call" {
                    paths.retain(|path| path != "self.value");
                }
                paths
            });
            if actual != expected {
                failures.push(format!(
                    "{name} {query}: expected {expected:?}, got {actual:?}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn immediate_literal_moves_preserve_complete_empty_owned_and_shared_frames() {
    for (name, source) in [
        (
            "zero_length_array",
            r#"
            data Outer { inner: [u64; 0]; }
            machine write_outer(mut outer: Outer) { outer.inner = []; }
            machine probe() {
                let local: [u64; 0] = [];
                write_outer(Outer { inner: local });
            }
            "#,
        ),
        (
            "shared_only_subrecord",
            r#"
            data SharedView { body: &u64; tag: u64; }
            data SharedOuter { inner: SharedView; }
            data Outer { inner: SharedView; }
            data Main { value: u64; }
            machine write_outer(mut outer: Outer) { outer.inner.tag = 1; }
            machine Main::probe(&self) {
                let local: SharedOuter = SharedOuter {
                    inner: SharedView { body: &self.value, tag: 0 }
                };
                write_outer(Outer { inner: local.inner });
            }
            "#,
        ),
    ] {
        let program = typed_program(source);
        let machine = program
            .machines()
            .iter()
            .find(|machine| matches!(machine.name.as_str(), "probe" | "Main::probe"))
            .expect("caller");
        let state = &program.machine_states(machine)[0];
        let StatementNode::Call(call) = program
            .statement_table
            .statements(state.statement_nodes)
            .last()
            .expect("call")
        else {
            panic!("call");
        };
        let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
        for (query, paths) in [
            (
                "state",
                resolver
                    .inferred_state_write_frame(machine, state)
                    .into_complete_paths(),
            ),
            ("public call", resolver.may_write_paths(machine, call)),
        ] {
            assert_eq!(paths, Some(Vec::new()), "{name} {query}");
        }
        lower_typed_trees(program, &CheckingRequest::settled())
            .expect("zero-leaf literal move reaches checked trees");
    }
}
