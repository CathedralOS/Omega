use super::super::{ShapeCollector, build_checked_machine, machine_binders, structural_signature};
use super::{build_structural_scalar_field_store_sequence, frame};
use checked_trees::{CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan};

#[test]
fn structural_entry_field_write_retains_its_ordered_unit_plan() {
    let source = r#"
        data Flag { enabled: bool; }
        data Helper {}
        boundary trait Sink { machine record(value: bool); }
        machine trigger() -> bool crashes Trap { crash Trap; }
        machine Helper::forward(record: &mut Flag)
        requires record.enabled
        crashes Trap record.enabled
        { record.enabled = false; Sink::record(trigger()); }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = crate::lower_typed_trees(typed).unwrap();
    let program = &checked.typed;
    let facts = &checked.facts;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("forward"))
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let mut shapes = ShapeCollector::new(program);
    let (_, structural_parameters) = structural_signature(
        program,
        &mut shapes,
        machine,
        state,
        &machine_binders(program, machine),
        true,
    )
    .expect("mutable record signature");
    let frame = &facts
        .mutation
        .for_machine(machine.symbol)
        .unwrap()
        .state_write_frames
        .iter()
        .find(|frame| frame.state == state.symbol)
        .unwrap()
        .frame;
    assert!(
        frame::matches(program, machine, state, frame),
        "assignment frame does not match exact authored record.enabled store: {frame:?}"
    );
    assert!(
        facts
            .values
            .scalar_expressions
            .expression_at(
                state.symbol,
                0,
                CheckedScalarExpressionRole::AssignmentValue
            )
            .is_some(),
        "checked literal assignment value"
    );
    let stores = build_structural_scalar_field_store_sequence(
        program,
        facts,
        machine,
        state,
        &structural_parameters,
        &[],
        0,
    )
    .expect("exact ordered scalar field store sequence");
    assert_eq!(stores.len(), 1);
    let plan = build_checked_machine(program, facts, &mut shapes, machine, &[], &[])
        .expect("store plus crashing scalar argument call retains Unit machine plan");
    assert!(matches!(
        plan.operations.first(),
        Some(CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(
            _
        ))
    ));
}

#[test]
fn ordered_stores_replay_successor_writes_and_reject_modified_frames() {
    let source = r#"
        data Flags { first: bool; second: bool; }
        machine Flags::run(&mut self) {
            self.first = false;
            transition { _ -> update() }
            state update(&mut self) {
                self.second = true;
                transition self.first { true -> run() _ -> done() }
            }
            state done(&mut self) {}
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = crate::lower_typed_trees(typed).unwrap();
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Flags::run")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let mut shapes = ShapeCollector::new(program);
    let (_, parameters) =
        structural_signature(program, &mut shapes, machine, state, &[], true).unwrap();
    let frame = &checked
        .facts
        .mutation
        .for_machine(machine.symbol)
        .unwrap()
        .state_write_frames[0]
        .frame;
    assert_eq!(
        frame.complete_paths().unwrap(),
        ["self.first", "self.second"]
    );
    let stores = build_structural_scalar_field_store_sequence(
        program,
        &checked.facts,
        machine,
        state,
        &parameters,
        &[],
        0,
    )
    .expect("entry store retains the complete successor frame");
    assert_eq!(
        stores.len(),
        1,
        "only this state's local assignment is emitted"
    );
    for replacement in [
        facts::NormalizedWriteFrame::complete(vec!["self.first".into()]),
        facts::NormalizedWriteFrame::complete(vec![
            "self.first".into(),
            "self.second".into(),
            "self.absent".into(),
        ]),
        facts::NormalizedWriteFrame::opaque(),
    ] {
        let mut changed = checked.facts.clone();
        changed
            .mutation
            .machines
            .iter_mut()
            .find(|fact| fact.machine == machine.symbol)
            .unwrap()
            .state_write_frames[0]
            .frame = replacement;
        assert!(
            build_structural_scalar_field_store_sequence(
                program,
                &changed,
                machine,
                state,
                &parameters,
                &[],
                0,
            )
            .is_none(),
            "missing, extra, or opaque successor writes cannot authorize stores"
        );
    }
}
