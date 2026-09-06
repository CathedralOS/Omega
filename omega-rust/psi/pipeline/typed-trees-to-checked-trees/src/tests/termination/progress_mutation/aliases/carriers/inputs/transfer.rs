use super::*;

fn assert_no_subject(program: &checked_trees::CheckedTrees) {
    let plan = program
        .facts
        .termination
        .for_machine(symbol_of_checked(program, "replace"))
        .unwrap();
    assert_eq!(plan.checked_summary, TerminationGuarantee::NoGuarantee);
}

#[test]
fn replacing_an_input_reference_slot_or_carrier_retires_the_old_relation() {
    for operation in [
        "carrier.context = &replacement;",
        "carrier = Carrier { context: &replacement };",
    ] {
        let program = check_source(&fixture_source(
            "",
            &format!(
                "let saved: Carrier = carrier;
             {operation}
             let borrowed: &Context = saved.context;
             transition {{ _ -> wait_context(borrowed) }}"
            ),
            "",
        ));
        assert_no_subject(&program);
    }
}

#[test]
fn a_live_input_reference_uses_the_scheduler_replacement_subject() {
    let program = check_source(&fixture_source(
        "mut ",
        "let borrowed: &mut Context = carrier.context;
         borrowed.scheduler = replacement.scheduler;
         transition { _ -> wait_context(borrowed) }",
        "",
    ));
    assert_subjects(&program, &["replacement"]);
}

#[test]
fn rebinding_a_copy_does_not_redirect_an_earlier_input_reference_copy() {
    let program = check_source(&fixture_source(
        "",
        "let mut borrowed: &Context = carrier.context;
         let prior: &Context = borrowed;
         borrowed = &replacement;
         transition { _ -> wait_context(prior) }",
        "",
    ));
    assert_input_subject(&program);
}

#[test]
fn helper_results_preserve_the_actual_input_reference_boundary() {
    for body in [
        "let borrowed: &Context = forward(carrier.context);",
        "let saved: Carrier = identity(carrier); let borrowed: &Context = saved.context;",
        "let saved: Carrier = wrap(carrier.context); let borrowed: &Context = saved.context;",
    ] {
        let program = check_source(&fixture_source(
            "",
            &format!("{body} transition {{ _ -> wait_context(borrowed) }}"),
            "machine forward(context: &Context) -> &Context { context }
             machine identity(carrier: Carrier) -> Carrier { carrier }
             machine wrap(context: &Context) -> Carrier { Carrier { context: context } }",
        ));
        assert_input_subject(&program);
    }
}

#[test]
fn an_input_reference_does_not_prove_a_missing_qualification() {
    let source = fixture_source(
        "",
        "let borrowed: &Context = carrier.context;
         transition { _ -> waiting(borrowed) }
         state waiting(selected: &Context) -> u64
         requires selected.scheduler in WeakFair
         { wait_context(selected) }",
        "",
    );
    assert_input_subject(&check_source(&source));
    let missing = source.replace("requires carrier.context.scheduler in WeakFair", "");
    let tokens = Lexer::new(&missing).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let diagnostics = lower_typed_trees(typed).expect_err("identity cannot mint a qualification");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove requires contract for call waiting")),
        "{diagnostics:#?}"
    );
}

#[test]
fn a_published_input_reference_premise_retains_its_exact_projection() {
    let source = fixture_source(
        "",
        "let borrowed: &Context = carrier.context;
         transition { _ -> wait_context(borrowed) }",
        "",
    )
    .replace("data Carrier", "pub data Carrier")
    .replace("machine replace", "pub machine replace")
    .replace("requires replacement.scheduler in WeakFair", "terminates;");
    let program = check_source(&source);
    assert_input_subject(&program);
    let plan = program
        .facts
        .termination
        .for_machine(symbol_of_checked(&program, "replace"))
        .unwrap();
    let language_semantics::TerminationInterface::Published(summary) = &plan.interface else {
        panic!("published progress contract")
    };
    assert_eq!(summary, &plan.checked_summary);
    assert!(plan.implementation_witness.is_none());
}

#[test]
fn earlier_operand_writes_must_preserve_the_input_referents_qualification() {
    for (assignment, preserved) in [
        ("context.counter = 1;", true),
        ("context.scheduler = SchedulerHandle {};", false),
    ] {
        let source = fixture_source(
            "mut ",
            "let borrowed: &mut Context = carrier.context;
             transition { _ -> waiting(change(borrowed), borrowed) }
             state waiting(ignored: u64, selected: &Context) -> u64
             requires selected.scheduler in WeakFair
             { wait_context(selected) }",
            &format!("machine change(context: &mut Context) -> u64 {{ {assignment} 0 }}"),
        );
        if preserved {
            assert_input_subject(&check_source(&source));
        } else {
            let typed = super::exposure::typed_source(&source);
            let Err(diagnostics) = lower_typed_trees(typed) else {
                panic!("overlapping write must retire the old qualification");
            };
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("cannot prove requires contract for call waiting")),
                "{diagnostics:#?}"
            );
        }
    }
}
