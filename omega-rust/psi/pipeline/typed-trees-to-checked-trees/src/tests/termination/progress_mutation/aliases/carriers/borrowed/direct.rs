use super::*;

mod adversarial;
mod selectors;

fn direct_source(access: &str, body: &str) -> String {
    source(
        access,
        "",
        "let borrowed: &Context = select(carrier);
         transition { _ -> wait_context(borrowed) }",
        &format!("machine select(carrier: &{access}Carrier) -> &Context {{ {body} }}"),
    )
}

#[test]
fn a_direct_helper_result_keeps_the_borrowed_input_subject() {
    for access in ["", "mut "] {
        for body in [
            "carrier.context",
            "let borrowed: &Context = carrier.context; borrowed",
            "let saved: Carrier = Carrier { context: carrier.context }; saved.context",
            "transition { _ -> carrier.context }",
        ] {
            assert_input_premise(&check_source(&direct_source(access, body)));
        }
    }
}

#[test]
fn a_direct_result_query_keeps_the_exact_input_leaf() {
    let program = typed_source(&direct_source("", "carrier.context"));
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let statements = program.statement_table.statements(state.statement_nodes);
    let typed_trees::statement::StatementNode::LocalData(borrowed) = &statements[0] else {
        panic!("reference local")
    };
    let resolver = validation::CallFrameResolver::new(&program).unwrap();
    let frame = resolver.inferred_state_write_frame(machine, state);
    let (root, segments) = resolver
        .local_reference_origin_before_statement(
            machine,
            statements.last().unwrap(),
            borrowed.symbol,
        )
        .expect("a direct result transports the checked loaded leaf");
    assert_eq!(root, program.state_parameters(state)[0].symbol);
    let [facts::PlaceSegment::Field { symbol }] = segments.as_slice() else {
        panic!("one exact field: {segments:?}")
    };
    assert_eq!(
        program.symbols.display_path(*symbol, "::"),
        "Carrier::context"
    );
    assert_eq!(resolver.inferred_state_write_frame(machine, state), frame);
}

#[test]
fn a_direct_result_can_load_an_owned_input_carriers_reference() {
    let source =
        direct_source("", "carrier.context").replace("carrier: &Carrier)", "carrier: Carrier)");
    assert_input_premise(&check_source(&source));
}

#[test]
fn nested_direct_results_preserve_the_same_loaded_subject() {
    let source = direct_source("", "inner(carrier)");
    assert_input_premise(&check_source(&format!(
        "{source} machine inner(carrier: &Carrier) -> &Context {{ carrier.context }}"
    )));
}

#[test]
fn direct_exclusive_results_keep_identity_separate_from_helper_writes() {
    for (assignment, preserved) in [
        ("", true),
        ("carrier.context.counter = 1;", true),
        ("carrier.context.scheduler = SchedulerHandle {};", false),
    ] {
        let source = source(
            "mut ",
            "mut ",
            "let borrowed: &mut Context = select(carrier);
             transition { _ -> waiting(borrowed) }
             state waiting(selected: &Context) -> u64
             requires selected.scheduler in WeakFair
             { wait_context(selected) }",
            &format!(
                "machine select(carrier: &mut Carrier) -> &mut Context {{ {assignment}
                 carrier.context }}"
            ),
        );
        if preserved {
            assert_input_premise(&check_source(&source));
        } else {
            let diagnostics = lower_typed_trees(typed_source(&source))
                .expect_err("returning the reference cannot restore a changed qualification");
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("cannot prove requires contract for call waiting")),
                "{diagnostics:#?}"
            );
        }
    }
}

#[test]
fn raw_exclusive_member_results_forward_the_declared_reference_type() {
    let source = source(
        "mut ",
        "mut ",
        "let borrowed: &mut Context = select(carrier); transition { _ -> 0 }",
        "machine select(carrier: &mut Carrier) -> &mut Context { carrier.context }",
    );
    let program = typed_source(&source);
    adversarial::assert_identity(&program, "borrowed", true);
    lower_typed_trees(program).expect("an exact reference field is already a reference value");
}
