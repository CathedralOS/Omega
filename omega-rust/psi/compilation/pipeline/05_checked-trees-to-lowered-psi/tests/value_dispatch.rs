//! Selective value dispatch survives checked custody and canonical execution.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IeeeFloatValue, IntegerSign, IntegerType, IntegerValue};
use terminal_fuel::FuelChargeSite;
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_interpreter::{
    MeasuredTerminalExecution, TerminalExecutionResult, TerminalScalarValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralScalarFieldValue, TerminalStructuralValue,
    interpret_terminal_artifact_measured,
};
use terminal_psi::{OperationKind, TerminalModule};

#[path = "value_dispatch/borrowed_results.rs"]
mod borrowed_results;
#[path = "value_dispatch/owned_results.rs"]
mod owned_results;

fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
        value: IntegerValue::Unsigned(value),
    }
}

fn signed(value: i128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Signed, 64).expect("i64"),
        value: IntegerValue::Signed(value),
    }
}

#[test]
fn explicit_erasure_composes_with_introduction_and_partial_forgetting() {
    for body in [
        "(value as i64 in Position) as i64 in Km",
        "((value as i64 in Seconds) as i64) as i64 in Km",
    ] {
        let source = format!(
            "domain i64::Km; domain i64::Seconds; domain i64::Position = Km & Seconds;
            machine choose(value: i64) -> i64 {{ ({body}) as i64 }}"
        );
        let (_, execution) = execute(&source, &[signed(-17)]);
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(signed(-17))
        );
    }
}

#[test]
fn explicit_erasure_preserves_boolean_and_float_payload_bits() {
    for (carrier, left, right) in [
        (
            "bool",
            TerminalScalarValue::Boolean(true),
            TerminalScalarValue::Boolean(false),
        ),
        (
            "f32",
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(0x8000_0000)),
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(0x7fc0_0042)),
        ),
        (
            "f64",
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(0x8000_0000_0000_0000)),
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(0x7ff8_0000_0000_0042)),
        ),
    ] {
        let source = format!("domain {carrier}::Tagged;
            machine choose(flag: bool, left: {carrier}, right: {carrier}) -> {carrier} {{
                (match flag {{ true -> left as {carrier} in Tagged, false -> right as {carrier} in Tagged }}) as {carrier}
            }}");
        for (selected, expected) in [(true, left), (false, right)] {
            let (_, execution) = execute(
                &source,
                &[TerminalScalarValue::Boolean(selected), left, right],
            );
            assert_eq!(execution.value(), TerminalExecutionResult::Scalar(expected));
        }
    }
}

fn execute(
    source: &str,
    arguments: &[TerminalScalarValue],
) -> (TerminalModule, MeasuredTerminalExecution) {
    execute_machine(source, "choose", arguments)
}

fn execute_machine(
    source: &str,
    machine_name: &str,
    arguments: &[TerminalScalarValue],
) -> (TerminalModule, MeasuredTerminalExecution) {
    execute_machine_with_structural_inputs(source, machine_name, arguments, |_| {
        (Vec::new(), Vec::new(), Vec::new())
    })
}

