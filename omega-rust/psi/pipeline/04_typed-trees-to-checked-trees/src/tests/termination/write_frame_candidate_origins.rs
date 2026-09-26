use crate::tests::front_end::typed_program;
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

// A method call on a divergent computed receiver resolves to its attached
// machine by target symbol alone; the callee's `self`-relative writes then
// instantiate under every proven candidate and union. The divergent set is
// evidence of where the receiver may land, never a spelling that selects the
// callee.
#[test]
fn divergent_computed_receiver_unions_candidate_writes() {
    for (name, helpers, body, expected) in [
        (
            "pick_receiver",
            "machine Cell::set_n(&mut self, x: u64) -> u64 { self.n = x; x }",
            "let sink: u64 = pick_cell(&mut self.c1, &mut self.c2, self.tag).set_n(1);",
            ["self.c1.n", "self.c2.n"].as_slice(),
        ),
        // The same finite set reached through the binding's NAME rather than
        // the computing expression. A divergent local used as a receiver had
        // no member-chain spelling to check, so the demand walk refused it
        // before contextual substitution even though every route was proven.
        (
            "name_receiver",
            "machine Cell::set_n(&mut self, x: u64) -> u64 { self.n = x; x }",
            "let chosen: &mut Cell = pick_cell(&mut self.c1, &mut self.c2, self.tag); let sink: u64 = chosen.set_n(1);",
            ["self.c1.n", "self.c2.n"].as_slice(),
        ),
        // A match expression receiver carries the same finite set.
        (
            "match_receiver",
            "machine Cell::set_n(&mut self, x: u64) -> u64 { self.n = x; x }",
            "let sink: u64 = (match self.tag { 0 -> &mut self.c1, _ -> &mut self.c2 }).set_n(1);",
            ["self.c1.n", "self.c2.n"].as_slice(),
        ),
    ] {
        let program = probe_program_with_helpers(body, helpers);
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
        // A receiver call whose result has no finite candidate set — a
        // recursive helper, or an arm landing on private storage — cannot
        // route `self`-relative callee writes and fails closed.
        (
            "recursive_receiver_result",
            "machine Cell::set_n(&mut self, x: u64) -> u64 { self.n = x; x }\n         machine bounce_cell(a: &mut Cell) -> &mut Cell { bounce_cell(a) }",
            "let sink: u64 = bounce_cell(&mut self.c1).set_n(1);",
        ),
        (
            "private_arm_receiver",
            "machine Cell::set_n(&mut self, x: u64) -> u64 { self.n = x; x }\n         machine pick_priv_cell(a: &mut Cell, tag: u64) -> &mut Cell { let local: Cell = Cell { n: 0 }; match tag { 0 -> a, _ -> &mut local } }",
            "let sink: u64 = pick_priv_cell(&mut self.c1, self.tag).set_n(1);",
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
