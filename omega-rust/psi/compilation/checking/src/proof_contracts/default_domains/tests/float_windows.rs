use super::validate_default_domain_writes;

fn window_diagnostics(statements: &str, zero_gated: bool) -> Vec<diagnostics::Diagnostic> {
    let mut program = crate::front_end::typed_program(&format!(
        "data Sample where value >= 0.0 && value <= 1.0 {{ value: f32; other: f32; }}
         machine update(sample: &mut Sample) {{ {statements} }}"
    ));
    crate::land_float_literal_destinations(&mut program);
    // Exercise both conservatively gated typed input and the refined checked
    // input: temporary write debt applies even when zero establishes a type.
    let definitions = program.roots.data_definitions;
    program
        .tables
        .data_definitions
        .span_mut_or_empty(definitions)[0]
        .zero_gated = zero_gated;
    let mut diagnostics = Vec::new();
    validate_default_domain_writes(&program, &mut diagnostics);
    diagnostics
}

#[test]
fn float_window_repairs_exact_place_before_consumption() {
    for zero_gated in [true, false] {
        for statements in [
            "sample.value = 2.0; sample.value = 0.5;",
            "sample.value = 0.0f32 / 0.0; sample.value = 0.5;",
            "sample.value = 1.0e-50;",
        ] {
            let diagnostics = window_diagnostics(statements, zero_gated);
            assert!(diagnostics.is_empty(), "{statements}\n{diagnostics:#?}");
        }
        for statements in [
            "sample.value = 2.0;",
            "sample.value = 0.0f32 / 0.0;",
            "sample.value = 2.0; sample.other = 0.5;",
            "sample.value = 2.0; let exposed: &Sample = sample; sample.value = 0.5;",
            "sample.value = 2.0; transition { _ -> next(sample) } state next(sample: &Sample) {}",
        ] {
            let diagnostics = window_diagnostics(statements, zero_gated);
            assert!(
                !diagnostics.is_empty(),
                "accepted open window: {statements}"
            );
        }
    }
    let diagnostics = window_diagnostics(
        "sample.value = 2.0; let raw: f32 = sample.value; sample.value = 0.5;",
        false,
    );
    assert!(
        diagnostics.is_empty(),
        "raw reads do not assume the record predicate: {diagnostics:#?}"
    );
}

#[test]
fn ieee_self_equality_is_not_an_integer_symbolic_identity() {
    let mut program = crate::front_end::typed_program(
        "data Sample where value == value { value: f32; }
         machine update(sample: &mut Sample, incoming: f32) { sample.value = incoming; }",
    );
    crate::land_float_literal_destinations(&mut program);
    let mut diagnostics = Vec::new();
    validate_default_domain_writes(&program, &mut diagnostics);
    assert!(!diagnostics.is_empty(), "unknown float could be NaN");
}

#[test]
fn transported_float_literals_meet_by_format_and_bits() {
    use super::super::state_flow::meet_valuations;
    use super::super::symbolic_values::LiteralValue;
    let valuation = |value| {
        vec![(
            "self.sample".to_owned(),
            vec![("value".to_owned(), Some(value))],
            None,
        )]
    };
    let nan = valuation(LiteralValue::Float32(0x7fc00001));
    assert_eq!(meet_valuations(&nan, &nan), nan);
    let zero = valuation(LiteralValue::Float32(0));
    for other in [
        LiteralValue::Float32(0x80000000),
        LiteralValue::Float64(0),
        LiteralValue::Integer(0),
    ] {
        assert!(meet_valuations(&zero, &valuation(other))[0].1.is_empty());
    }
}

#[test]
fn nested_whole_values_cannot_escape_but_raw_fields_remain_readable() {
    for (statements, accepted) in [
        (
            "let raw: f32 = sample.child.value; sample.child.value = 0.5;",
            true,
        ),
        (
            "let exposed: &Sample = &sample.child; sample.child.value = 0.5;",
            false,
        ),
        (
            "transition { _ -> next(&sample.child) } state next(child: &Sample) {}",
            false,
        ),
    ] {
        let mut program = crate::front_end::typed_program(&format!(
            "data Sample where value >= 0.0 && value <= 1.0 {{ value: f32; }}
             data Envelope {{ child: Sample; }}
             machine update(sample: &mut Envelope) {{ sample.child.value = 2.0; {statements} }}"
        ));
        crate::land_float_literal_destinations(&mut program);
        let definitions = program.roots.data_definitions;
        for definition in program
            .tables
            .data_definitions
            .span_mut_or_empty(definitions)
        {
            definition.zero_gated = false;
        }
        let mut diagnostics = Vec::new();
        validate_default_domain_writes(&program, &mut diagnostics);
        assert_eq!(
            diagnostics.is_empty(),
            accepted,
            "{statements}\n{diagnostics:#?}"
        );
    }
}

