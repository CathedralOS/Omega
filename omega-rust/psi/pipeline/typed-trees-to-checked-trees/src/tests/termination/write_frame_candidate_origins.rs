use super::{Lexer, ResolutionRequest, lower_symbol_resolved_trees, parse_syntax_trees, resolve};
use typed_trees::statement::StatementNode;

fn probe_program(body: &str) -> typed_trees::TypedTrees {
    probe_program_with_helpers(body, "")
}

fn probe_program_with_helpers(body: &str, helpers: &str) -> typed_trees::TypedTrees {
    let source = format!(
        r#"
        data View {{ body: &mut u64; }}
        data Cell {{ n: u64; }}
        data Main {{ value: u64; other: u64; audit: u64; tag: u64; c1: Cell; c2: Cell; v1: View; v2: View; }}
        machine consume(value: &mut u64) {{ value = 1; }}
        machine write_through(value: &mut u64) -> u64 {{ value = 1; 0 }}
        machine identity(value: u64) -> u64 {{ value }}
        machine write_only(value: &write u64) -> u64 {{ value = 1; 0 }}
        machine opaque_ref(value: &mut u64) -> &mut u64 {{ opaque_ref(value) }}
        machine pick(a: &mut u64, b: &mut u64, tag: u64) -> &mut u64 {{ match tag {{ 0 -> a, _ -> b }} }}
        machine pick_view(a: View, b: View, tag: u64) -> &mut u64 {{ match tag {{ 0 -> a.body, _ -> b.body }} }}
        machine pick_carrier(a: &mut View, b: &mut View, tag: u64) -> &mut View {{ match tag {{ 0 -> a, _ -> b }} }}
        machine pick_cell(a: &mut Cell, b: &mut Cell, tag: u64) -> &mut Cell {{ match tag {{ 0 -> a, _ -> b }} }}
        machine pick_write_cell(a: &write Cell, b: &write Cell, tag: u64) -> &write Cell {{ match tag {{ 0 -> a, _ -> b }} }}
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

// A divergent result bound to a reference local keeps its whole referent set:
// a bare write through the binding lands on one proven candidate, so the
// state frame records the exact union. A later proven rebind replaces the
// set, and the write that follows instantiates the replacement alone.
#[test]
fn divergent_local_binding_writes_through_every_candidate() {
    for (name, body, expected) in [
        (
            "write_through_divergent_binding",
            "let alias: &mut u64 = pick(&mut self.value, &mut self.other, self.tag); alias = 1; consume(&mut self.audit);",
            ["self.audit", "self.other", "self.value"].as_slice(),
        ),
        // Rebinding to another proven place retires the first set entirely.
        (
            "proven_rebind_replaces_the_set",
            "let alias: &mut u64 = pick(&mut self.value, &mut self.other, self.tag); alias = &mut self.audit; alias = 1; consume(&mut self.tag);",
            ["self.audit", "self.tag"].as_slice(),
        ),
        // A member write through the binding composes its suffix onto every
        // proven carrier.
        (
            "member_write_through_divergent_binding",
            "let alias: &mut Cell = pick_cell(&mut self.c1, &mut self.c2, self.tag); alias.n = 1; consume(&mut self.audit);",
            ["self.audit", "self.c1.n", "self.c2.n"].as_slice(),
        ),
    ] {
        let program = probe_program(body);
        let expected = expected
            .iter()
            .map(|path| (*path).to_owned())
            .collect::<Vec<_>>();
        let [state, _public] = caller_frames(&program);
        assert_eq!(state, Some(expected), "{name}");
    }
}

// A divergent binding lent to a callee through a direct exclusive reborrow
// (`&mut alias`, with member or index projections composing onto every
// candidate) or passed as the actual itself keeps its whole proven referent
// set at the call boundary: the callee's parameter writes instantiate
// through every route the binding admits.
#[test]
fn divergent_binding_call_argument_unions_candidates() {
    for (name, body, expected) in [
        (
            "reborrow_of_divergent_binding",
            "let alias: &mut u64 = pick(&mut self.value, &mut self.other, self.tag); consume(&mut alias);",
            ["self.other", "self.value"].as_slice(),
        ),
        // A member projection inside the reborrow composes its suffix onto
        // every proven carrier.
        (
            "member_reborrow_of_divergent_binding",
            "let alias: &mut Cell = pick_cell(&mut self.c1, &mut self.c2, self.tag); consume(&mut alias.n);",
            ["self.c1.n", "self.c2.n"].as_slice(),
        ),
        // The bare binding as the actual lends the same referent set.
        (
            "binding_passed_as_actual",
            "let alias: &mut u64 = pick(&mut self.value, &mut self.other, self.tag); consume(alias);",
            ["self.other", "self.value"].as_slice(),
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

#[test]
fn divergent_origins_compose_through_expression_calls() {
    for (name, body, expected) in [
        (
            "value_call",
            "let alias: &mut u64 = pick(&mut self.value, &mut self.other, self.tag); let sink: u64 = write_through(&mut alias);",
            ["self.other", "self.value"].as_slice(),
        ),
        (
            "projected_value_call",
            "let alias: &mut Cell = pick_cell(&mut self.c1, &mut self.c2, self.tag); let sink: u64 = write_through(&mut alias.n);",
            ["self.c1.n", "self.c2.n"].as_slice(),
        ),
        (
            "surrounding_expression",
            "let alias: &mut u64 = pick(&mut self.value, &mut self.other, self.tag); let sink: u64 = write_through(&mut alias) + write_through(&mut self.audit);",
            ["self.audit", "self.other", "self.value"].as_slice(),
        ),
        (
            "nested_reference_call",
            "let alias: &mut u64 = pick(&mut self.value, &mut self.other, self.tag); consume(forward_pick(&mut alias, &mut self.audit, self.tag));",
            ["self.audit", "self.other", "self.value"].as_slice(),
        ),
        (
            "conditional_actual",
            "let alias: &mut u64 = pick(&mut self.value, &mut self.other, self.tag); consume(match self.tag { 0 -> &mut alias, _ -> &mut self.audit });",
            ["self.audit", "self.other", "self.value"].as_slice(),
        ),
        (
            "replacement_origin",
            "let alias: &mut u64 = pick(&mut self.value, &mut self.other, self.tag); alias = &mut self.audit; let sink: u64 = write_through(&mut alias);",
            ["self.audit"].as_slice(),
        ),
        (
            "scalar_actual_needs_effects_not_origins",
            "let alias: &mut u64 = pick(&mut self.value, &mut self.other, self.tag); let sink: u64 = identity(write_through(&mut alias));",
            ["self.other", "self.value"].as_slice(),
        ),
        (
            "transported_binding",
            "let alias: &mut u64 = pick(&mut self.value, &mut self.other, self.tag); let again: &mut u64 = alias; let sink: u64 = write_through(&mut again);",
            ["self.other", "self.value"].as_slice(),
        ),
        (
            "write_only_projection",
            "let alias: &write Cell = pick_write_cell(&write self.c1, &write self.c2, self.tag); let sink: u64 = write_only(&write alias.n);",
            ["self.c1.n", "self.c2.n"].as_slice(),
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
            "{name}"
        );
    }
}

// Routes that cannot name every arm's provenance still fail closed: a
// recursive helper, an arm landing on helper-private storage, a divergent
// binding rebound to an unproven source, and an unproven interior reference
// load keep an opaque frame rather than selecting one route.
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
        // Rebinding a divergent binding to an unproven result loses the set.
        (
            "divergent_binding_unproven_rebind",
            "",
            "let alias: &mut u64 = pick(&mut self.value, &mut self.other, self.tag); alias = opaque_ref(&mut self.audit); consume(&mut self.tag);",
        ),
        (
            "opaque_callee_cannot_drop_conditional_actual",
            "machine opaque_write(value: &mut u64) { opaque_write(value); }",
            "let alias: &mut u64 = pick(&mut self.value, &mut self.other, self.tag); opaque_write(match self.tag { 0 -> &mut alias, _ -> &mut self.audit });",
        ),
        // A member-read actual transports an interior reference value whose
        // own referent the frame cannot name.
        (
            "divergent_binding_member_actual",
            "",
            "let alias: &mut View = pick_carrier(&mut self.v1, &mut self.v2, self.tag); consume(alias.body);",
        ),
        (
            "divergent_binding_member_value_call",
            "",
            "let alias: &mut View = pick_carrier(&mut self.v1, &mut self.v2, self.tag); let sink: u64 = write_through(alias.body);",
        ),
        (
            "computed_projection_cannot_hide_reference_load",
            "machine identity_view(value: &mut View) -> &mut View { value }",
            "let alias: &mut View = pick_carrier(&mut self.v1, &mut self.v2, self.tag); consume(identity_view(alias).body);",
        ),
        // A value write into a reference-typed interior slot passes through
        // the slot's own referent, which the path frame cannot name.
        (
            "divergent_binding_reference_interior",
            "",
            "let alias: &mut View = pick_carrier(&mut self.v1, &mut self.v2, self.tag); alias.body = 1; consume(&mut self.audit);",
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

#[test]
fn divergent_named_state_transfer_cannot_drop_candidate_writes() {
    let program = probe_program(
        "let alias: &mut u64 = pick(&mut self.value, &mut self.other, self.tag);
        transition { _ -> next(alias) }
        state next(value: &mut u64) { value = 1; }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("caller");
    let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
    assert!(
        !resolver
            .inferred_state_write_frame(machine, &program.machine_states(machine)[0])
            .is_complete()
    );
}

#[test]
fn divergent_slice_candidates_keep_collection_coarse_writes() {
    let program = probe_program_with_helpers(
        "",
        r#"
        machine pick_slice(a: &mut [u64], b: &mut [u64], tag: u64) -> &mut [u64] {
            match tag { 0 -> a, _ -> b }
        }
        machine write_slice(a: &mut [u64], b: &mut [u64], tag: u64) {
            let alias: &mut [u64] = pick_slice(a, b, tag);
            let sink: u64 = write_through(&mut alias[0]);
        }
    "#,
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "write_slice")
        .expect("caller");
    let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
    let mut paths = resolver
        .inferred_state_write_frame(machine, &program.machine_states(machine)[0])
        .into_complete_paths()
        .expect("complete slice candidates");
    paths.sort();
    assert_eq!(paths, ["$P0", "$P1"]);
}
