use super::{Lexer, ResolutionRequest, lower_symbol_resolved_trees, parse_syntax_trees, resolve};
use typed_trees::statement::StatementNode;

fn probe_program(body: &str) -> typed_trees::TypedTrees {
    probe_program_with_helpers(body, "")
}

fn probe_program_with_helpers(body: &str, helpers: &str) -> typed_trees::TypedTrees {
    let source = format!(
        r#"
        data View {{ body: &mut u64; }}
        data Main {{ value: u64; other: u64; audit: u64; tag: u64; }}
        machine consume(value: &mut u64) {{ value = 1; }}
        machine write_through(value: &mut u64) -> u64 {{ value = 1; 0 }}
        machine opaque_ref(value: &mut u64) -> &mut u64 {{ opaque_ref(value) }}
        machine pick(a: &mut u64, b: &mut u64, tag: u64) -> &mut u64 {{ match tag {{ 0 -> a, _ -> b }} }}
        machine pick_view(a: View, b: View, tag: u64) -> &mut u64 {{ match tag {{ 0 -> a.body, _ -> b.body }} }}
        machine forward_pick(a: &mut u64, b: &mut u64, tag: u64) -> &mut u64 {{ pick(a, b, tag) }}
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

// A helper whose conditional result arms each resolve to a proven reference
// parameter yields the exact finite union of its candidate origins: a write
// through the result may land on either actual, so both join the frame.
#[test]
fn divergent_result_arms_keep_their_candidate_union() {
    for (name, body, expected) in [
        // A computed `&mut` argument outside a single-origin relation: each
        // arm's proven parameter supplies one candidate.
        (
            "direct_divergent_pick",
            "consume(pick(&mut self.value, &mut self.other, self.tag));",
            ["self.other", "self.value"].as_slice(),
        ),
        // The same union inside a value-position call argument.
        (
            "expression_position_pick",
            "let sink: u64 = write_through(pick(&mut self.value, &mut self.other, self.tag));",
            ["self.other", "self.value"].as_slice(),
        ),
        // A match actual diverging on direct places resolves the same way.
        (
            "direct_match_actual",
            "consume(match self.tag { 0 -> &mut self.value, _ -> &mut self.other });",
            ["self.other", "self.value"].as_slice(),
        ),
        // A nested helper returning the divergent call composes per arm.
        (
            "forwarded_divergent_pick",
            "consume(forward_pick(&mut self.value, &mut self.other, self.tag));",
            ["self.other", "self.value"].as_slice(),
        ),
        // Convergent arms still collapse to the one shared origin.
        (
            "convergent_pick",
            "consume(match self.tag { 0 -> &mut self.value, _ -> &mut self.value });",
            ["self.value"].as_slice(),
        ),
    ] {
        let program = probe_program(body);
        let expected = expected
            .iter()
            .map(|path| (*path).to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            caller_frames(&program),
            [Some(expected.clone()), Some(expected)],
            "{name}",
        );
    }
}

// Divergent carrier-leaf arms instantiate each side through its own actual.
#[test]
fn divergent_carrier_leaf_arms_union_actual_leaf_origins() {
    let program = probe_program(
        "let first: View = View { body: &mut self.value }; let second: View = View { body: &mut self.other }; consume(pick_view(first, second, self.tag));",
    );
    let expected = vec!["self.other".to_owned(), "self.value".to_owned()];
    assert_eq!(
        caller_frames(&program),
        [Some(expected.clone()), Some(expected)],
    );
}

// Routes that cannot name every arm's provenance still fail closed: a
// recursive helper, an arm landing on helper-private storage, and a divergent
// binding rebinding all keep an opaque frame rather than selecting one route.
#[test]
fn unproven_candidate_routes_stay_opaque() {
    for (name, helpers, body) in [
        // A recursive helper has no finite candidate set.
        (
            "recursive_result",
            "",
            "consume(opaque_ref(&mut self.value));",
        ),
        // An arm that binds helper-private storage supplies no caller route.
        (
            "private_arm",
            "machine pick_private(a: &mut u64, b: &mut u64, tag: u64) -> &mut u64 { let local: u64 = 0; match tag { 0 -> a, _ -> &mut local } }",
            "consume(pick_private(&mut self.value, &mut self.other, self.tag));",
        ),
        // A divergent result bound to a reference local has no single alias
        // origin, so the binding itself stays opaque.
        (
            "divergent_local_binding",
            "",
            "let alias: &mut u64 = pick(&mut self.value, &mut self.other, self.tag); alias = 1; consume(&mut self.audit);",
        ),
    ] {
        let program = probe_program_with_helpers(body, helpers);
        let [state, _public] = caller_frames(&program);
        assert!(
            state.is_none(),
            "{name} state frame must stay opaque: {state:?}"
        );
    }
}
