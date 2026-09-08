use super::*;
use symbols::SymbolHandle;
use typed_trees::data::DataMember;
use typed_trees::name::Identifier;
use typed_trees::statement::StatementNode;

#[test]
fn direct_fields_and_scalar_operands_share_exact_arithmetic_bounds() {
    for (expression, expected) in [
        ("input.value", (8, 12)),
        ("input.value + input.divisor", (10, 16)),
        ("input.value - input.divisor", (4, 10)),
        ("input.value * input.divisor", (16, 48)),
        ("input.value / input.divisor", (2, 6)),
        ("input.value % input.divisor", (0, 3)),
        ("input.value + scalar", (10, 16)),
        ("input.value - scalar", (4, 10)),
        ("scalar * input.value", (16, 48)),
        ("input.value / scalar", (2, 6)),
        ("input.value % scalar", (0, 3)),
        ("1 + input.value", (9, 13)),
        ("20u16 - input.value", (8, 12)),
        ("2u16 * input.value", (16, 24)),
        ("24u16 / input.value", (2, 3)),
        ("24u16 % input.value", (0, 11)),
        ("(input.value + input.divisor) * 2", (20, 32)),
    ] {
        assert_eq!(
            query(&format!(
                "data Inputs {{ value: u16 [8..=12]; divisor: u16 [2..=4]; }}
                 machine value(input: Inputs, scalar: u16 [2..=4]) -> u16 {{ {expression} }}"
            )),
            Some(expected),
            "{expression}"
        );
    }
}

#[test]
fn signed_field_bounds_keep_truncated_quotient_and_dividend_sign_remainder() {
    for (expression, expected) in [
        ("input.value + input.divisor", (-11, -3)),
        ("input.value - input.divisor", (-5, 3)),
        ("input.value * input.divisor", (2, 28)),
        ("input.value / input.divisor", (0, 3)),
        ("input.value % input.divisor", (-3, 0)),
        ("input.value % 2i16", (-1, 0)),
    ] {
        assert_eq!(
            query(&format!(
                "data Inputs {{ value: i16 [-7..=-1]; divisor: i16 [-4..=-2]; }}
                 machine value(input: Inputs) -> i16 {{ {expression} }}"
            )),
            Some(expected),
            "{expression}"
        );
    }
}

#[test]
fn unbounded_unsigned_field_retains_its_useful_remainder_floor() {
    for (expression, expected) in [
        ("input.value", None),
        ("input.value % input.divisor", Some((0, 4))),
        ("(input.value % input.divisor) + 6", Some((6, 10))),
        ("((input.value + 0) % input.divisor) + 6", Some((6, 10))),
        ("((input.value + 1) % input.divisor) + 6", None),
        ("(input.value * 2) % input.divisor", None),
        ("(input.value - 1) % input.divisor", None),
    ] {
        assert_eq!(
            query(&format!(
                "data Inputs {{ value: u64; divisor: u64 [1..=5]; }}
                 machine value(input: Inputs) -> u64 {{ {expression} }}"
            )),
            expected,
            "{expression}: an absent i64 ceiling cannot hide an Exact overflow"
        );
    }
}

#[test]
fn safe_final_field_bounds_do_not_excuse_overflow_at_an_inner_operation() {
    for (carrier, range, expression) in [
        ("u8", "[250..=255]", "(input.value + 1) % 5"),
        ("u8", "[0..=5]", "(input.value - 1) + 1"),
        ("u8", "[100..=130]", "(input.value * 2) / 2"),
        ("i8", "[120..=127]", "(input.value + 1) - 1"),
        ("i8", "[-128..=-120]", "(input.value - 1) + 1"),
    ] {
        assert_eq!(
            query(&format!(
                "data Input {{ value: {carrier} {range}; }}
                 machine value(input: Input) -> {carrier} {{ {expression} }}"
            )),
            None,
            "{carrier} {range}: {expression}"
        );
    }
}

#[test]
fn field_quotient_and_remainder_require_a_nonzero_divisor() {
    for divisor_range in ["[0..=0]", "[-1..=1]", "[0..=3]"] {
        for operator in ["/", "%"] {
            assert_eq!(
                query(&format!(
                    "data Inputs {{ value: i16 [1..=5]; divisor: i16 {divisor_range}; }}
                     machine value(input: Inputs) -> i16 {{ input.value {operator} input.divisor }}"
                )),
                None,
                "{operator} with {divisor_range} cannot establish primitive definedness"
            );
        }
    }
}

