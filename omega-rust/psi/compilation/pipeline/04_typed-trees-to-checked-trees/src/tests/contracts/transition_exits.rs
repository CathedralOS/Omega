use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

#[test]
fn ordinary_empty_transition_arms_owe_machine_postconditions() {
    for body in [
        "transition { _ -> {} }",
        "transition self.flag { true -> {} false -> {} }",
    ] {
        let source = format!(
            "data Main {{ flag: bool; }} machine Main::run(&mut self) ensures false {{ {body} }}"
        );
        let diagnostics =
            lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
                .expect_err("ordinary Unit exit owes ensures");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn crash_and_named_dispatch_do_not_create_fallthrough_exits() {
    let checked = lower_typed_trees(
        parse_typed_trees(
            r#"
        machine fail()
        crashes Abort
        ensures false
        { crash Abort; }
        "#,
        ),
        &CheckingRequest::settled(),
    )
    .expect("crash does not return ordinarily");
    assert_eq!(checked.facts.proof.contract_exits.len(), 0);

    let checked = lower_typed_trees(
        parse_typed_trees(
            r#"
        data Main {}
        machine Main::run(&mut self)
        ensures true
        {
            transition { _ -> done() }
            state done(&mut self) {}
        }
        "#,
        ),
        &CheckingRequest::settled(),
    )
    .expect("the final named state, not its incoming jump, returns");
    let exits = checked
        .facts
        .proof
        .contract_exits
        .iter()
        .map(|(_, exit)| exit)
        .collect::<Vec<_>>();
    assert_eq!(exits.len(), 1);
    assert!(!exits[0].transition_target.is_valid());
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("machine");
    let done = checked
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str() == "done")
        .expect("done");
    assert_eq!(exits[0].state_symbol, done.symbol);
}

#[test]
fn returning_arm_guards_prove_only_their_own_truth_value() {
    for (ensures, accepted) in [("self.flag", true), ("self.flag == false", false)] {
        let source = format!(
            r#"
            data Main {{ flag: bool; }}
            machine Main::run(&mut self) ensures {ensures}
            {{
                transition self.flag {{ true -> {{}} false -> fail() }}
                state fail(&mut self) {{ crash Abort; }}
            }}
        "#
        );
        let result = lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled());
        assert_eq!(result.is_ok(), accepted, "{ensures}: {result:#?}");
    }
    let source = r#"
        data Main { flag: bool; }
        machine Main::run(&mut self) ensures self.flag == false {
            transition self.flag { true -> fail() false -> {} }
            state fail(&mut self) { crash Abort; }
        }
    "#;
    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("false arm retains negative guard evidence");
}

#[test]
fn loop_reference_origins_retain_unknown_incoming_alternatives() {
    for (declaration, argument) in [
        ("let mut local: [u8; 4] = [0, 0, 0, 0];", "&mut local"),
        ("let mut alias: &mut [u8; 4] = destination;", "alias"),
    ] {
        let source = format!(
            r#"
            domain [u8; 4]::Utf8 requires valid_utf8(self);
            machine fill(out_line: &mut [u8; 4], repeat: bool)
            ensures out_line in Utf8 {{
                transition {{ _ -> visit(out_line, repeat) }}
                state visit(destination: &mut [u8; 4], repeat: bool) {{
                    transition repeat {{ true -> again(destination, repeat) false -> write(destination) }}
                }}
                state again(destination: &mut [u8; 4], repeat: bool) {{
                    {declaration}
                    transition {{ _ -> visit({argument}, repeat) }}
                }}
                state write(result: &mut [u8; 4]) {{ result = "ok"; }}
            }}
        "#
        );
        let diagnostics =
            lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
                .expect_err("the direct entry edge cannot erase an unknown loop-carried origin");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("no exact incoming reference origin for out_line")),
            "{declaration}: {diagnostics:#?}"
        );
    }
}

#[test]
fn unreachable_predecessors_do_not_contaminate_reference_origins() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        machine fill(out_line: &mut [u8; 4]) ensures out_line in Utf8 {
            transition { _ -> write(out_line) }
            state detached(unrelated: &mut [u8; 4], repeat: bool) {
                transition repeat { true -> detached(unrelated, repeat) false -> write(unrelated) }
            }
            state write(destination: &mut [u8; 4]) { destination = "ok"; }
        }
    "#;
    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("an entry-unreachable component supplies no runtime incoming reference");
}

