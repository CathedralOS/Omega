use crate::tests::front_end::typed_program;
use typed_trees::statement::StatementNode;

fn case_result_program(body: &str) -> typed_trees::TypedTrees {
    case_result_program_with_helpers(body, "")
}

fn case_result_program_with_helpers(body: &str, helpers: &str) -> typed_trees::TypedTrees {
    let source = format!(
        r#"
        data View {{ body: &mut u64; }}
        data Outer {{ inner: View; }}
        data Choice {{ case Selected(view: View); case Empty; }}
        data Nested {{ inner: Choice; }}
        data Pair {{ left: Choice; right: Choice; }}
        data Plain {{ tag: u64; }}
        data PlainChoice {{ case Selected(view: Plain); case Empty; }}
        data PlainOuter {{ inner: Plain; }}
        data Shared {{ body: &u64; tag: u64; }}
        data SharedChoice {{ case Selected(view: Shared); case Empty; }}
        data SharedOuter {{ inner: Shared; }}
        data Main {{ value: u64; other: u64; audit: u64; }}
        machine identity(input: Choice) -> Choice {{ input }}
        machine identity_nested(input: Nested) -> Nested {{ input }}
        machine identity_array(input: [Choice; 2]) -> [Choice; 2] {{ input }}
        machine identity_pair(input: Pair) -> Pair {{ input }}
        machine identity_plain(input: PlainChoice) -> PlainChoice {{ input }}
        machine identity_shared(input: SharedChoice) -> SharedChoice {{ input }}
        machine copy_input(input: Choice) -> Choice {{ let local: Choice = input; local }}
        machine wrap_input(input: Choice) -> Nested {{ Nested {{ inner: input }} }}
        machine project_nested(input: Nested) -> Choice {{ input.inner }}
        machine project_array(input: [Choice; 2]) -> Choice {{ input[0] }}
        machine project_runtime(input: [Choice; 2], index: u64) -> Choice {{ input[index] }}
        machine project_case(input: Choice) -> View {{ input.view }}
        machine forward(input: Choice) -> Choice {{ identity(input) }}
        machine make_choice(value: &mut u64) -> Choice {{ Choice::Selected {{ view: View {{ body: value }} }} }}
        machine replace_empty(input: Choice) -> Choice {{ Choice::Empty {{}} }}
        machine replace_selected(input: Choice, value: &mut u64) -> Choice {{ Choice::Selected {{ view: View {{ body: value }} }} }}
        machine private_result(input: Choice) -> Choice {{ let mut local: u64 = 0; Choice::Selected {{ view: View {{ body: &mut local }} }} }}
        machine private_pair(input: Choice) -> Pair {{ let mut local: u64 = 0; Pair {{ left: input, right: Choice::Selected {{ view: View {{ body: &mut local }} }} }} }}
        machine effectful(input: Choice, audit: &mut u64) -> Choice {{ audit = 1; input }}
        machine write_outer(mut outer: Outer) {{ outer.inner.body = 1; }}
        machine write_value(mut outer: Outer) -> u64 {{ outer.inner.body = 1; 0 }}
        machine write_plain(mut outer: PlainOuter) {{ outer.inner.tag = 1; }}
        machine write_shared(mut outer: SharedOuter) {{ outer.inner.tag = 1; }}
        machine Main::run(&mut self, input: Choice, index: u64) {{ {body} }}
        {helpers}
        "#
    );
    typed_program(&source)
}

fn visible_paths(paths: Option<Vec<String>>) -> Option<Vec<String>> {
    paths.map(|paths| {
        let mut paths = paths
            .into_iter()
            .filter(|path| path == "self" || path.starts_with("self."))
            .collect::<Vec<_>>();
        paths.sort();
        paths.dedup();
        paths
    })
}

fn caller_frames(program: &typed_trees::TypedTrees) -> [Option<Vec<String>>; 2] {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("caller");
    let state = &program.machine_states(machine)[0];
    let resolver = validation::CallFrameResolver::new(program).expect("resolver");
    let public = match program
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .expect("consumer")
    {
        StatementNode::Call(call) => resolver.may_write_paths(machine, call),
        StatementNode::LocalData(local) => {
            resolver.expression_may_write_paths(machine, local.initial_value)
        }
        _ => panic!("consumer"),
    };
    [
        resolver
            .inferred_state_write_frame(machine, state)
            .into_complete_paths(),
        public,
    ]
    .map(visible_paths)
}