#[test]
fn signed_minimum_field_remainder_keeps_the_quotient_overflow_obligation() {
    for carrier in ["i8", "i16", "i32", "i64"] {
        for operator in ["/", "%"] {
            assert_eq!(
                query(&format!(
                    "data Inputs {{ value: {carrier}; divisor: {carrier} [-1..=-1]; }}
                     machine value(input: Inputs) -> {carrier} {{ input.value {operator} input.divisor }}"
                )),
                None,
                "{carrier} MIN {operator} -1 is not an Exact value"
            );
        }
        assert_eq!(
            query(&format!(
                "data Inputs {{ value: {carrier} [0..=5]; divisor: {carrier} [-1..=-1]; }}
                 machine value(input: Inputs) -> {carrier} {{ input.value % input.divisor }}"
            )),
            Some((0, 0)),
            "a negative divisor alone does not invalidate {carrier} remainder"
        );
    }
}

#[test]
fn field_arithmetic_lands_completed_anonymous_rationals_once() {
    for (expression, expected) in [
        ("input.value + (3 / 2) * 2", Some((11, 15))),
        ("((3 / 2) * 2) + input.value", Some((11, 15))),
        ("input.value + 3 / 2", None),
        ("input.value + 1 / 0", None),
        ("input.value % 256", None),
        ("input.value % -1", None),
        ("input.value + 1u16", None),
        ("input.value + 1u8", Some((9, 13))),
    ] {
        assert_eq!(
            query(&format!(
                "data Input {{ value: u8 [8..=12]; }}
                 machine value(input: Input) -> u8 {{ {expression} }}"
            )),
            expected,
            "{expression}"
        );
    }
}

#[test]
fn field_arithmetic_does_not_reuse_operand_refinements_as_result_bounds() {
    assert_eq!(
        query(
            "data Input { value: u16 [20..=30]; }
             machine value(input: Input) -> u16 { (input.value % 5) + 1 }"
        ),
        Some((1, 5))
    );
}

#[test]
fn field_queries_report_unsupported_custody_without_declaring_source_invalid() {
    for (source, reason) in [
        (
            "data Input { value: u16 [0..=5]; }
             machine value(input: &Input) -> u16 { input.value + 1 }",
            "shared references need live storage evidence outside this invariant query",
        ),
        (
            "data Input { value: u16 [0..=5]; }
             machine value(input: &mut Input) -> u16 { input.value + 1 }",
            "mutable references do not provide snapshot-independent field facts",
        ),
        (
            "data Input { value: u16 [0..=5]; }
             machine value(mut input: Input) -> u16 { input.value + 1 }",
            "mutable owned parameters are outside the immutable query's support",
        ),
        (
            "data Input { value: u16 [0..=5]; }
             machine value(input: Input) -> u16 { let saved: Input = input; saved.value + 1 }",
            "local custody is not recovered from its initializer",
        ),
        (
            "data Input { value: u16 [0..=5]; }
             machine value() -> u16 { let saved: Input = Input { value: 2 }; saved.value + 1 }",
            "a constructor initializer is not a recurring state invariant",
        ),
        (
            "data Input { value: u16 [0..=5]; }
             data Indirect { target: &Input; }
             machine value(input: Indirect) -> u16 { input.target.value + 1 }",
            "projected references require storage-origin and mutation evidence",
        ),
        (
            "data Input { value: &u16 [0..=5]; }
             machine value(input: Input) -> &u16 [0..=5] { input.value }",
            "an owned record does not turn a reference field into an owned integer",
        ),
        (
            "data Input { value: u16 [0..=5] in Wrapping; }
             machine value(input: Input) -> u16 in Wrapping { input.value + 1 }",
            "Wrapping arithmetic is valid but is not Exact interval arithmetic",
        ),
    ] {
        assert_eq!(query(source), None, "support boundary: {reason}");
    }
}

#[test]
fn field_queries_do_not_infer_ranges_from_entry_flow_premises() {
    assert_eq!(
        query(
            "data Input { value: u64; }
             machine value(input: Input) -> u64
                 requires input.value <= 5 { input.value + 1 }"
        ),
        None,
        "support boundary: this query only reads declared ranges; entry proof belongs to flow analysis"
    );
}

