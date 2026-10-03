use super::*;
use crate::proof_contracts::arithmetic_domains::{
    requires_value_environment, validate_value_range,
};
use layout_plans::{IntegerInterpretation, LayoutPlacementReport};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::data::DataMember;
use typed_trees::statement::StatementNode;

fn program_with_storage(source: &str, width: u16, sign: IntegerSign) -> TypedTrees {
    let mut program = crate::front_end::typed_program(source);
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Cell")
        .unwrap();
    let symbol = data.symbol;
    let [DataMember::Field(field)] = program.data_members(data) else {
        panic!("one field");
    };
    let field = field.symbol;
    let interpretation = match sign {
        IntegerSign::Signed => IntegerInterpretation::Signed,
        IntegerSign::Unsigned => IntegerInterpretation::Unsigned,
    };
    // This query consumes the exact normalized geometry published by layout
    // evaluation. The public recast corpus fixture exercises that producer.
    program.plan_laid_layouts.push(typed_trees::PlanLaidLayout {
        data_name: "Cell".to_owned(),
        data_symbol: symbol,
        field_symbols: vec![field],
        schema_symbol: symbol,
        schema_field_symbols: vec![field],
        policy_symbol: symbols::SymbolHandle::invalid(),
        policy_plan_machine_symbol: symbols::SymbolHandle::invalid(),
        validated_layout: layout_plans::LayoutPlanReport {
            schema_report_fingerprint: 0,
            entries: vec![layout_plans::LayoutFieldEntryReport {
                field: "value".to_owned(),
                member_identity: None,
                placement: LayoutPlacementReport::IntegerAt {
                    offset: 0,
                    stored_width: u64::from(width),
                    interpretation,
                },
            }],
            offsets: None,
            size: Some(u64::from(width / 8)),
            align: 1,
        },
        private_callback_demands: Vec::new(),
        offsets: vec![0],
        bit_fields: Vec::new(),
        integer_fields: vec![typed_trees::PlanLaidIntegerField {
            field_index: 0,
            stored_width_bits: width,
            interpretation,
            write_is_total: false,
        }],
        repeated_fields: Vec::new(),
        size: usize::from(width / 8),
        align: 1,
    });
    program
}

fn discharge(source: &str, width: u16, sign: IntegerSign) -> bool {
    let program = program_with_storage(source, width, sign);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "store")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let assignment = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::Assignment(assignment) => Some(assignment),
            _ => None,
        })
        .unwrap();
    let mut environment = requires_value_environment(&program, machine, state);
    for statement in program.statement_table.statements(state.statement_nodes) {
        if let StatementNode::LocalData(local) = statement {
            environment = crate::proof_contracts::arithmetic_domains::guard_narrowed_environment(
                &program,
                machine,
                Some(state),
                &typed_trees::statement::TransitionGuardNode::When(local.initial_value),
                &environment,
            );
        }
    }
    let mut diagnostics = Vec::new();
    let primitive = program.primitive_type_reference(
        places::declared_place_type_raw(&program, machine, Some(state), assignment.target).unwrap(),
    );
    let (interval, source, _) = validate_value_range(
        &program,
        machine,
        Some(state),
        assignment.value,
        &environment,
        primitive,
        ArithmeticDomain::Exact,
        "store",
        &mut diagnostics,
    );
    assert!(diagnostics.is_empty(), "RHS: {diagnostics:?}");
    let result = validate_stored_integer_write(
        &program,
        machine,
        Some(state),
        assignment.target,
        assignment.value,
        interval,
        source,
        &environment,
        "store",
        &mut diagnostics,
    );
    if let Some(result) = result {
        assert_eq!(result.destination, assignment.target);
        assert_eq!(result.value, assignment.value);
        assert_eq!(result.stored_type, IntegerType::new(sign, width).unwrap());
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        true
    } else {
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("requires a value proven to fit")),
            "a missing geometry or skipped store is not a range rejection: {diagnostics:?}"
        );
        false
    }
}

#[test]
fn stored_integer_write_checks_signed_boundaries_and_unknown_values() {
    for (value, accepted) in [
        ("-129", false),
        ("-128", true),
        ("127", true),
        ("128", false),
        ("input", false),
    ] {
        assert_eq!(
            discharge(
                &format!(
                    "data Cell {{ value: i64; }} machine store(cell: &mut Cell, input: i64) {{ cell.value = {value}; }}"
                ),
                8,
                IntegerSign::Signed
            ),
            accepted,
            "{value}"
        );
    }
}

#[test]
fn stored_integer_write_retains_current_parameter_bounds() {
    for (bound, accepted) in [("127", true), ("128", false)] {
        assert_eq!(
            discharge(
                &format!(
                    "data Cell {{ value: i64; }} machine store(cell: &mut Cell, input: i64) requires 0 <= input, input <= {bound} {{ cell.value = input; }}"
                ),
                8,
                IntegerSign::Signed
            ),
            accepted
        );
    }
}

#[test]
fn stored_integer_write_retains_declared_rhs_range() {
    assert!(discharge(
        "data Cell { value: i64; } machine store(cell: &mut Cell, input: i64[0..=127]) { cell.value = input; }",
        8,
        IntegerSign::Signed,
    ));
}

#[test]
fn stored_integer_write_uses_the_selected_guard_environment() {
    assert!(discharge(
        "data Cell { value: i64; } machine store(cell: &mut Cell, input: i64) {
            let eligible: bool = 0 <= input && input <= 127;
            cell.value = input;
        }",
        8,
        IntegerSign::Signed,
    ));
}

#[test]
fn stored_integer_write_preserves_unsigned_64_boundaries() {
    for (width, sign, value, accepted) in [
        (8, IntegerSign::Unsigned, "255", true),
        (8, IntegerSign::Unsigned, "256", false),
        (64, IntegerSign::Unsigned, "18446744073709551615", true),
        (64, IntegerSign::Signed, "18446744073709551615", false),
        (64, IntegerSign::Unsigned, "input", true),
        (64, IntegerSign::Signed, "input", false),
    ] {
        assert_eq!(
            discharge(
                &format!(
                    "data Cell {{ value: u64; }} machine store(cell: &mut Cell, input: u64) {{ cell.value = {value}; }}"
                ),
                width,
                sign
            ),
            accepted,
            "{value}/{width}/{sign:?}"
        );
    }
}

#[test]
fn stored_integer_write_cannot_use_storage_width_as_an_unknown_rhs_bound() {
    let byte = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert!(!super::interval_fits(byte, Interval::UNBOUNDED, None));
    assert!(!super::interval_fits(
        byte,
        Interval::UNBOUNDED,
        Some(PrimitiveType::U64)
    ));
    assert!(super::interval_fits(
        byte,
        Interval::UNBOUNDED,
        Some(PrimitiveType::U8)
    ));
}
