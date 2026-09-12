use super::*;

fn checked_source(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize tail call");
    let syntax = parse_syntax_trees(&tokens).expect("parse tail call");
    let resolved = lower_syntax_trees(&syntax).expect("resolve tail call");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type tail call");
    lower_typed_trees(typed).expect("check tail call")
}

#[test]
fn named_tail_call_coordinates_match_borrow_and_semantic_traversal() {
    for internal in [false, true] {
        let target = if internal { "finish" } else { "Root::target" };
        let state = if internal {
            "state finish(value: u64) -> u64 { value }"
        } else {
            ""
        };
        let checked = checked_source(&format!(
            "data Root {{}}\n\
             machine Root::argument() -> u64 {{ 0 }}\n\
             machine Root::target(value: u64) -> u64 {{ value }}\n\
             machine Root::entry() -> u64 {{\n\
                 transition {{ _ -> {target}(Root::argument()) }}\n\
                 {state}\n\
             }}",
        ));
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Root::entry")
            .expect("entry machine");
        let state = checked
            .machine_states(machine)
            .first()
            .expect("entry state");
        let operational = validation::infer_operational_may(&checked);
        let state_summary = operational
            .states
            .iter()
            .map(|(_, state)| state)
            .find(|summary| summary.symbol == state.symbol)
            .expect("entry operational state");
        let calls = operational.calls.span_or_empty(state_summary.calls);
        let ordinals = calls
            .iter()
            .map(|call| call.call_ordinal)
            .collect::<Vec<_>>();
        assert_eq!(ordinals, if internal { vec![1] } else { vec![0, 1] });
        assert!(calls.iter().all(|call| call.statement_index == 0));
        assert!(calls.iter().all(|call| {
            call.acknowledgement.origin
                == language_semantics::CallOperationalAcknowledgementOrigin::Source
                && !call.acknowledgement.acknowledges_suspend
                && !call.acknowledgement.acknowledges_block
        }));
        assert!(matches!(
            crate::semantic_calls::find_call_site(&checked, machine.symbol, state.symbol, 0, 0),
            Some(crate::semantic_calls::CallSite::TransitionNamed { .. }),
        ));
        assert!(matches!(
            crate::semantic_calls::find_call_site(&checked, machine.symbol, state.symbol, 0, 1),
            Some(crate::semantic_calls::CallSite::Expression { .. }),
        ));
        let borrow = &checked.facts.borrow;
        let state_borrow = borrow
            .states
            .iter()
            .map(|(_, state)| state)
            .find(|summary| summary.state_symbol == state.symbol)
            .expect("entry borrow state");
        let borrow_calls = borrow.calls.span_or_empty(state_borrow.calls);
        assert_eq!(
            borrow_calls
                .iter()
                .map(|call| call.call_ordinal)
                .collect::<Vec<_>>(),
            [0, 1]
        );
        let argument = calls.last().expect("nested argument call");
        assert_eq!(argument.target_state_symbol, borrow_calls[1].target_symbol);
        assert_eq!(argument.acknowledgement, Default::default());
    }
}

#[test]
fn named_tail_helper_propagates_synchronous_invocation_contract() {
    let checked = checked_source(
        r#"
        pub boundary trait Host { machine ping() reaches Host; }
        pub data Root {}
        machine Root::helper() -> u64 reaches Host { Host::ping(); 0 }
        pub machine Root::entry() -> u64 invokes Host; {
            transition { _ -> Root::helper() }
        }
        "#,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::entry")
        .expect("public tail wrapper");
    let host = checked
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Host")
        .expect("Host service")
        .symbol;
    let invocations = validation::infer_synchronous_invocations(&checked);
    let summary = invocations
        .for_machine(machine.symbol)
        .expect("tail invocation summary");
    assert_eq!(
        summary.effective,
        [flow_effects::InvocationTarget::Service(host)]
    );
    assert_eq!(
        summary.inferred_transitive,
        [flow_effects::InvocationTarget::Service(host)]
    );
}

#[test]
fn named_static_binder_tail_retains_fixed_requirement_reach() {
    for requirement in [
        "where machine Work() -> u64 reaches Audit;",
        "where machine Work satisfies Task::run;",
    ] {
        let checked = checked_source(&format!(
            "boundary trait Audit {{}}\n\
             trait Task {{ machine run() -> u64 reaches Audit; }}\n\
             machine invoke<machine Work>() -> u64\n\
             {requirement}\n\
             {{ transition {{ _ -> Work() }} }}",
        ));
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "invoke")
            .expect("generic tail wrapper");
        let reaches = &checked.facts.service_reaches;
        let audit = reaches
            .services
            .id_for_name("Audit")
            .expect("Audit service");
        let summary = reaches
            .for_machine(machine.symbol)
            .expect("generic reach summary");
        assert_eq!(reaches.rows.services(summary.effective), [audit]);
    }
}

#[test]
fn named_static_binder_tail_checks_signature_arguments() {
    for argument in ["0", "", "true"] {
        let source = format!(
            "machine invoke<machine Work>() -> u64\n\
             where machine Work(value: u64) -> u64;\n\
             {{ transition {{ _ -> Work({argument}) }} }}",
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize binder arguments");
        let syntax = parse_syntax_trees(&tokens).expect("parse binder arguments");
        let resolved = lower_syntax_trees(&syntax).expect("resolve binder arguments");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type binder arguments");
        let result = lower_typed_trees(typed);
        if argument == "0" {
            result.expect("the matching binder argument should check");
        } else {
            let diagnostics =
                result.expect_err("a named binder tail must check its signature arguments");
            if argument.is_empty() {
                assert!(
                    diagnostics.iter().any(|diagnostic| diagnostic
                        .message
                        .contains("expects 1 argument(s), got 0")),
                    "{diagnostics:?}"
                );
            }
        }
    }
}

#[test]
fn named_tail_call_does_not_invent_suspension_acknowledgement() {
    for target in [
        "Root::wait()",
        "(Root::wait())",
        "Root::target(suspend Root::wait())",
    ] {
        let source = r#"
        pub data Root {}
        pub machine Root::wait() -> u64 suspends; { 0 }
        machine Root::target(value: u64) -> u64 { value }
        pub machine Root::entry() -> u64 suspends; {
            transition { _ -> TAIL }
        }
    "#
        .replace("TAIL", target);
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize suspending tail");
        let syntax = parse_syntax_trees(&tokens).expect("parse suspending tail");
        let resolved = lower_syntax_trees(&syntax).expect("resolve suspending tail");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type suspending tail");
        let diagnostics = lower_typed_trees(typed)
            .expect_err("an unmarked named transfer cannot acknowledge a suspending call");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("suspend")),
            "{target}: {diagnostics:?}"
        );
    }
}