#[test]
fn same_spelled_foreign_state_parameters_cannot_supply_field_bounds() {
    let program = typed(
        "data Input { value: u16 [0..=5]; }
         machine first(input: Input) -> u16 { input.value + 1 }
         machine second(input: Input) -> u16 { input.value + 1 }",
    );
    let first = &program.machines()[0];
    let second = &program.machines()[1];
    let first_state = &program.machine_states(first)[0];
    let second_state = &program.machine_states(second)[0];
    let StatementNode::Expression(expression) = program
        .statement_table
        .statements(first_state.statement_nodes)[0]
    else {
        panic!("field arithmetic expression");
    };
    assert_eq!(
        immutable_integer_expression_bounds(&program, first, first_state, expression),
        Some((1, 6))
    );
    for (machine, state) in [(second, second_state), (second, first_state)] {
        assert_eq!(
            immutable_integer_expression_bounds(&program, machine, state, expression),
            None,
            "both the parameter and its state must belong to the querying owner"
        );
    }
}

#[test]
fn field_bounds_require_the_exact_nominal_owner_and_field_selector() {
    for corruption in [
        "missing",
        "foreign",
        "parameter",
        "other_field",
        "case",
        "owner",
    ] {
        let mut program = typed(
            "data Input { value: u16 [0..=5]; spare: u16 [20..=30]; }
             data Foreign { value: u16 [100..=120]; }
             machine value(input: Input) -> u16 { input.value + 1 }",
        );
        let machine = program.machines()[0].clone();
        let state = program.machine_states(&machine)[0].clone();
        let StatementNode::Expression(expression) =
            program.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("field arithmetic expression");
        };
        assert_eq!(
            immutable_integer_expression_bounds(&program, &machine, &state, expression),
            Some((1, 6))
        );
        let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
            panic!("addition");
        };
        let field_expression = binary.left;
        let field_symbol = |definition_position, field_position| {
            let definition = &program.data_definitions()[definition_position];
            let DataMember::Field(field) = &program.data_members(definition)[field_position] else {
                panic!("declared field");
            };
            field.symbol
        };
        let replacement = match corruption {
            "foreign" => field_symbol(1, 0),
            "other_field" => field_symbol(0, 1),
            "parameter" => program.state_parameters(&state)[0].symbol,
            _ => SymbolHandle::invalid(),
        };
        if corruption == "owner" {
            let foreign_owner = program.data_definitions()[1].symbol;
            let forged_owner = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: foreign_owner,
                    name: Identifier::generated_static("Input"),
                });
            program.state_parameters.span_mut_or_empty(state.parameters)[0].type_reference =
                forged_owner;
        } else {
            let ExpressionNode::Member(member) =
                program.expression_table.expression_mut(field_expression)
            else {
                panic!("direct field");
            };
            if corruption == "case" {
                member.case_variant = Some(Identifier::generated_static("Case"));
            } else {
                member.member_symbol = replacement;
            }
        }
        assert_eq!(
            immutable_integer_expression_bounds(&program, &machine, &state, expression),
            None,
            "{corruption} identity cannot borrow Input.value's range"
        );
    }
}

#[test]
fn field_receiver_handles_cannot_be_repaired_by_parameter_spelling() {
    for corruption in [
        "missing_symbol",
        "missing_head",
        "foreign_symbol",
        "foreign_head",
        "spoofed_parameter",
    ] {
        let mut program = typed(
            "data Input { value: u16 [0..=5]; }
             machine value(input: Input, other: Input) -> u16 { input.value + 1 }
             machine foreign(input: Input) -> u16 { input.value }",
        );
        let machine = program.machines()[0].clone();
        let state = program.machine_states(&machine)[0].clone();
        let foreign_state = &program.machine_states(&program.machines()[1])[0];
        let foreign_symbol = program.state_parameters(foreign_state)[0].symbol;
        let other_symbol = program.state_parameters(&state)[1].symbol;
        let StatementNode::Expression(expression) =
            program.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("field arithmetic expression");
        };
        let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
            panic!("addition");
        };
        let ExpressionNode::Member(member) = program.expression_table.expression(binary.left)
        else {
            panic!("direct field");
        };
        let receiver = member.receiver;
        let ExpressionNode::Name(path) = program.expression_table.expression_mut(receiver) else {
            panic!("parameter receiver");
        };
        match corruption {
            "missing_symbol" => path.symbol = SymbolHandle::invalid(),
            "missing_head" => path.head_symbol = SymbolHandle::invalid(),
            "foreign_symbol" => {
                path.symbol = foreign_symbol;
                path.head_symbol = foreign_symbol;
            }
            "foreign_head" => path.head_symbol = foreign_symbol,
            "spoofed_parameter" => {
                path.symbol = other_symbol;
                path.head_symbol = other_symbol;
            }
            _ => unreachable!(),
        }
        assert_eq!(
            immutable_integer_expression_bounds(&program, &machine, &state, expression),
            None,
            "{corruption} must not recover custody from the original input spelling"
        );
    }
}

