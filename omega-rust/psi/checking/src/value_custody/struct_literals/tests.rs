use super::{
    DataMember, Diagnostic, ExpressionNode, StatementNode, guard_bounds,
    validate_struct_literal_fields,
};
fn construction_diagnostics(source: &str) -> Vec<Diagnostic> {
    let program = crate::front_end::typed_program(source);
    let mut diagnostics = Vec::new();
    validate_struct_literal_fields(&program, &mut diagnostics);
    diagnostics
}

#[test]
fn a_where_bounded_construction_reads_requires_and_guarded_arithmetic() {
    let source = "data Pair where left <= 5 { left: u32; }
         machine make(value: u32) -> Pair requires value <= 5 { Pair { left: value } }
         machine step(pair: Pair) -> Pair {
             transition pair.left > 0 {
                 true -> next(Pair { left: pair.left - 1 })
                 false -> (pair)
             }
             state next(result: Pair) -> Pair { result }
         }";
    let unproved = |source: &str| {
        construction_diagnostics(source).iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("literal cannot prove the default domain")
        })
    };
    assert!(!unproved(source), "{:?}", construction_diagnostics(source));
    // A wider clause, or a step that can climb past the ceiling, stays unproved.
    assert!(unproved(
        &source.replace("requires value <= 5", "requires value <= 6")
    ));
    assert!(unproved(&source.replace("pair.left - 1", "pair.left + 1")));
}

#[test]
fn synthesized_payload_tags_are_not_constructor_values() {
    let mut program = crate::front_end::typed_program(
        "trait Equatable { machine equals(&self, rhs: &Self) -> bool; }
         data Message { case Empty; case Data(value: i32); }
         MessageEquatable: Message satisfies Equatable;
         data Envelope { message: Message; }
         EnvelopeEquatable: Envelope satisfies Equatable;
         machine equal(left: Envelope, right: Envelope) -> bool { left == right }",
    );
    let mut diagnostics = Vec::new();
    validate_struct_literal_fields(&program, &mut diagnostics);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let generated: Vec<_> = program.expression_table.expression_entries().filter_map(|(expression, node)| {
        matches!(node, ExpressionNode::Binary(binary) if binary.operator == typed_trees::expression::BinaryOperator::CaseMembership)
            .then_some(expression)
    }).collect();
    assert!(!generated.is_empty());
    for expression in generated {
        let ExpressionNode::Binary(binary) = program.expression_table.expression_mut(expression)
        else {
            panic!("generated tag");
        };
        binary.operator = typed_trees::expression::BinaryOperator::Equal;
    }
    validate_struct_literal_fields(&program, &mut diagnostics);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("has a payload")),
        "{diagnostics:?}"
    );
}

#[test]
fn array_elements_require_explicit_integer_carrier_conversions() {
    for source in [
        "machine read(input: u8) -> [u16;1] { [input] }",
        "machine read(input: u8) -> [u16;1] { let values: [u16;1] = [input]; values }",
        "machine read(input: u8) -> [[u16;1];1] { [[input]] }",
        "machine helper(input: u8) -> u8 { input } machine read(input: u8) -> [u16;1] { [helper(input)] }",
        "machine read(input: u8) -> [u16;1] { [input ^ 1u8] }",
        "machine read() -> [u8;1] { let value: u16 = 7u16; [value] }",
        "machine read() -> [u8;1] { [7u16] }",
    ] {
        let program = crate::front_end::typed_program(source);
        let diagnostics = crate::validate_program(&program).expect_err(source);
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("array literal element has type")
                    && diagnostic.message.contains("explicit conversion")
            }),
            "{source}: {diagnostics:?}"
        );
    }
}

#[test]
fn field_guard_requires_the_exact_retained_selector() {
    for selector in ["invalid", "foreign", "parameter", "other_field"] {
        let mut program = crate::front_end::typed_program(
            "data Countdown { remaining: u64 [0..=5]; other: u64 [0..=5]; }
             data Foreign { remaining: u64 [0..=5]; }
             machine read(countdown: Countdown) -> u64 { countdown.remaining }",
        );
        let machine = program.machines()[0].clone();
        let state = program.machine_states(&machine)[0].clone();
        let StatementNode::Expression(expression) =
            program.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("field expression");
        };
        let field_symbol = |data_position, field_position| {
            let data = &program.data_definitions()[data_position];
            let DataMember::Field(field) = &program.data_members(data)[field_position] else {
                panic!("field");
            };
            field.symbol
        };
        let replacement = match selector {
            "foreign" => field_symbol(1, 0),
            "other_field" => field_symbol(0, 1),
            "parameter" => program.state_parameters(&state)[0].symbol,
            _ => symbols::SymbolHandle::invalid(),
        };
        let ExpressionNode::Member(member) = program.expression_table.expression_mut(expression)
        else {
            panic!("member");
        };
        member.member_symbol = replacement;
        assert!(
            !guard_bounds::has_immutable_inputs(&program, &machine, &state, expression),
            "{selector}"
        );
    }
}

#[test]
fn field_guard_does_not_import_authored_operator_facts() {
    for (operator, guard) in [
        (
            "operator > u64::not_a_bound(left: u64, right: u64) -> bool;",
            "countdown.remaining > 0",
        ),
        (
            "operator - u64::not_a_decrement(left: u64, right: u64) -> u64;",
            "countdown.remaining > 0",
        ),
    ] {
        let diagnostics = construction_diagnostics(&format!(
            "{operator}
             data Countdown {{ remaining: u64 [0..=5]; }}
             machine rebuild(countdown: Countdown) -> Countdown {{
                 transition {guard} {{ true -> Countdown {{ remaining: countdown.remaining - 1 }} _ -> Countdown {{ remaining: 0 }} }}
             }}"
        ));
        assert!(!diagnostics.is_empty(), "{operator}, {guard}");
    }
}

#[test]
fn default_domain_cannot_read_a_case_payload_name() {
    // A type-wide fact is indexed over common fields only: `hi` is case
    // `Range`'s payload, so the default domain cannot consult it. Authored
    // field VALUES are unaffected -- the rejection is for names a fact could
    // never have bound.
    let diagnostics = construction_diagnostics(
        "data Interval where hi >= 0, { case Range(lo: u64, hi: u64); }
         data Main { i: Interval; }
         machine Main::main(&mut self) { self.i = Interval::Range { lo: 1 }; }",
    );
    assert!(
        !diagnostics.is_empty(),
        "a type-wide fact naming an omitted case payload must not prove vacuously"
    );
}
