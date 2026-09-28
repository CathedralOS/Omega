use crate::tests::front_end::typed_program;
use typed_trees::statement::StatementNode;

#[test]
fn named_state_payload_calls_retain_state_scoped_case_evidence() {
    for (name, body) in [
        (
            "acyclic_literal_edge",
            r#"
            let local: Choice = Choice::Selected { view: View { body: value } };
            transition { _ -> finish(Outer { inner: local.view }) }
            state finish(mut outer: Outer) { outer.inner.body = 1; }
        "#,
        ),
        (
            "call_inside_cycle",
            r#"
            transition { _ -> cycle(value) }
            state cycle(item: &mut u64) {
                let local: Choice = Choice::Selected { view: View { body: item } };
                write_outer(Outer { inner: local.view });
                transition { _ -> cycle(item) }
            }
        "#,
        ),
    ] {
        let source = format!(
            r#"
            data View {{ body: &mut u64; }}
            data Outer {{ inner: View; }}
            data Choice {{ case Selected(view: View); case Empty; }}
            machine write_outer(mut outer: Outer) {{ outer.inner.body = 1; }}
            machine probe(value: &mut u64) {{ {body} }}
        "#
        );
        let program = typed_program(&source);
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "probe")
            .expect("caller");
        let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
        for _ in 0..2 {
            assert_eq!(
                resolver
                    .inferred_state_write_frame(machine, &program.machine_states(machine)[0])
                    .into_complete_paths(),
                Some(vec!["$P0".to_owned()]),
                "{name}"
            );
        }
        if name == "call_inside_cycle" {
            let state = program
                .machine_states(machine)
                .iter()
                .find(|state| state.name.as_str() == "cycle")
                .expect("cycle");
            let call = program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| match statement {
                    StatementNode::Call(call) => Some(call),
                    _ => None,
                })
                .expect("cycle call");
            let paths = resolver
                .may_write_paths(machine, call)
                .expect("public cycle call");
            assert!(paths.iter().any(|path| path == "item"), "{name}: {paths:?}");
        }
    }
}
