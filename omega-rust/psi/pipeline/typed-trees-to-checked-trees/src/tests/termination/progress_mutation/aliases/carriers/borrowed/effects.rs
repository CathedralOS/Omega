use super::*;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

// These are identity queries, not permission to mutate a parent while its
// saved loan is active. Keep replacement/exposure cases below the borrow check.
fn assert_prefix_effect(inner_access: &str, operation: &str, extra: &str, retained: bool) {
    let source = source(
        "mut ",
        inner_access,
        &format!(
            "let borrowed: &{inner_access}Context = carrier.context;
             let observed: u64 = 0;
             {operation}
             transition {{ _ -> 0 }}"
        ),
        extra,
    )
    .replace(
        "carrier: &mut Carrier)",
        "carrier: &mut Carrier, replacement: &Context)",
    );
    let mut program = typed_source(&source);
    crate::lookup::resolve_projected_receiver_calls(&mut program).unwrap();
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(borrowed) = &statements[0] else {
        panic!("reference local")
    };
    let resolver = validation::CallFrameResolver::new(&program).unwrap();
    let frame = resolver.inferred_state_write_frame(machine, state);
    let origin_before = |statement| {
        resolver.local_reference_origin_before_statement(machine, statement, borrowed.symbol)
    };
    let initial = origin_before(&statements[1]).expect("unexposed input leaf");
    assert_eq!(initial.0, program.state_parameters(state)[0].symbol);
    let [facts::PlaceSegment::Field { symbol }] = initial.1.as_slice() else {
        panic!("one exact reference field: {initial:?}")
    };
    assert_eq!(
        program.symbols.display_path(*symbol, "::"),
        "Carrier::context"
    );
    assert_eq!(
        origin_before(statements.last().unwrap()),
        retained.then_some(initial.clone()),
        "{operation}"
    );
    assert_eq!(
        origin_before(&statements[1]),
        Some(initial),
        "later effects cannot poison an earlier prefix: {operation}"
    );
    assert_eq!(
        resolver.inferred_state_write_frame(machine, state),
        frame,
        "identity queries cannot change cached write frames"
    );
}

#[test]
fn whole_carrier_referent_replacement_retires_only_later_origin_queries() {
    assert_prefix_effect("", "carrier = Carrier { context: replacement };", "", false);
}

#[test]
fn replacing_a_reference_slot_retires_its_frozen_relation() {
    assert_prefix_effect("", "carrier.context = replacement;", "", false);
}

#[test]
fn referent_contents_writes_preserve_reference_identity() {
    for operation in [
        "borrowed.counter = 1;",
        "borrowed.scheduler = SchedulerHandle {};",
        "borrowed = Context { scheduler: SchedulerHandle {}, counter: 1 };",
    ] {
        // Identity survives even when the scheduler qualification does not.
        assert_prefix_effect("mut ", operation, "", true);
    }
}

#[test]
fn explicit_mutable_exposure_of_a_readonly_slot_retires_its_relation() {
    assert_prefix_effect(
        "",
        "_ = inspect_context(&mut carrier.context);",
        "machine inspect_context(context: &mut Context) -> u64 { 0 }",
        false,
    );
}

#[test]
fn empty_mutable_ancestor_methods_retire_frozen_reference_leaves() {
    for operation in ["carrier.touch();", "let ignored: u64 = carrier.touch();"] {
        assert_prefix_effect(
            "",
            operation,
            "machine Carrier::touch(&mut self) -> u64 { 0 }",
            false,
        );
    }
}

#[test]
fn mutable_ancestor_exposure_through_a_carrier_alias_stays_opaque() {
    // This does not require a positive carrier-alias load relation.
    assert_prefix_effect(
        "",
        "let selected: &mut Carrier = carrier; selected.touch();",
        "machine Carrier::touch(&mut self) -> u64 { 0 }",
        false,
    );
}

#[test]
fn earlier_operand_exposure_is_not_exempted_by_an_empty_frame() {
    for (operand, helper, target) in [
        (
            "inspect_context(&mut carrier.context)",
            "machine inspect_context(context: &mut Context) -> u64 { 0 }",
            "inspect_context",
        ),
        (
            "carrier.touch()",
            "machine Carrier::touch(&mut self) -> u64 { 0 }",
            "touch",
        ),
    ] {
        let mut program = typed_source(&source(
            "mut ",
            "",
            &format!(
                "let borrowed: &Context = carrier.context;
                 transition {{ _ -> waiting({operand}, borrowed) }}
                 state waiting(ignored: u64, selected: &Context) -> u64
                 requires selected.scheduler in WeakFair
                 {{ wait_context(selected) }}"
            ),
            helper,
        ));
        crate::lookup::resolve_projected_receiver_calls(&mut program).unwrap();
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "inspect")
            .unwrap();
        let state = &program.machine_states(machine)[0];
        let statements = program.statement_table.statements(state.statement_nodes);
        let StatementNode::LocalData(borrowed) = &statements[0] else {
            panic!("reference local")
        };
        let resolver = validation::CallFrameResolver::new(&program).unwrap();
        let frame = resolver.inferred_state_write_frame(machine, state);
        assert!(
            resolver
                .local_reference_origin_before_statement(
                    machine,
                    statements.last().unwrap(),
                    borrowed.symbol,
                )
                .is_some(),
            "the owning statement has not executed yet"
        );
        let operands = program
            .expression_table
            .iter_expressions()
            .filter_map(|(expression, node)| {
                matches!(node, ExpressionNode::Call(call)
                    if call.target.as_str() == target)
                .then_some(expression)
            })
            .collect::<Vec<_>>();
        assert_eq!(operands.len(), 1, "one exposed operand: {operand}");
        assert!(
            !resolver.expression_reference_bindings_are_stable(machine, operands[0]),
            "{operand}"
        );
        assert_eq!(resolver.inferred_state_write_frame(machine, state), frame);
    }
}

#[test]
fn earlier_operand_writes_distinguish_counter_and_scheduler_qualifications() {
    for (assignment, preserved) in [
        ("context.counter = 1;", true),
        ("context.scheduler = SchedulerHandle {};", false),
    ] {
        let source = source(
            "mut ",
            "mut ",
            "let borrowed: &mut Context = carrier.context;
             transition { _ -> waiting(change(borrowed), borrowed) }
             state waiting(ignored: u64, selected: &Context) -> u64
             requires selected.scheduler in WeakFair
             { wait_context(selected) }",
            &format!("machine change(context: &mut Context) -> u64 {{ {assignment} 0 }}"),
        );
        if preserved {
            let program = check_source(&source);
            let machine = program
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "inspect")
                .unwrap();
            let state = &program.machine_states(machine)[0];
            let plan = program
                .facts
                .termination
                .for_machine(machine.symbol)
                .unwrap();
            let TerminationGuarantee::Terminates { premises } = &plan.checked_summary else {
                panic!("a counter write preserves the scheduler premise")
            };
            let [premise] = premises.as_slice() else {
                panic!("one exact input premise: {premises:?}")
            };
            assert_eq!(
                premise.subject.root,
                program.state_parameters(state)[0].symbol
            );
            assert_eq!(
                premise
                    .subject
                    .projections
                    .iter()
                    .map(|field| program.symbols.display_path(*field, "::"))
                    .collect::<Vec<_>>(),
                ["Carrier::context", "Context::scheduler"]
            );
        } else {
            let diagnostics = lower_typed_trees(typed_source(&source))
                .expect_err("an earlier scheduler write retires the qualification");
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("cannot prove requires contract for call waiting")),
                "{diagnostics:#?}"
            );
        }
    }
}