#[test]
fn helper_result_cases_preserve_array_selectors_and_runtime_unions() {
    for (name, body, expected) in [
        (
            "fixed_selected",
            "let local: [Choice; 2] = identity_array([Choice::Selected { view: View { body: &mut self.value } }, Choice::Empty {}]); write_outer(Outer { inner: local[0].view });",
            Some(vec!["self.value"]),
        ),
        (
            "fixed_second",
            "let local: [Choice; 2] = identity_array([Choice::Empty {}, Choice::Selected { view: View { body: &mut self.other } }]); write_outer(Outer { inner: local[1].view });",
            Some(vec!["self.other"]),
        ),
        (
            "projected_selected",
            "let local: Choice = project_array([Choice::Selected { view: View { body: &mut self.value } }, Choice::Empty {}]); write_outer(Outer { inner: local.view });",
            Some(vec!["self.value"]),
        ),
        (
            "runtime_selected",
            "let local: [Choice; 2] = identity_array([Choice::Selected { view: View { body: &mut self.value } }, Choice::Selected { view: View { body: &mut self.other } }]); write_outer(Outer { inner: local[index].view });",
            Some(vec!["self.other", "self.value"]),
        ),
        (
            "projected_runtime_selected",
            "let local: Choice = project_runtime([Choice::Selected { view: View { body: &mut self.value } }, Choice::Selected { view: View { body: &mut self.other } }], index); write_outer(Outer { inner: local.view });",
            Some(vec!["self.other", "self.value"]),
        ),
        (
            "fixed_absent",
            "let local: [Choice; 2] = identity_array([Choice::Selected { view: View { body: &mut self.value } }, Choice::Empty {}]); write_outer(Outer { inner: local[1].view });",
            None,
        ),
        (
            // A runtime-mixed payload is a checked partial access: selecting
            // Empty traps the projection, so the Selected leaf is the only
            // reachable write.
            "runtime_mixed",
            "let local: [Choice; 2] = identity_array([Choice::Selected { view: View { body: &mut self.value } }, Choice::Empty {}]); write_outer(Outer { inner: local[index].view });",
            Some(vec!["self.value"]),
        ),
        (
            "projected_runtime_mixed",
            "let local: Choice = project_runtime([Choice::Selected { view: View { body: &mut self.value } }, Choice::Empty {}], index); write_outer(Outer { inner: local.view });",
            Some(vec!["self.value"]),
        ),
    ] {
        let expected = expected.map(|paths| paths.into_iter().map(str::to_owned).collect());
        assert_eq!(
            caller_frames(&case_result_program(body)),
            [expected.clone(), expected],
            "{name}"
        );
    }
}

#[test]
fn helper_bodies_transfer_mutable_case_state_through_proven_replacements() {
    let helpers = r#"
        machine reset_selected(mut input: Choice, value: &mut u64) -> Choice { input = Choice::Selected { view: View { body: value } }; input }
        machine reset_empty(mut input: Choice) -> Choice { input = Choice::Empty {}; input }
        machine reset_via(mut input: Choice, value: &mut u64) -> Choice { input = identity(Choice::Selected { view: View { body: value } }); input }
        machine reset_self(mut input: Choice) -> Choice { input = identity(input); input }
        machine reassign_twice(mut input: Choice, value: &mut u64) -> Choice { input = Choice::Empty {}; input = Choice::Selected { view: View { body: value } }; input }
        machine clear_choice(value: &mut Choice) { value = Choice::Empty {}; }
        machine expose_choice(mut input: Choice) -> Choice { clear_choice(&mut input); input }
        machine set_payload(mut input: Choice, value: &mut u64) -> Choice { input.view.body = value; input }
        machine Choice::reset(&mut self, value: &mut u64) { self = Choice::Selected { view: View { body: value } }; }
        machine receive_choice(mut input: Choice, value: &mut u64) -> Choice { input.reset(value); input }
    "#;
    // A whole-carrier assignment retires the parameter's frozen rows and
    // installs the replacement's own proven origins, so a later result reads
    // the replacement rather than the caller's overwritten actual. Replacing
    // a reference leaf, lending the binding to a nested call, or exposing it
    // through a mutable receiver stays opaque.
    for (name, body, expected) in [
        (
            "reset_selected",
            "let local: Choice = reset_selected(Choice::Empty {}, &mut self.value); write_outer(Outer { inner: local.view });",
            [Some(vec!["self.value"]), Some(vec!["self.value"])],
        ),
        (
            // The carrier write itself still claims the overwritten actual's
            // leaf; the replacement payload supplies the consumer's origin.
            "reset_selected_over_selected",
            "let local: Choice = reset_selected(Choice::Selected { view: View { body: &mut self.other } }, &mut self.value); write_outer(Outer { inner: local.view });",
            [
                Some(vec!["self.other", "self.value"]),
                Some(vec!["self.value"]),
            ],
        ),
        (
            // The replacement's Empty case is proven, so the payload access on
            // the result is a checked partial operation with no reachable
            // write.
            "reset_empty",
            "let local: Choice = reset_empty(Choice::Selected { view: View { body: &mut self.value } }); write_outer(Outer { inner: local.view });",
            [None, None],
        ),
        (
            "reset_via",
            "let local: Choice = reset_via(Choice::Empty {}, &mut self.value); write_outer(Outer { inner: local.view });",
            [Some(vec!["self.value"]), Some(vec!["self.value"])],
        ),
        (
            "reset_self",
            "let local: Choice = reset_self(Choice::Selected { view: View { body: &mut self.value } }); write_outer(Outer { inner: local.view });",
            [Some(vec!["self.value"]), Some(vec!["self.value"])],
        ),
        (
            // The final assignment owns the binding; the earlier Empty case
            // cannot resurrect its absent payload.
            "reassign_twice",
            "let local: Choice = reassign_twice(Choice::Empty {}, &mut self.value); write_outer(Outer { inner: local.view });",
            [Some(vec!["self.value"]), Some(vec!["self.value"])],
        ),
        (
            "expose_choice",
            "let local: Choice = expose_choice(Choice::Selected { view: View { body: &mut self.value } }); write_outer(Outer { inner: local.view });",
            [None, None],
        ),
        (
            "set_payload",
            "let local: Choice = set_payload(Choice::Selected { view: View { body: &mut self.other } }, &mut self.value); write_outer(Outer { inner: local.view });",
            [None, None],
        ),
        (
            "receive_choice",
            "let local: Choice = receive_choice(Choice::Empty {}, &mut self.value); write_outer(Outer { inner: local.view });",
            [None, None],
        ),
    ] {
        assert_eq!(
            caller_frames(&case_result_program_with_helpers(body, helpers)),
            expected.map(|paths| paths
                .map(|paths: Vec<&str>| paths.into_iter().map(str::to_owned).collect())),
            "{name}",
        );
    }
}
