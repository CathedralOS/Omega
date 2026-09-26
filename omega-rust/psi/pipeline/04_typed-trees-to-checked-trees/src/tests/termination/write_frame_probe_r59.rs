use crate::tests::front_end::typed_program;
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

// Opaque callees, literal carriers, rebound parameters, lent carrier slots,
// and reborrowed bindings all stay opaque.
#[test]
fn divergent_or_unproven_carrier_results_stay_opaque() {
    for (name, helpers, body) in [
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
    ] {
        let program = probe_program(body, helpers);
        let [state, _public] = caller_frames(&program);
        assert!(
            state.is_none(),
            "{name} state frame must stay opaque: {state:?}"
        );
    }
}