/// `execute_machine` plus host-supplied structural entry contents. The
/// callback receives the decoded module so argument carriers and field rows
/// can name the exact structural type and field identities it declares.
fn execute_machine_with_structural_inputs(
    source: &str,
    machine_name: &str,
    arguments: &[TerminalScalarValue],
    inputs: impl FnOnce(
        &TerminalModule,
    ) -> (
        Vec<TerminalStructuralValue>,
        Vec<TerminalStructuralScalarFieldValue>,
        Vec<TerminalStructuralPrimitiveValue>,
    ),
) -> (TerminalModule, MeasuredTerminalExecution) {
    let checked = crate::front_end::checked_program(source);
    for machine in checked.machines() {
        assert_eq!(
            checked.machine_states(machine).len(),
            1,
            "dispatch does not manufacture source states"
        );
    }
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name(machine_name),
    )
    .unwrap_or_else(|error| panic!("lowering {source}: {error:#?}"));
    let semantic_bytes =
        terminal_codec::encode_module(&lowered.semantic_module).expect("encode semantics");
    let proof_bytes =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("encode proof");
    let module = terminal_codec::decode_module(&semantic_bytes).expect("decode semantics");
    let proof = terminal_codec::decode_proof_bundle(&proof_bytes).expect("decode proof");
    let profile = AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile)
        .expect("independent dispatch verification");
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    let (structural_arguments, scalar_fields, primitive_values) = inputs(&module);
    let execution = interpret_terminal_artifact_measured(
        &semantic_bytes,
        &proof_bytes,
        &profile,
        arguments,
        TerminalStructuralInputs {
            arguments: &structural_arguments,
            scalar_fields: &scalar_fields,
            primitive_values: &primitive_values,
            ..Default::default()
        },
        &mut AcceptTerminalEffects,
    )
    .unwrap_or_else(|error| panic!("execution {source}: {error:#?}"));
    (module, execution)
}

#[test]
fn float_match_results_compose_with_selected_calls_and_anonymous_subjects() {
    for (format, left, right) in [
        (
            "f32",
            IeeeFloatValue::Binary32(0x8000_0000),
            IeeeFloatValue::Binary32(0x7fc0_0042),
        ),
        (
            "f64",
            IeeeFloatValue::Binary64(0x8000_0000_0000_0000),
            IeeeFloatValue::Binary64(0x7ff8_0000_0000_0042),
        ),
    ] {
        for (expression, constant_left, calls) in [
            (
                "match flag { true -> identity(left), false -> identity(right) }",
                false,
                1,
            ),
            (
                "identity(match flag { true -> left, false -> right })",
                false,
                1,
            ),
            (
                "match 7 / 2 { 3 -> identity(right), 7 / 2 -> identity(left), _ -> identity(right) }",
                true,
                1,
            ),
        ] {
            let source = format!(
                "machine identity(value: {format}) -> {format} {{ value }} machine choose(flag: bool, left: {format}, right: {format}) -> {format} {{ {expression} }}"
            );
            for flag in [true, false] {
                let (module, execution) = execute(
                    &source,
                    &[
                        TerminalScalarValue::Boolean(flag),
                        TerminalScalarValue::IeeeFloat(left),
                        TerminalScalarValue::IeeeFloat(right),
                    ],
                );
                assert_eq!(
                    execution.value(),
                    TerminalExecutionResult::Scalar(TerminalScalarValue::IeeeFloat(
                        if constant_left || flag { left } else { right }
                    )),
                    "{source}"
                );
                let entry = module
                    .machines
                    .iter()
                    .find(|machine| machine.id == module.entry)
                    .expect("entry");
                let actual_calls: u64 = entry
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
                    .map(|operation| {
                        execution
                            .usage()
                            .at(FuelChargeSite::Operation(operation.id))
                            .map_or(0, |usage| usage.executions())
                    })
                    .sum();
                assert_eq!(actual_calls, calls, "{source}");
            }
        }
    }
}

#[test]
fn anonymous_numeric_dispatch_executes_only_the_selected_named_call() {
    for (subject, expected) in [("7 / 2", 7), ("5 / 2", 9)] {
        let source = format!(
            "machine identity(value: u64) -> u64 {{ value }}
             machine choose() -> u64 {{
                 match {subject} {{ 3 -> identity(99), 7 / 2 -> identity(7), _ -> identity(9) }}
             }}"
        );
        let (module, execution) = execute(&source, &[]);
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(unsigned(expected))
        );
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .expect("entry machine");
        let executions: u64 = entry
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
            .map(|operation| {
                execution
                    .usage()
                    .at(FuelChargeSite::Operation(operation.id))
                    .map_or(0, |usage| usage.executions())
            })
            .sum();
        assert_eq!(executions, 1, "only the selected result invokes identity");
    }
}