#[test]
fn indexed_whole_value_transfer_cannot_escape_a_known_element_window() {
    let mut program = crate::front_end::typed_program(
        "data Sample where value >= 0.0 && value <= 1.0 { value: f32; }
         data Envelope { children: [Sample; 2]; }
         machine update(sample: &mut Envelope, index: u32) {
             sample.children[0].value = 2.0;
             transition { _ -> next(&sample.children[index]) }
             state next(child: &Sample) {}
         }",
    );
    crate::land_float_literal_destinations(&mut program);
    let definitions = program.roots.data_definitions;
    for definition in program
        .tables
        .data_definitions
        .span_mut_or_empty(definitions)
    {
        definition.zero_gated = false;
    }
    let mut diagnostics = Vec::new();
    validate_default_domain_writes(&program, &mut diagnostics);
    assert!(
        !diagnostics.is_empty(),
        "dynamic selector may consume the invalid element"
    );
}

#[test]
fn poisoned_boot_places_do_not_resurrect_zero_cofields() {
    use super::super::assignment_windows::handle_assignment;
    let mut program = crate::front_end::typed_program(
        "data Pair where left <= right { left: f32; right: f32; }
         data Main { pair: Pair; }
         machine Main::main(&mut self) { self.pair.right = 0.0; }",
    );
    crate::land_float_literal_destinations(&mut program);
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let typed_trees::statement::StatementNode::Assignment(assignment) =
        &program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("field assignment");
    };
    for (poisoned_all, poisoned_paths) in [
        (true, Vec::new()),
        (false, vec!["self.pair.left".to_owned()]),
    ] {
        let mut tracked = Vec::new();
        let mut diagnostics = Vec::new();
        handle_assignment(
            &program,
            machine,
            state,
            assignment.target,
            assignment.value,
            &mut tracked,
            &[],
            poisoned_all,
            &poisoned_paths,
            true,
            &mut diagnostics,
        );
        assert_eq!(tracked.len(), 1);
        assert!(
            !tracked[0].born_zero,
            "poisoned storage has an unknown prior value"
        );
        assert!(
            tracked[0].window_open,
            "unknown left cannot prove left <= zero"
        );
    }
}

#[test]
fn wildcard_field_invalidation_keeps_an_unknown_tombstone() {
    use super::super::assignment_windows::handle_assignment;
    use super::super::symbolic_values::LiteralValue;
    let mut program = crate::front_end::typed_program(
        "data Pair where left <= right { left: f32; right: f32; }
         data Main { pairs: [Pair; 2]; }
         machine Main::main(&mut self, index: u32) { self.pairs[index].left = 0.5; }",
    );
    crate::land_float_literal_destinations(&mut program);
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let typed_trees::statement::StatementNode::Assignment(assignment) =
        &program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("field assignment");
    };
    let mut tracked = vec![super::super::TrackedPlace {
        spelling: "self.pairs[0]".to_owned(),
        definition: &program.data_definitions()[0],
        fields: vec![(
            "left".to_owned(),
            Some(LiteralValue::Float32(1.0f32.to_bits())),
        )],
        symbols: Vec::new(),
        measures: Vec::new(),
        established: true,
        born_zero: true,
        window_open: false,
        active_case: None,
    }];
    let mut diagnostics = Vec::new();
    handle_assignment(
        &program,
        machine,
        state,
        assignment.target,
        assignment.value,
        &mut tracked,
        &[],
        false,
        &[],
        true,
        &mut diagnostics,
    );
    let concrete = tracked
        .iter()
        .find(|place| place.spelling == "self.pairs[0]")
        .unwrap();
    assert_eq!(concrete.fields, vec![("left".to_owned(), None)]);
}