#[test]
fn same_spelled_nominal_field_carrier_is_not_a_builtin_integer() {
    let mut program = typed(
        "data Input { value: u16 [0..=5]; }
         data Impostor {}
         machine value(input: Input) -> u16 { input.value + 1 }",
    );
    let impostor_symbol = program.data_definitions()[1].symbol;
    let forged_carrier = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: impostor_symbol,
            name: Identifier::generated_static("u16"),
        });
    let members = program.data_definitions()[0].members;
    let DataMember::Field(field) = &mut program.data_members.span_mut_or_empty(members)[0] else {
        panic!("declared field");
    };
    field.type_reference = forged_carrier;
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let StatementNode::Expression(expression) =
        program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("field arithmetic expression");
    };
    assert_eq!(
        immutable_integer_expression_bounds(&program, machine, state, expression),
        None
    );
}

#[test]
fn declared_field_operators_do_not_acquire_builtin_arithmetic_meaning() {
    for (operator, name) in [
        ("+", "add"),
        ("-", "subtract"),
        ("*", "multiply"),
        ("/", "divide"),
        ("%", "remainder"),
    ] {
        assert_eq!(
            query(&format!(
                "operator {operator} u16::{name}(left: u16, right: u16) -> u16;
                 data Inputs {{ value: u16 [8..=12]; divisor: u16 [2..=4]; }}
                 machine value(input: Inputs) -> u16 {{ input.value {operator} input.divisor }}"
            )),
            None,
            "the selected {operator} declaration supplies no builtin interval law"
        );
    }
}

#[test]
fn computed_field_bounds_retain_selected_conformance_operator_meaning() {
    use typed_trees::typed_trees::{
        ClosedConformanceApplication, ClosedConformanceRowIdentity, MachineSpecialization,
    };

    let mut program = typed(
        "trait SelectedArithmetic { operator + add(left: Self, right: Self) -> Self; }
         Chosen: u64 satisfies SelectedArithmetic {
             machine add(left: u64, right: u64) -> u64 { 100u64 }
         }
         data Input { value: u64; }
         machine value(input: Input) -> u64 { (input.value % 5) + 1 }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .expect("query machine")
        .clone();
    let state = program.machine_states(&machine)[0].clone();
    let StatementNode::Expression(expression) =
        program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("computed field expression");
    };
    assert_eq!(
        immutable_integer_expression_bounds(&program, &machine, &state, expression),
        Some((1, 5))
    );
    let DataMember::Field(field) = &program.data_members(&program.data_definitions()[0])[0] else {
        panic!("integer field");
    };
    let field_type = field.type_reference;
    let conformance = program.conformances()[0].clone();
    let rows = program
        .closed_conformance_rows(&conformance)
        .expect("closed selected conformance")
        .iter()
        .map(|row| ClosedConformanceRowIdentity {
            declaring_trait: row.declaring_trait,
            requirement: row.requirement,
            realization_machine: row.realization_machine,
            realization_state: row.realization_state,
        })
        .collect();
    // As in the parent suite, this supplies query-level selection, not publication evidence.
    let application = ClosedConformanceApplication {
        declaration: conformance.symbol,
        subject_identity: Some(program.display_type_reference(field_type)),
        trait_definition: conformance.trait_symbol,
        rows,
        ..Default::default()
    };
    program.machine_specializations.push(MachineSpecialization {
        template: machine.symbol,
        instance: machine.symbol,
        conformance_arguments: vec![conformance.symbol],
        conformance_applications: vec![application],
        ..Default::default()
    });
    assert_eq!(
        typed_trees::operator::selected_trait_operator_meanings(
            &program,
            machine.symbol,
            OperatorSpelling::Add,
            &[Some(field_type), None],
        )
        .len(),
        1,
        "the fixture selects the field carrier's authored addition"
    );
    assert_eq!(
        immutable_integer_expression_bounds(&program, &machine, &state, expression),
        None,
        "a safe builtin remainder does not establish the selected parent's arithmetic law"
    );
}