#[test]
fn anonymous_numeric_dispatch_composes_at_ordinary_value_edges() {
    let dispatch =
        "match 7 / 2 { 3 -> identity(99), 14 / 4 -> identity(value), _ -> identity(98) }";
    for (expression, expected) in [
        (format!("({dispatch}) | 8u64"), 15),
        (format!("8u64 | ({dispatch})"), 15),
        (format!("identity({dispatch})"), 7),
        (format!("({dispatch}) as u64"), 7),
        (
            format!("match value {{ 7 -> {dispatch}, _ -> identity(97) }}"),
            7,
        ),
    ] {
        let source = format!(
            "machine identity(value: u64) -> u64 {{ value }}
             machine choose(value: u64) -> u64 {{ {expression} }}"
        );
        let (module, execution) = execute(&source, &[unsigned(7)]);
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(unsigned(expected)),
            "{expression}"
        );
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .expect("entry machine");
        let calls: u64 = entry
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
            .map(|operation| {
                execution
                    .usage()
                    .at(FuelChargeSite::Operation(operation.id))
                    .map_or(0, |usage| usage.executions())
            })
            .sum();
        assert_eq!(
            calls,
            if expression.starts_with("identity(") {
                2
            } else {
                1
            },
            "selected call executes once, plus an optional outer identity: {expression}"
        );
    }
}

#[test]
fn value_dispatch_default_call_keeps_its_nonzero_premise_through_terminal() {
    let source = "machine nonzero(value: i64) -> i64 requires value != 0 { value }
        machine choose(value: i64) -> i64 { match value { 0 -> 7 _ -> nonzero(value) } }";
    let signed = |value| TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Signed, 64).expect("i64"),
        value: IntegerValue::Signed(value),
    };
    for (input, expected) in [(0, 7), (-3, -3), (9, 9)] {
        let (_, execution) = execute(source, &[signed(input)]);
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(signed(expected))
        );
    }
}

#[test]
fn value_dispatch_arms_land_exact_arithmetic_at_the_result_destination() {
    for source in [
        "machine choose(flag: bool) -> u64 {
        match flag {
            true -> 18446744073709551616 / 18446744073709551616
            false -> 7 / 2 * 2
        }
    }",
        "machine take(value: u64) -> u64 { value }
     machine choose(flag: bool) -> u64 {
        take(match flag {
            true -> match flag { _ -> 18446744073709551616 / 18446744073709551616 }
            false -> take(7 / 2 * 2)
        })
    }",
    ] {
        for (flag, expected) in [(true, 1), (false, 7)] {
            let (_, execution) = execute(source, &[TerminalScalarValue::Boolean(flag)]);
            assert_eq!(
                execution.value(),
                TerminalExecutionResult::Scalar(unsigned(expected))
            );
        }
    }
}

#[test]
fn composed_integer_peers_supply_anonymous_numeric_result_carriers() {
    // Each peer produces 24 in its actual u64 carrier before the outer OR.
    for typed_peer in [
        "8u64 | 16u64",
        "8u64 + 16u64",
        "~18446744073709551591u64",
        "192u64 >> 3u64",
    ] {
        for (anonymous, when_true, when_false) in [
            ("18446744073709551616 / 18446744073709551616", 25, 25),
            ("7 / 2 * 2", 31, 31),
            (
                "match flag { true -> 18446744073709551616 / 18446744073709551616, false -> 7 / 2 * 2 }",
                25,
                31,
            ),
            (
                "match flag { true -> match flag { _ -> 18446744073709551616 / 18446744073709551616 }, false -> match flag { _ -> 7 / 2 * 2 } }",
                25,
                31,
            ),
        ] {
            for expression in [
                format!("({anonymous}) | ({typed_peer})"),
                format!("({typed_peer}) | ({anonymous})"),
            ] {
                let source = format!("machine choose(flag: bool) -> u64 {{ {expression} }}");
                for (flag, expected) in [(true, when_true), (false, when_false)] {
                    let (_, execution) = execute(&source, &[TerminalScalarValue::Boolean(flag)]);
                    assert_eq!(
                        execution.value(),
                        TerminalExecutionResult::Scalar(unsigned(expected)),
                        "{expression}; flag={flag}"
                    );
                }
            }
        }
    }
}