#[test]
fn named_reference_origin_requires_consistent_whole_name_symbols() {
    use typed_trees::expression::ExpressionNode;
    use typed_trees::statement::{StatementNode, TransitionTargetNode};
    let original = parse_typed_trees(
        r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        machine fill(out_line: &mut [u8; 4], other: &mut [u8; 4])
        ensures out_line in Utf8 {
            transition { _ -> write(out_line) }
            state write(destination: &mut [u8; 4]) { destination = "ok"; }
        }
    "#,
    );
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "fill")
        .expect("fill");
    let state = &original.machine_states(machine)[0];
    let StatementNode::Transition(transition) =
        &original.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("transition");
    };
    let TransitionTargetNode::Named { arguments, .. } = original
        .statement_table
        .transition_target(transition.target)
    else {
        panic!("named target");
    };
    let expression = original.statement_table.expression_handles(*arguments)[0];
    let exact = original.state_parameters(state)[0].symbol;
    let foreign = original.state_parameters(state)[1].symbol;
    for (name, symbol, accepted) in [
        ("exact", exact, true),
        ("foreign_final", foreign, false),
        ("missing_final", symbols::SymbolHandle::invalid(), false),
        (
            "stale_final",
            symbols::SymbolHandle::from_parts(exact.arena_index(), exact.generation() + 1),
            false,
        ),
    ] {
        let mut program = original.clone();
        let ExpressionNode::Name(path) = program.expression_table.expression_mut(expression) else {
            panic!("argument");
        };
        path.symbol = symbol;
        let result = lower_typed_trees(program, &CheckingRequest::settled());
        assert_eq!(result.is_ok(), accepted, "{name}: {result:#?}");
    }
}

#[test]
fn crash_exits_cannot_carry_ordinary_edge_obligations() {
    use typed_trees::statement::{
        StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetHandle,
    };

    fn transitions_mut<'a>(
        program: &'a mut typed_trees::TypedTrees,
        machine_name: &str,
        state_ordinal: usize,
    ) -> &'a mut [StatementNode] {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
            .expect("machine");
        let span = program.machine_states(machine)[state_ordinal].statement_nodes;
        program.statement_table.statements_mut(span)
    }

    fn expect_isolation_rejection(program: typed_trees::TypedTrees, edge: &str, context: &str) {
        let diagnostics = lower_typed_trees(program, &CheckingRequest::settled())
            .expect_err("a crash exit cannot keep ordinary edge obligations");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(edge)),
            "{context}\n{diagnostics:#?}"
        );
    }

    // Control: the same edge kept Ordinary owes `ensures` and rejects.
    let ordinary =
        parse_typed_trees("machine m() -> bool ensures false { transition { _ -> true } }");
    lower_typed_trees(ordinary, &CheckingRequest::settled())
        .expect_err("an ordinary value return cannot evade ensures");

    // Re-labeling that value edge as a crash keeps the return target while
    // contract exits and ensures attach only to `Ordinary` exits; covered by
    // `crashes Trap`, the crash site would silently absorb the obligation.
    for source in [
        "machine m() -> bool crashes Trap ensures false { transition { _ -> true } }",
        "machine m() -> bool crashes Trap ensures false { transition { _ -> done() } \
         state done() -> bool { true } }",
    ] {
        let mut program = parse_typed_trees(source);
        for statement in transitions_mut(&mut program, "m", 0) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            transition.exit = TransitionExit::Crash(typed_trees::signature::CrashCause::Trap);
        }
        expect_isolation_rejection(program, "an ordinary successor edge", source);
    }

    // A continuation on a crash transition is an ordinary fallthrough edge
    // that no contract exit will ever name.
    let mut program = parse_typed_trees(
        "machine m() -> bool crashes Trap { transition { _ -> done() } \
         state done() -> bool { crash Trap; } }",
    );
    let named_target = {
        let mut found = TransitionTargetHandle::invalid();
        for statement in program.statement_table.statements(
            program.machine_states(program.machines().iter().next().expect("m"))[0].statement_nodes,
        ) {
            if let StatementNode::Transition(transition) = statement {
                found = transition.target;
            }
        }
        found
    };
    for statement in transitions_mut(&mut program, "m", 1) {
        let StatementNode::Transition(transition) = statement else {
            continue;
        };
        transition.continuation = named_target;
    }
    expect_isolation_rejection(
        program,
        "a continuation edge",
        "crash transition with a forged continuation",
    );

    // A `when` guard on a crash transition would masquerade as a conditional
    // arm while its site is recorded unconditionally. A literal subject keeps
    // the forged guard inside the state's declared scope, isolating the edge
    // check from ordinary name resolution.
    let mut program = parse_typed_trees("machine m() -> bool crashes Trap { crash Trap; }");
    let guard = program
        .expression_table
        .insert(typed_trees::expression::ExpressionNode::Boolean(true));
    for statement in transitions_mut(&mut program, "m", 0) {
        let StatementNode::Transition(transition) = statement else {
            continue;
        };
        transition.guard = TransitionGuardNode::When(guard);
    }
    expect_isolation_rejection(
        program,
        "a conditional `when` guard",
        "crash transition with a forged guard",
    );
}
