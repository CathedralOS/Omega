//! Byte indexes retain their source carrier before exact Terminal conversion.

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarType};
use terminal_interpreter::{
    AcceptTerminalEffects, TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalScalarValue, TerminalStructuralInputs, TerminalStructuralValue,
};
use terminal_psi::OperationKind;

fn check(source: &str) -> Result<CheckedTrees, Vec<Diagnostic>> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed)
}

fn assert_replacement_executes(index_type: &str) {
    let source = format!(
        r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record {{ out: [u8;3] in Utf8; }}
        machine Record::replace(&mut self, position: {index_type} [0..=2]) {{
            self.out = "XXX";
            self.out[position] = 65u8;
        }}
    "#
    );
    assert_replacement_program_executes(&source, index_type, b"XAX");
}

fn assert_replacement_program_executes(source: &str, index_type: &str, expected: &[u8]) {
    let checked = check(source).expect("bounded index source checks");
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Record::replace")
        .produce_artifact()
        .expect("bounded index publishes independently verified Terminal");
    drop(checked);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).expect("decode");
    terminal_verifier::validate_module(&module).expect("validate decoded module");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry");
    let ScalarType::Integer(scalar_type) = entry.parameters[0].scalar_type else {
        panic!("authored integer index");
    };
    assert_eq!(scalar_type.bits(), index_type[1..].parse::<u16>().unwrap());
    assert_eq!(
        scalar_type.sign(),
        if index_type.starts_with('i') {
            IntegerSign::Signed
        } else {
            IntegerSign::Unsigned
        },
    );
    let value = match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(1),
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
    };
    let (path, field) = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            OperationKind::StructuralByteSequenceFieldByteStore { path, field, .. } => {
                Some((path.clone(), *field))
            }
            _ => None,
        })
        .expect("byte store retained");
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[TerminalScalarValue::Integer { scalar_type, value }],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 73,
                structural_type: entry.structural_parameters[0].structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            ..Default::default()
        },
    )
    .expect("start source-free execution");
    let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(1000);
    assert_eq!(
        execution
            .resume(&mut fuel, &mut AcceptTerminalEffects)
            .expect("execute"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(
        execution.structural_byte_sequence_field(73, &path, field),
        Some(expected)
    );
}

#[test]
fn unsigned_64_byte_index_control_executes() {
    assert_replacement_executes("u64");
}

#[test]
fn signed_32_byte_index_executes_with_exact_coordinate_conversion() {
    assert_replacement_executes("i32");
}

#[test]
fn remaining_integer_byte_index_carriers_execute() {
    for carrier in ["i8", "i16", "i64", "u8", "u16"] {
        assert_replacement_executes(carrier);
    }
}

#[test]
fn unsigned_32_byte_index_executes_with_exact_coordinate_conversion() {
    assert_replacement_executes("u32");
}

#[test]
fn negative_and_past_end_byte_indexes_reject() {
    for position in ["-1i32", "3i32"] {
        let source = format!(
            r#"
            data Record {{ out: [u8;3]; }}
            machine Record::replace(&mut self) {{
                self.out = "XXX";
                self.out[{position}] = 65u8;
            }}
        "#
        );
        assert!(
            check(&source).is_err(),
            "invalid index {position} must reject"
        );
    }
}

#[test]
fn wrapping_index_arithmetic_executes_before_count_conversion() {
    // Every u8 result is in bounds. At input 1 the authored addition wraps to
    // zero; widening its operands first would instead index past the live end.
    let initial = "X".repeat(256);
    let source = format!(
        r#"
        domain [u8;256]::Utf8 requires valid_utf8(self);
        data Record {{ out: [u8;256] in Utf8; }}
        machine Record::replace(&mut self, position: u8 in Wrapping) {{
            self.out = "{initial}";
            self.out[position + 255] = 65u8;
        }}
    "#
    );
    let mut expected = initial.into_bytes();
    expected[0] = b'A';
    assert_replacement_program_executes(&source, "u8", &expected);
}

#[test]
fn signed_index_guard_executes() {
    let source = r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8;3] in Utf8; }
        machine Record::replace(&mut self, position: i32) {
            self.out = "XXX";
            transition position >= 0 && position < 3 {
                true -> store(position)
                false -> done()
            }
            state store(&mut self, position: i32) { self.out[position] = 65u8; }
            state done() {}
        }
    "#;
    assert_replacement_program_executes(source, "i32", b"XAX");
}

#[test]
fn missing_or_stale_signed_index_bounds_reject() {
    for parameter in ["i32", "i32 [-1..=2]", "i32 [0..=3]", "addr"] {
        let source = format!(
            r#"
            data Record {{ out: [u8;3]; }}
            machine Record::replace(&mut self, position: {parameter}) {{
                self.out = "XXX";
                self.out[position] = 65u8;
            }}
        "#
        );
        assert!(check(&source).is_err(), "missing bounds on {parameter}");
    }
    let stale = r#"
        data Record { out: [u8;3]; position: i32; }
        machine Record::replace(&mut self) {
            self.out = "XXX";
            self.position = 1;
            self.position = -1;
            self.out[self.position] = 65u8;
        }
    "#;
    assert!(
        check(stale).is_err(),
        "a prior nonnegative value cannot justify the write"
    );
}