#[test]
fn integer_cast_preserves_computed_match_arm_carrier_before_widening() {
    for typed_arm in [
        "8u32 | 16u32",
        "8u32 + 16u32",
        "~4294967271u32",
        "192u32 >> 3u32",
    ] {
        for (dispatch, when_true, when_false) in [
            (
                format!("match flag {{ true -> {typed_arm}, false -> 7 / 2 * 2 }}"),
                24,
                7,
            ),
            (
                format!(
                    "match flag {{ true -> 18446744073709551616 / 18446744073709551616, false -> {typed_arm} }}"
                ),
                1,
                24,
            ),
        ] {
            let source = format!("machine choose(flag: bool) -> u64 {{ ({dispatch}) as u64 }}");
            for (flag, expected) in [(true, when_true), (false, when_false)] {
                let (module, execution) = execute(&source, &[TerminalScalarValue::Boolean(flag)]);
                assert_eq!(
                    execution.value(),
                    TerminalExecutionResult::Scalar(unsigned(expected)),
                    "{dispatch}; flag={flag}"
                );
                assert!(
                    module
                        .machines
                        .iter()
                        .flat_map(|machine| &machine.blocks)
                        .flat_map(|block| &block.operations)
                        .any(|operation| matches!(
                            operation.kind,
                            OperationKind::IntegerWiden { .. }
                        )),
                    "the computed u32 arm must retain its carrier through the match join: {dispatch}"
                );
            }
        }
    }
}

#[test]
fn arithmetic_drops_match_input_ranges_without_erasing_integer_policy() {
    for (policy, computed_result) in [("Wrapping", 4), ("Saturating", 255)] {
        let qualified = format!("250 as u8[0..=250] in {policy}");
        for (expression, otherwise_result) in [
            (
                format!(
                    "(match subject {{ true -> {qualified}, false -> 18446744073709551616 / 18446744073709551616 }}) + 10"
                ),
                11,
            ),
            (
                format!("match subject {{ true -> ({qualified}) + 10, false -> 255 }}"),
                255,
            ),
        ] {
            // Erase policy at u8 before widening, so the outer u64 destination
            // cannot change the addition's carrier or preserve the input range.
            let source =
                format!("machine choose(subject: bool) -> u64 {{ (({expression}) as u8) as u64 }}");
            for subject in [true, false] {
                let (_, execution) = execute(&source, &[TerminalScalarValue::Boolean(subject)]);
                let expected = if subject {
                    computed_result
                } else {
                    otherwise_result
                };
                assert_eq!(
                    execution.value(),
                    TerminalExecutionResult::Scalar(unsigned(expected)),
                    "{source}; subject={subject}"
                );
            }
        }
    }
}

#[test]
fn match_numeric_range_weakening_preserves_each_arm_value() {
    for policy in ["", " in Wrapping", " in Saturating", " in Trapping"] {
        for alternative in ["11".to_owned(), format!("11 as u64[0..=20]{policy}")] {
            let source = format!(
                "machine choose(subject: bool) -> u64 {{ (match subject {{ true -> 1 as u64[0..=10]{policy}, false -> {alternative} }}) as u64 }}"
            );
            for (subject, expected) in [(true, 1), (false, 11)] {
                let (_, execution) = execute(&source, &[TerminalScalarValue::Boolean(subject)]);
                assert_eq!(
                    execution.value(),
                    TerminalExecutionResult::Scalar(unsigned(expected)),
                    "{source}; subject={subject}"
                );
            }
        }
    }
}
