use super::{Lexer, ResolutionRequest, lower_symbol_resolved_trees, parse_syntax_trees, resolve};
use typed_trees::statement::StatementNode;

/// Conditional helper bodies route their result through match arms. When
/// every producing arm resolves to the same caller-storage path, the binding
/// keeps that one origin; when arms diverge on an aggregate result the
/// relation keeps the exact finite union, and a single-origin reference
/// binding stays opaque. An arm that cannot resolve at all keeps the whole
/// route opaque.
fn conditional_program(body: &str) -> typed_trees::TypedTrees {
    conditional_program_with_helpers(body, "")
}

fn conditional_program_with_helpers(body: &str, helpers: &str) -> typed_trees::TypedTrees {
    let source = format!(
        r#"
        data View {{ body: &mut u64; }}
        data Outer {{ inner: View; }}
        data Choice {{ case Selected(view: View); case Empty; }}
        data Main {{ value: u64; other: u64; audit: u64; tag: u64; }}
        machine consume(value: &mut u64) {{ value = 1; }}
        machine write_outer(mut outer: Outer) {{ outer.inner.body = 1; }}
        machine bump(value: &mut u64) -> u64 {{ value = 1; 0 }}
        machine opaque_ref(value: &mut u64) -> &mut u64 {{ opaque_ref(value) }}
        machine identity_ref(value: &mut u64) -> &mut u64 {{ value }}
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

#[test]
fn conditional_reference_results_keep_a_convergent_origin() {
    let helpers = r#"
        machine pick(a: &mut u64, b: &mut u64, tag: u64) -> &mut u64 { match tag { 0 -> a, _ -> a } }
        machine pick_nested(a: &mut u64, tag: u64) -> &mut u64 {
            match tag { 0 -> a, _ -> match tag { 1 -> a, _ -> a } }
        }
        machine pick_through(a: &mut u64, tag: u64) -> &mut u64 {
            match tag { 0 -> a, _ -> identity_ref(a) }
        }
        machine pick_wildcard(a: &mut u64, tag: u64) -> &mut u64 { match tag { _ -> a } }
    "#;
    for (name, body) in [
        (
            "same_parameter",
            "let alias: &mut u64 = pick(&mut self.value, &mut self.other, self.tag); consume(&mut alias);",
        ),
        (
            "nested_match",
            "let alias: &mut u64 = pick_nested(&mut self.value, self.tag); consume(&mut alias);",
        ),
        (
            "transparent_arm",
            "let alias: &mut u64 = pick_through(&mut self.value, self.tag); consume(&mut alias);",
        ),
        (
            "wildcard_only",
            "let alias: &mut u64 = pick_wildcard(&mut self.value, self.tag); consume(&mut alias);",
        ),
    ] {
        let expected = Some(vec!["self.value".to_owned()]);
        assert_eq!(
            caller_frames(&conditional_program_with_helpers(body, helpers)),
            [expected.clone(), expected],
            "{name}"
        );
    }
}

#[test]
fn conditional_reference_results_stay_opaque_when_arms_diverge() {
    let helpers = r#"
        machine pick(a: &mut u64, b: &mut u64, tag: u64) -> &mut u64 { match tag { 0 -> a, _ -> b } }
        machine pick_late(a: &mut u64, b: &mut u64, tag: u64) -> &mut u64 {
            match tag { 0 -> a, _ -> match tag { 1 -> a, _ -> b } }
        }
        machine pick_opaque(a: &mut u64, tag: u64) -> &mut u64 { match tag { 0 -> a, _ -> opaque_ref(a) } }
        machine pick_loaded(a: &mut View, b: &mut u64, tag: u64) -> &mut u64 {
            match tag { 0 -> b, _ -> a.body }
        }
    "#;
    for (name, body) in [
        (
            "unproven_arm",
            "let alias: &mut u64 = pick_opaque(&mut self.value, self.tag); consume(&mut alias);",
        ),
        // A field loaded through a borrowed carrier is not an owned-storage
        // origin, so the arm stays opaque even when the other arm resolves.
        (
            "loaded_reference_arm",
            "let view: View = View { body: &mut self.other }; let alias: &mut u64 = pick_loaded(&mut view, &mut self.value, self.tag); consume(&mut alias);",
        ),
        // An unresolved arm keeps the binding opaque even when the other arm
        // is proven.
        (
            "caller_unproven_arm",
            "let alias: &mut u64 = match self.tag { 0 -> &mut self.value, _ -> opaque_ref(&mut self.value) }; consume(&mut alias);",
        ),
    ] {
        assert_eq!(
            caller_frames(&conditional_program_with_helpers(body, helpers)),
            [None, None],
            "{name}"
        );
    }
}

// A divergent exclusive binding lent through a direct exclusive reborrow
// call argument carries its whole proven referent set: the callee's
// parameter write instantiates through every admitted route rather than
// selecting one or failing closed.
#[test]
fn conditional_results_union_through_reborrow_arguments() {
    let helpers = r#"
        machine pick(a: &mut u64, b: &mut u64, tag: u64) -> &mut u64 { match tag { 0 -> a, _ -> b } }
        machine pick_late(a: &mut u64, b: &mut u64, tag: u64) -> &mut u64 {
            match tag { 0 -> a, _ -> match tag { 1 -> a, _ -> b } }
        }
    "#;
    for (name, body) in [
        (
            "different_parameters",
            "let alias: &mut u64 = pick(&mut self.value, &mut self.other, self.tag); consume(&mut alias);",
        ),
        (
            "late_divergence",
            "let alias: &mut u64 = pick_late(&mut self.value, &mut self.other, self.tag); consume(&mut alias);",
        ),
        // A caller-side binding whose arms carry different proven referents
        // keeps their exact finite union.
        (
            "caller_divergent",
            "let alias: &mut u64 = match self.tag { 0 -> &mut self.value, _ -> &mut self.other }; consume(&mut alias);",
        ),
    ] {
        let expected = Some(vec!["self.other".to_owned(), "self.value".to_owned()]);
        assert_eq!(
            caller_frames(&conditional_program_with_helpers(body, helpers)),
            [expected.clone(), expected],
            "{name}"
        );
    }
}

#[test]
fn conditional_bindings_keep_a_convergent_origin() {
    for (name, body) in [
        (
            "same_referent",
            "let alias: &mut u64 = match self.tag { 0 -> &mut self.value, _ -> &mut self.value }; consume(&mut alias);",
        ),
        (
            "nested_same_referent",
            "let alias: &mut u64 = match self.tag { 0 -> &mut self.value, _ -> match self.tag { 1 -> &mut self.value, _ -> &mut self.value } }; consume(&mut alias);",
        ),
    ] {
        let program = conditional_program(body);
        let expected = Some(vec!["self.value".to_owned()]);
        assert_eq!(
            caller_frames(&program),
            [expected.clone(), expected],
            "{name}"
        );
    }
}

#[test]
fn conditional_aggregate_results_keep_their_arm_leaves() {
    let helpers = r#"
        machine pick_choice(a: &mut u64, tag: u64) -> Choice {
            match tag {
                0 -> Choice::Selected { view: View { body: a } },
                _ -> Choice::Selected { view: View { body: a } }
            }
        }
        machine pick_choice_split(a: &mut u64, b: &mut u64, tag: u64) -> Choice {
            match tag {
                0 -> Choice::Selected { view: View { body: a } },
                _ -> Choice::Selected { view: View { body: b } }
            }
        }
        machine pick_choice_mixed(a: &mut u64, tag: u64) -> Choice {
            match tag {
                0 -> Choice::Selected { view: View { body: a } },
                _ -> Choice::Empty {}
            }
        }
        machine pick_choice_opaque(a: &mut u64, tag: u64) -> Choice {
            match tag {
                0 -> Choice::Selected { view: View { body: a } },
                _ -> Choice::Selected { view: View { body: opaque_ref(a) } }
            }
        }
    "#;
    for (name, body, expected) in [
        // Every producing arm resolves to the same caller-storage path.
        (
            "same_leaf",
            "let local: Choice = pick_choice(&mut self.value, self.tag); write_outer(Outer { inner: local.view });",
            Some(vec!["self.value"]),
        ),
        // Divergent arms keep the exact finite union, matching the runtime
        // index union semantics for stored carriers.
        (
            "split_leaf",
            "let local: Choice = pick_choice_split(&mut self.value, &mut self.other, self.tag); write_outer(Outer { inner: local.view });",
            Some(vec!["self.other", "self.value"]),
        ),
        // The Empty arm contributes the case but no leaf; the payload access
        // is a checked partial operation reached only through Selected.
        (
            "mixed_cases",
            "let local: Choice = pick_choice_mixed(&mut self.value, self.tag); write_outer(Outer { inner: local.view });",
            Some(vec!["self.value"]),
        ),
        // An unproven leaf in one arm keeps the whole route opaque.
        (
            "unproven_arm",
            "let local: Choice = pick_choice_opaque(&mut self.value, self.tag); write_outer(Outer { inner: local.view });",
            None,
        ),
    ] {
        let expected = expected.map(|paths| paths.into_iter().map(str::to_owned).collect());
        assert_eq!(
            caller_frames(&conditional_program_with_helpers(body, helpers)),
            [expected.clone(), expected],
            "{name}"
        );
    }
}

#[test]
fn conditional_result_subjects_keep_their_own_writes() {
    let helpers = r#"
        machine pick(a: &mut u64, tag: u64) -> &mut u64 { match tag { 0 -> a, _ -> a } }
    "#;
    // The convergent alias keeps `self.value`; the match subject's own call
    // still publishes `self.audit` in the inferred caller frame. The public
    // per-call answer for `consume(&mut alias)` covers only that call.
    let program = conditional_program_with_helpers(
        "let alias: &mut u64 = pick(&mut self.value, bump(&mut self.audit)); consume(&mut alias);",
        helpers,
    );
    assert_eq!(
        caller_frames(&program),
        [
            Some(vec!["self.audit".to_owned(), "self.value".to_owned()]),
            Some(vec!["self.value".to_owned()]),
        ]
    );
}