#[test]
fn signed_index_rejects_substituted_retained_operand() {
    let source = r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8;3] in Utf8; }
        machine Record::replace(&mut self, position: i32 [0..=2]) {
            self.out = "XXX";
            self.out[position] = 65u8;
        }
    "#;
    let checked = check(source).expect("source checks");
    let _artifact =
        terminal_production::TerminalProductionRequest::new(&checked, "Record::replace")
            .produce_artifact()
            .expect("original dynamic index publishes");
    let mut changed = checked.clone();
    let replacement = checked_trees::CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(0).with_landing(
            numerics::literals::IntegerLanding {
                landed_type: numerics::literals::LandedIntegerType::I32,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let mut substituted = 0;
    for row in &mut changed.facts.values.scalar_expressions.expressions {
        if row.role == checked_trees::CheckedScalarExpressionRole::AssignmentIndex {
            row.expression = replacement.clone();
            substituted += 1;
        }
    }
    assert_eq!(substituted, 1);
    // Even when the store agrees with the substituted scalar row and the
    // constant itself is in bounds, authored operand correspondence must reject.
    let replace_store = |operation: &mut checked_trees::CheckedUnitEffectOperationPlan| {
        if let checked_trees::CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(
            store,
        ) = operation
        {
            store.index = replacement.clone();
            return 1;
        }
        0
    };
    let plans = &mut changed.facts.flow.terminal_unit_effects;
    let mut stores = 0;
    for machine in &mut plans.machines {
        for operation in &mut machine.operations {
            stores += replace_store(operation);
        }
    }
    for machine in &mut plans.composed_machines {
        for state in &mut machine.states {
            for operation in &mut state.operations {
                stores += replace_store(operation);
            }
        }
    }
    assert!(stores > 0);
    assert!(
        terminal_production::TerminalProductionRequest::new(&changed, "Record::replace")
            .produce_artifact()
            .is_err(),
        "an in-bounds substituted operand is not the authored index",
    );
}

#[test]
fn carrier_maximum_does_not_prove_nonnegativity_or_unused_capacity() {
    for (carrier, live_length) in [("i8", 256), ("u8", 255), ("u16", 256)] {
        let initial = "X".repeat(live_length);
        let source = format!(
            r#"
            domain [u8;256]::Utf8 requires valid_utf8(self);
            data Record {{ out: [u8;256] in Utf8; }}
            machine Record::replace(&mut self, position: {carrier}) {{
                self.out = "{initial}";
                self.out[position] = 65u8;
            }}
        "#
        );
        assert!(
            check(&source).is_err(),
            "{carrier}, live length {live_length}"
        );
    }
    let initial = "X".repeat(256);
    let shortened = format!(
        r#"
        domain [u8;256]::Utf8 requires valid_utf8(self);
        data Record {{ out: [u8;256] in Utf8; }}
        machine Record::replace(&mut self, position: u8) {{
            self.out = "{initial}";
            self.out = "X";
            self.out[position] = 65u8;
        }}
    "#
    );
    assert!(
        check(&shortened).is_err(),
        "the former live length does not survive replacement"
    );
}

#[test]
fn signed_index_mutates_the_callers_borrowed_byte_view() {
    let checked = check(
        r#"
        machine put(out: &mut [u8], position: i32 [0..=2]) {
            transition position >= 0 && position <= 2 && out.len > 2 {
                true -> store(out, position)
                false -> done()
            }
            state store(out: &mut [u8], position: i32 [0..=2]) {
                out[position] = 65u8;
            }
            state done() {}
        }
        machine run(out: &mut [u8;3], position: i32 [0..=2]) {
            put(out, position);
        }
    "#,
    )
    .expect("borrowed index checks");
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "run")
        .produce_artifact()
        .expect("borrowed index publishes");
    drop(checked);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).expect("decode");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry");
    let ScalarType::Integer(scalar_type) = entry.parameters[0].scalar_type else {
        panic!("integer")
    };
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[TerminalScalarValue::Integer {
            scalar_type,
            value: IntegerValue::Signed(1),
        }],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 73,
                structural_type: entry.structural_parameters[0].structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            byte_arrays: &[terminal_interpreter::TerminalStructuralByteArrayValue {
                argument_index: 0,
                path: Vec::new(),
                bytes: b"XXX".to_vec(),
            }],
            ..Default::default()
        },
    )
    .expect("start borrowed execution");
    let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(1000);
    assert_eq!(
        execution
            .resume(&mut fuel, &mut AcceptTerminalEffects)
            .expect("execute"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit),
    );
    assert_eq!(
        execution.structural_byte_array(73, &[]),
        Some(b"XAX".as_slice())
    );
}
