use super::*;

mod adversarial;

fn result_source(outer_access: &str, body: &str) -> String {
    source(
        outer_access,
        "",
        "let saved: Carrier = rebuild(carrier);
         let borrowed: &Context = saved.context;
         transition { _ -> wait_context(borrowed) }",
        &format!("machine rebuild(carrier: &{outer_access}Carrier) -> Carrier {{ {body} }}"),
    )
}

#[test]
fn a_reconstructed_borrowed_input_result_retains_its_reference_subject() {
    for access in ["", "mut "] {
        for body in [
            "Carrier { context: carrier.context }",
            "let borrowed: &Context = carrier.context; Carrier { context: borrowed }",
            "let saved: Carrier = Carrier { context: carrier.context }; saved",
            "transition { _ -> Carrier { context: carrier.context } }",
        ] {
            assert_input_premise(&check_source(&result_source(access, body)));
        }
    }
}

#[test]
fn a_nested_result_reuses_the_borrowed_input_relation() {
    let source = result_source("", "reconstruct(carrier)");
    assert_input_premise(&check_source(&format!(
        "{source}
         machine reconstruct(carrier: &Carrier) -> Carrier {{
             Carrier {{ context: carrier.context }}
         }}"
    )));
}

#[test]
fn a_reconstructed_exclusive_leaf_retains_its_subject() {
    let source = source(
        "mut ",
        "mut ",
        "let saved: Carrier = rebuild(carrier);
         let borrowed: &mut Context = saved.context;
         transition { _ -> wait_context(borrowed) }",
        "machine rebuild(carrier: &mut Carrier) -> Carrier {
            Carrier { context: carrier.context }
         }",
    );
    assert_input_premise(&check_source(&source));
}

#[test]
fn reconstructed_results_do_not_restore_qualifications_retired_by_helper_writes() {
    for (assignment, retained) in [
        ("carrier.context.counter = 1;", true),
        ("carrier.context.scheduler = SchedulerHandle {};", false),
    ] {
        let source = source(
            "mut ",
            "mut ",
            "let saved: Carrier = rebuild(carrier);
             let borrowed: &mut Context = saved.context;
             transition { _ -> waiting(borrowed) }
             state waiting(selected: &Context) -> u64
             requires selected.scheduler in WeakFair
             { wait_context(selected) }",
            &format!(
                "machine rebuild(carrier: &mut Carrier) -> Carrier {{
                {assignment} Carrier {{ context: carrier.context }}
             }}"
            ),
        );
        if retained {
            assert_input_premise(&check_source(&source));
        } else {
            let diagnostics = lower_typed_trees(typed_source(&source))
                .expect_err("returning a reference cannot restore its scheduler qualification");
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
fn caller_substitution_resolves_the_selected_frozen_leaf() {
    let source = result_source("", "Carrier { context: carrier.second }")
        .replace(
            "data Carrier { context: &Context; }",
            "data Carrier { context: &Context; } data Pair { first: &Context; second: &Context; }",
        )
        .replace(
            "machine inspect(carrier: &Carrier)",
            "machine inspect(carrier: &Pair)",
        )
        .replace(
            "requires carrier.context.scheduler in WeakFair",
            "requires carrier.second.scheduler in WeakFair",
        )
        .replace(
            "machine rebuild(carrier: &Carrier)",
            "machine rebuild(carrier: &Pair)",
        );
    let program = check_source(&source);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    let plan = program
        .facts
        .termination
        .for_machine(machine.symbol)
        .unwrap();
    let TerminationGuarantee::Terminates { premises } = &plan.checked_summary else {
        panic!(
            "one selected input qualification: {:?}",
            plan.checked_summary
        )
    };
    let [premise] = premises.as_slice() else {
        panic!("one premise: {premises:?}")
    };
    assert_eq!(
        premise.subject.root,
        program.state_parameters(&program.machine_states(machine)[0])[0].symbol
    );
    assert_eq!(
        premise
            .subject
            .projections
            .iter()
            .map(|field| program.symbols.display_path(*field, "::"))
            .collect::<Vec<_>>(),
        ["Pair::second", "Context::scheduler"]
    );
}

#[test]
fn a_local_actual_uses_its_captured_source_after_reconstruction() {
    let program = fixture_with_body(
        "let original: Carrier = Carrier { context: &context };
         let returned: Carrier = reconstruct(&original);
         let borrowed: &Context = returned.context;
         transition { _ -> wait_context(borrowed) }",
        true, false,
        "data Carrier { context: &Context; }
         machine reconstruct(carrier: &Carrier) -> Carrier { Carrier { context: carrier.context } }",
    );
    assert_subjects(&program, &["context"]);
}

#[test]
fn a_reconstructed_result_query_retains_the_input_leaf_without_changing_frames() {
    let program = typed_source(&result_source("", "Carrier { context: carrier.context }"));
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let statements = program.statement_table.statements(state.statement_nodes);
    let typed_trees::statement::StatementNode::LocalData(borrowed) = &statements[1] else {
        panic!("the selected reference local")
    };
    let resolver = validation::CallFrameResolver::new(&program).unwrap();
    let frame = resolver.inferred_state_write_frame(machine, state);
    let (root, segments) = resolver
        .local_reference_origin_before_statement(
            machine,
            statements.last().unwrap(),
            borrowed.symbol,
        )
        .expect("the helper preserves one frozen input leaf");
    assert_eq!(root, program.state_parameters(state)[0].symbol);
    let [facts::PlaceSegment::Field { symbol }] = segments.as_slice() else {
        panic!("one exact reference field: {segments:?}")
    };
    assert_eq!(
        program.symbols.display_path(*symbol, "::"),
        "Carrier::context"
    );
    assert_eq!(resolver.inferred_state_write_frame(machine, state), frame);
}
