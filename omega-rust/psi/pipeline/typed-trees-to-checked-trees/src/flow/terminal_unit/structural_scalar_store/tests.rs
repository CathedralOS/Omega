use super::super::{ShapeCollector, build_checked_machine, machine_binders, structural_signature};
use super::{assignment_frame_matches, build_structural_scalar_field_store_sequence};
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
    let parameters = program.state_parameters(state);
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
        assignment_frame_matches(program, state, parameters[0].symbol, "$P0", frame),
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
