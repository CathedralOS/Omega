use super::{Lexer, ResolutionRequest, lower_symbol_resolved_trees, parse_syntax_trees, resolve};
use typed_trees::statement::StatementNode;

fn probe_program(body: &str, helpers: &str) -> typed_trees::TypedTrees {
    let source = format!(
        r#"
        data View {{ body: &mut u64; }}
        data Outer {{ inner: View; }}
        data Main {{ value: u64; other: u64; audit: u64; tag: u64; }}
        machine consume(value: &mut u64) {{ value = 1; }}
        machine opaque_ref(value: &mut u64) -> &mut u64 {{ opaque_ref(value) }}
        machine Main::run(&mut self) {{ {body} }}
        {helpers}
        "#
    );
    let syntax =
        parse_syntax_trees(&Lexer::new(&source).tokenize().expect("tokenize")).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
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

fn assert_frames(
    program: &typed_trees::TypedTrees,
    state_paths: Option<&[&str]>,
    public_paths: Option<&[&str]>,
    name: &str,
) {
    let [state, public] = caller_frames(program);
    let expected_state = state_paths.map(|paths| {
        paths
            .iter()
            .map(|path| path.to_string())
            .collect::<Vec<_>>()
    });
    let expected_public = public_paths.map(|paths| {
        paths
            .iter()
            .map(|path| path.to_string())
            .collect::<Vec<_>>()
    });
    assert_eq!(state, expected_state, "{name} state frame");
    assert_eq!(public, expected_public, "{name} public answer");
}

// A helper returning `&mut` from a by-value carrier parameter's declared
// reference leaf preserves the exact caller storage origin.
#[test]
fn carrier_parameter_leaf_results_retain_exact_caller_origins() {
    let helpers = r#"
        machine pf(a: View) -> &mut u64 { a.body }
    "#;
    // The result binding composes with the caller's own writes.
    let program = probe_program(
        "let view: View = View { body: &mut self.value }; let alias: &mut u64 = pf(view); alias = 1; let sink: u64 = 0;",
        helpers,
    );
    assert_frames(
        &program,
        Some(&["self.value"]),
        Some(&[]),
        "carrier_leaf_assign",
    );
    // The same origin answers a direct call argument.
    let program = probe_program(
        "let view: View = View { body: &mut self.value }; consume(pf(view));",
        helpers,
    );
    assert_frames(
        &program,
        Some(&["self.value"]),
        Some(&["self.value"]),
        "direct_call_argument",
    );
    // A member actual projects the leaf through the nested carrier.
    let program = probe_program(
        "let outer: Outer = Outer { inner: View { body: &mut self.value } }; consume(pf(outer.inner));",
        helpers,
    );
    assert_frames(
        &program,
        Some(&["self.value"]),
        Some(&["self.value"]),
        "member_call_argument",
    );
}

// A nested carrier projection composes the same way.
#[test]
fn nested_carrier_leaf_results_retain_exact_caller_origins() {
    let helpers = r#"
        machine pf(a: Outer) -> &mut u64 { a.inner.body }
    "#;
    let program = probe_program(
        "let outer: Outer = Outer { inner: View { body: &mut self.value } }; let alias: &mut u64 = pf(outer); alias = 1; let sink: u64 = 0;",
        helpers,
    );
    assert_frames(&program, Some(&["self.value"]), Some(&[]), "nested_leaf");
}

// Conditional result arms converge only when every arm selects the same leaf.
#[test]
fn convergent_carrier_match_result_retains_exact_caller_origin() {
    let helpers = r#"
        machine pf(a: View, tag: u64) -> &mut u64 { match tag { 0 -> a.body, _ -> a.body } }
    "#;
    let program = probe_program(
        "let view: View = View { body: &mut self.value }; let alias: &mut u64 = pf(view, self.tag); alias = 1; let sink: u64 = 0;",
        helpers,
    );
    assert_frames(&program, Some(&["self.value"]), Some(&[]), "convergent");
}

// Divergent unions, opaque callees, literal carriers, rebound parameters, and
// lent carrier slots all stay opaque.
#[test]
fn divergent_or_unproven_carrier_results_stay_opaque() {
    for (name, helpers, body) in [
        // Divergent `-> &mut` result arms must not select one route.
        (
            "divergent_reference_arms",
            "machine pick(a: &mut u64, b: &mut u64, tag: u64) -> &mut u64 { match tag { 0 -> a, _ -> b } }",
            "let alias: &mut u64 = pick(&mut self.value, &mut self.other, self.tag); alias = 1; let sink: u64 = 0;",
        ),
        // Divergent carrier leaves must not select one route either.
        (
            "divergent_carrier_arms",
            "machine pf(a: View, b: View, tag: u64) -> &mut u64 { match tag { 0 -> a.body, _ -> b.body } }",
            "let view: View = View { body: &mut self.value }; let other: View = View { body: &mut self.other }; let alias: &mut u64 = pf(view, other, self.tag); alias = 1; let sink: u64 = 0;",
        ),
        // A recursive helper body has no finite convergent leaf.
        (
            "opaque_recursive_result",
            "machine pf(a: View) -> &mut u64 { opaque_ref(a.body) }",
            "let view: View = View { body: &mut self.value }; let alias: &mut u64 = pf(view); alias = 1; let sink: u64 = 0;",
        ),
        // A literal carrier actual has no caller place to name.
        (
            "literal_carrier_actual",
            "machine pf(a: View) -> &mut u64 { a.body }",
            "let alias: &mut u64 = pf(View { body: &mut self.value }); alias = 1; let sink: u64 = 0;",
        ),
        // Rebinding the carrier parameter leaves the result describing a
        // binding the helper no longer holds.
        (
            "rebound_carrier_parameter",
            "machine pf(mut a: View, b: View) -> &mut u64 { a = b; a.body }",
            "let view: View = View { body: &mut self.value }; let other: View = View { body: &mut self.other }; let alias: &mut u64 = pf(view, other); alias = 1; let sink: u64 = 0;",
        ),
        // Lending the carrier's leaf slot through an exclusive borrow lets a
        // callee rebind it, so the result cannot name the declared leaf.
        (
            "lent_carrier_leaf",
            "machine pf(mut a: View) -> &mut u64 { consume(&mut a.body); a.body }",
            "let view: View = View { body: &mut self.value }; let alias: &mut u64 = pf(view); alias = 1; let sink: u64 = 0;",
        ),
        // Reborrowing the bound result for another call stays behind the
        // frozen-binding exposure gate.
        (
            "reborrowed_result_binding",
            "machine pf(a: View) -> &mut u64 { a.body }",
            "let view: View = View { body: &mut self.value }; let alias: &mut u64 = pf(view); consume(&mut alias);",
        ),
        // The same exposure gate keeps a direct local carrier leaf closed.
        (
            "reborrowed_local_leaf",
            "",
            "let view: View = View { body: &mut self.value }; let alias: &mut u64 = view.body; consume(&mut alias);",
        ),
    ] {
        let program = probe_program(body, helpers);
        let [state, _public] = caller_frames(&program);
        assert!(
            state.is_none(),
            "{name} state frame must stay opaque: {state:?}"
        );
    }
}
