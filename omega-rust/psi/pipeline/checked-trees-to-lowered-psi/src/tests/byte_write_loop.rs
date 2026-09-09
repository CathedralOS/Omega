//! A fresh guard proves each indexed write without a termination claim.

use super::*;
use terminal_fuel::{FuelChargeSite, TerminalFuelMeter};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalStructuralByteArrayValue, TerminalStructuralValue,
};

const FILL: &str = r#"
machine fill(out: &mut [u8], byte: u8) {
    transition { _ -> scan(out, 0, byte) }
    state scan(out: &mut [u8], position: u64, byte: u8) {
        transition position < out.len {
            true -> store(out, position, byte)
            false -> done()
        }
    }
    state store(out: &mut [u8], position: u64, byte: u8) {
        out[position] = byte;
        transition { _ -> scan(out, position + 1, byte) }
    }
    state done() {}
}
"#;

const READ_ONE: &str = r#"
        data ByteRead { case Eof; case Byte(value: i32 [0..=255]); }
        boundary trait Console {
            machine read_byte() -> ByteRead reaches Console;
        }
        machine read_one(out: &mut [u8]) reaches Console {
            transition out.len > 0 {
                true -> read(out)
                false -> done()
            }
            state read(out: &mut [u8]) {
                let observed: ByteRead = Console::read_byte();
                transition observed {
                    ByteRead::Byte { value } -> store(out, value)
                    ByteRead::Eof -> done()
                }
            }
            state store(out: &mut [u8], value: i32 [0..=255]) {
                out[0] = value as u8;
            }
            state done() {}
        }
        "#;

#[test]
fn line_result_constructor_retains_runtime_count_in_terminal() {
    let checked = checked_source(
        r#"
        data LineReadResult {
            case Invalid;
            case LineComplete(count: u64);
            case EndOfInput(count: u64);
            case Full(count: u64);
        }
        machine full(count: u64) -> LineReadResult {
            LineReadResult::Full { count: count }
        }
        "#,
    );
    let artifact = produce_terminal_artifact(&checked, "full")
        .expect("a returned line outcome retains its runtime count");
    for count in [0, 7, u64::MAX] {
        let argument = terminal_interpreter::TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            value: IntegerValue::Unsigned(count as u128),
        };
        let mut execution = TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[argument],
        )
        .unwrap();
        let TerminalExecutionStatus::Complete(TerminalExecutionResult::ScalarCase(result)) =
            execution
                .resume(&mut TerminalFuelMeter::with_allowance(100))
                .unwrap()
        else {
            panic!("case return did not complete");
        };
        assert_eq!(result.value.fields.len(), 1);
        assert_eq!(result.value.fields[0].1, argument);
    }
}

#[test]
fn scalar_case_return_preserves_authored_multifield_identity_and_rejects_plan_drift() {
    let checked = checked_source(
        r#"
        data PairResult { case Empty; case Pair(left: u64, right: u64); }
        machine pair(left: u64, right: u64) -> PairResult {
            PairResult::Pair { right: right, left: left }
        }
    "#,
    );
    let artifact = produce_terminal_artifact(&checked, "pair").unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let selected = module
        .structural_types
        .iter()
        .find_map(|declaration| match &declaration.shape {
            StructuralTypeShape::Sum { cases } => cases.iter().find(|case| case.identity == "Pair"),
            _ => None,
        })
        .unwrap();
    let arguments = [11, 29].map(|value| terminal_interpreter::TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    });
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &arguments,
    )
    .unwrap();
    let TerminalExecutionStatus::Complete(TerminalExecutionResult::ScalarCase(result)) = execution
        .resume(&mut TerminalFuelMeter::with_allowance(100))
        .unwrap()
    else {
        panic!("pair did not return");
    };
    for (identity, expected) in [("left", arguments[0]), ("right", arguments[1])] {
        let field = selected
            .fields
            .iter()
            .find(|field| field.identity == identity)
            .unwrap();
        assert!(result.value.fields.contains(&(field.id, expected)));
    }
    for corruption in 0..6 {
        let mut changed = checked.clone();
        let plan = &mut changed.facts.flow.terminal_unit_effects.composed_machines[0];
        let checked_trees::CheckedComposedUnitControlTerminatorPlan::ReturnCase {
            fields,
            case_identity,
            ..
        } = &mut plan.states[0].terminator
        else {
            panic!("case plan missing");
        };
        match corruption {
            0 => fields.swap(0, 1),
            1 => fields[0].field_identity = fields[1].field_identity.clone(),
            2 => fields[0].expression = fields[1].expression.clone(),
            3 => *case_identity = "Empty".to_owned(),
            4 => {
                fields.pop();
            }
            _ => plan.result = checked_trees::CheckedControlResultPlan::Unit,
        }
        assert!(
            produce_terminal_artifact(&changed, "pair").is_err(),
            "corruption {corruption}"
        );
    }
}

#[test]
fn scalar_case_return_bounded_literal_requires_constructor_evidence() {
    let source = r#"
        data Bounded { case Empty; case Count(value: u64 [0..=7]); }
        machine bounded() -> Bounded { Bounded::Count { value: 7 } }
    "#;
    let checked = checked_source(source);
    let artifact = produce_terminal_artifact(&checked, "bounded")
        .expect("literal proves the exact declaration range");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert!(module.machines.iter().flat_map(|machine| &machine.blocks).flat_map(|block| &block.operations)
        .any(|operation| matches!(&operation.kind, OperationKind::EstablishScalarCase { fields, .. }
            if fields.len() == 1 && fields[0].range_obligation.is_some())));
    let invalid = source.replace("value: 7", "value: 8");
    let syntax = parse_syntax_trees(&Lexer::new(&invalid).tokenize().unwrap()).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    if let Ok(checked) = lower_typed_trees(typed) {
        assert!(
            produce_terminal_artifact(&checked, "bounded").is_err(),
            "out-of-range construction must not publish an artifact"
        );
    }
}

#[test]
fn scalar_case_return_multistate_borrowed_view_and_ordinary_call_observe_count() {
    let checked = checked_source(
        r#"
        data Outcome { case Empty; case Full(count: u64); }
        machine make(out: &mut [u8], count: u64, full: bool) -> Outcome {
            transition full { true -> scan(out, count) false -> empty(out) }
            state scan(out: &mut [u8], count: u64) -> Outcome {
                transition count < 7 { true -> advance(out, count) false -> filled(out, count) }
            }
            state advance(out: &mut [u8], count: u64) -> Outcome {
                transition { _ -> scan(out, count + 1) }
            }
            state filled(out: &mut [u8], count: u64) -> Outcome { Outcome::Full { count: count } }
            state empty(out: &mut [u8]) -> Outcome {
                transition out.len > 0 { true -> empty_store(out) false -> empty_return() }
            }
            state empty_store(out: &mut [u8]) -> Outcome {
                out[0] = 41;
                Outcome::Empty
            }
            state empty_return() -> Outcome { Outcome::Empty }
        }
        machine collect(out: &mut [u8], full: bool) {
            transition out.len > 0 { true -> read(out, full) false -> done() }
            state read(out: &mut [u8], full: bool) {
                let observed: Outcome = make(out, 0, full);
                transition observed {
                    Outcome::Full { count } -> observed_count(out, count)
                    Outcome::Empty -> done()
                }
            }
            state observed_count(out: &mut [u8], count: u64) {
                transition count == 7 { true -> writable(out) false -> done() }
            }
            state writable(out: &mut [u8]) {
                transition out.len > 0 { true -> store(out) false -> done() }
            }
            state store(out: &mut [u8]) { out[0] = 67; }
            state done() {}
        }
        data Record { out: [u8; 1]; }
        machine Record::run(&mut self, full: bool) { collect(&mut self.out, full); }
    "#,
    );
    let artifact = produce_terminal_artifact(&checked, "Record::run")
        .expect("ordinary scalar-case call composes with view and count transfers");
    for full in [false, true] {
        let path = vec![StructuralPathSegment::Field("out".into())];
        let mut execution =
            TerminalExecution::start_artifact_with_structural_arguments_and_byte_arrays(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &proof_admission::AdmissionProfile::default(),
                &[terminal_interpreter::TerminalScalarValue::Boolean(full)],
                &[entry_argument(&artifact)],
                &[TerminalStructuralByteArrayValue {
                    argument_index: 0,
                    path: path.clone(),
                    bytes: vec![19],
                }],
            )
            .unwrap();
        let mut fuel = TerminalFuelMeter::with_allowance(0);
        let mut completed = false;
        for _ in 0..512 {
            match execution.resume(&mut fuel).unwrap() {
                TerminalExecutionStatus::SponsorExhausted(_) => fuel.replenish(1).unwrap(),
                TerminalExecutionStatus::Complete(result) => {
                    assert_eq!(result, TerminalExecutionResult::Unit);
                    completed = true;
                    break;
                }
                other => panic!("unexpected execution {other:?}"),
            }
        }
        assert!(
            completed,
            "returning counter loop must finish within the fixture fuel bound"
        );
        assert_eq!(
            execution.structural_byte_array(73, &path),
            Some([if full { 67 } else { 41 }].as_slice())
        );
    }
}

#[test]
fn byte_input_exact_narrowing_uses_retained_payload_range_evidence() {
    let checked = checked_source(READ_ONE);
    let artifact = produce_terminal_artifact(&checked, "read_one")
        .expect("the selected declaration-bound payload proves exact byte narrowing");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert!(
        module.structural_types.iter().any(|declaration| {
            let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
                return false;
            };
            cases.iter().flat_map(|case| &case.fields).any(|field| {
                matches!(
                    field.field_type,
                    terminal_psi::StructuralFieldType::BoundedInteger(bounds)
                        if bounds.minimum() == semantic_vocabulary::IntegerValue::Signed(0)
                            && bounds.maximum() == semantic_vocabulary::IntegerValue::Signed(255)
                )
            })
        }),
        "canonical artifact retains the exact field range"
    );
}

#[test]
fn byte_input_exact_narrowing_rejects_a_state_annotation_without_field_bounds() {
    let source = READ_ONE.replace("case Byte(value: i32 [0..=255]);", "case Byte(value: i32);");
    let tokens = Lexer::new(&source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let diagnostics = lower_typed_trees(typed)
        .expect_err("state annotation cannot establish missing payload bounds");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("not provably within its declared range")
    }));
    assert!(diagnostics.iter().any(
        |diagnostic| diagnostic.message.contains("Exact integer cast")
            && diagnostic.message.contains("not provably representable")
    ));
}

#[test]
fn byte_input_exact_narrowing_preserves_forwarded_and_reordered_field_ranges() {
    let checked = checked_source(
        r#"
        data PairRead {
            case Eof;
            case Pair(low: i32 [0..=255], high: i32 [256..=511]);
        }
        boundary trait Input { machine read_pair() -> PairRead reaches Input; }
        machine read_one(out: &mut [u8]) reaches Input {
            transition out.len > 0 { true -> read(out) false -> done() }
            state read(out: &mut [u8]) {
                let observed: PairRead = Input::read_pair();
                transition observed {
                    PairRead::Pair { low, high } -> forward(out, high, low)
                    PairRead::Eof -> done()
                }
            }
            state forward(out: &mut [u8], high: i32, low: i32) {
                transition { _ -> store(out, low) }
            }
            state store(out: &mut [u8], value: i32) { out[0] = value as u8; }
            state done() {}
        }
    "#,
    );
    let _artifact = produce_terminal_artifact(&checked, "read_one")
        .expect("exact low-field bounds survive reversed case bindings and ordinary forwarding");
}

#[test]
fn byte_input_case_payload_and_borrowed_view_compose_in_a_write_cycle() {
    let checked = checked_source(include_str!(
        "../../../../../../tests/native-differential/tests/terminal_byte_views/byte_input.omg"
    ));
    let _artifact = produce_terminal_artifact(&checked, "classify_bytes")
        .expect("case payloads, ordinary scalar arguments and mutable views compose in a cycle");
}

#[test]
fn byte_input_case_roster_and_multiple_payloads_are_not_console_specific() {
    let checked = checked_source(
        r#"
        data Observation {
            case Empty;
            case One(value: u8);
            case Pair(first: u8, second: u8);
        }
        boundary trait Input {
            machine observe() -> Observation reaches Input;
        }
        machine collect(out: &mut [u8]) reaches Input {
            transition out.len > 0 { true -> read(out) false -> done() }
            state read(out: &mut [u8]) {
                let observed: Observation = Input::observe();
                transition observed {
                    Observation::Pair { first, second } -> combine(out, second, first)
                    Observation::One { value } -> store(out, value)
                    Observation::Empty -> done()
                }
            }
            state combine(out: &mut [u8], right: u8, left: u8) {
                out[0] = right ^ left;
            }
            state store(out: &mut [u8], value: u8) { out[0] = value; }
            state done() {}
        }
    "#,
    );
    let _artifact = produce_terminal_artifact(&checked, "collect")
        .expect("three cases and reversed payload positions use the same checked control path");
}

#[test]
fn byte_write_loop_publishes_fresh_guarded_writes() {
    let checked = checked_source(FILL);
    let _artifact = produce_terminal_artifact(&checked, "fill")
        .expect("a fresh guard proves every write and cursor advance");
}

fn entry_argument(artifact: &terminal_codec::CanonicalTerminalArtifact) -> TerminalStructuralValue {
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    TerminalStructuralValue {
        opaque_identity: 73,
        structural_type: entry.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    }
}

#[test]
fn byte_write_loop_fills_each_raw_prefix_once_across_fuel_suspension() {
    for initial in [vec![0x11], vec![0x11, 0x80, 0xff]] {
        for byte in [0, 65, 165] {
            let length = initial.len();
            let checked = checked_source(&format!(
                "{FILL}\ndata Record {{ out: [u8; {length}]; other: [u8; {length}]; }}\n\
                 machine Record::run(&mut self) {{ fill(&mut self.out, {byte}); }}"
            ));
            let artifact = produce_terminal_artifact(&checked, "Record::run")
                .expect("a raw array caller reaches the safety-checked fill loop");
            let path = vec![StructuralPathSegment::Field("out".into())];
            let sibling_path = vec![StructuralPathSegment::Field("other".into())];
            let sibling = vec![0x42; length];
            let arrays = [
                TerminalStructuralByteArrayValue {
                    argument_index: 0,
                    path: path.clone(),
                    bytes: initial.clone(),
                },
                TerminalStructuralByteArrayValue {
                    argument_index: 0,
                    path: sibling_path.clone(),
                    bytes: sibling.clone(),
                },
            ];
            let start = || {
                TerminalExecution::start_artifact_with_structural_arguments_and_byte_arrays(
                    artifact.semantic_bytes(),
                    artifact.proof_bytes(),
                    &proof_admission::AdmissionProfile::default(),
                    &[],
                    &[entry_argument(&artifact)],
                    &arrays,
                )
                .unwrap()
            };
            let mut execution = start();
            let mut fuel = TerminalFuelMeter::with_allowance(0);
            let mut prefixes = vec![initial.clone()];
            let mut completed = false;
            for _ in 0..512 {
                let status = execution.resume(&mut fuel).unwrap();
                let observed = execution.structural_byte_array(73, &path).unwrap().to_vec();
                assert_eq!(observed.len(), length);
                assert_eq!(
                    execution.structural_byte_array(73, &sibling_path),
                    Some(sibling.as_slice())
                );
                assert!(execution.effects().is_empty());
                if prefixes.last() != Some(&observed) {
                    prefixes.push(observed.clone());
                }
                match status {
                    TerminalExecutionStatus::SponsorExhausted(_) => {
                        let before = fuel.clone();
                        for _ in 0..2 {
                            assert!(matches!(
                                execution.resume(&mut fuel).unwrap(),
                                TerminalExecutionStatus::SponsorExhausted(_)
                            ));
                            assert_eq!(fuel, before, "unfunded resumption charges no work");
                            assert_eq!(
                                execution.structural_byte_array(73, &path),
                                Some(observed.as_slice())
                            );
                            assert_eq!(
                                execution.structural_byte_array(73, &sibling_path),
                                Some(sibling.as_slice())
                            );
                            assert!(execution.effects().is_empty());
                        }
                        fuel.replenish(1).unwrap();
                    }
                    TerminalExecutionStatus::Complete(result) => {
                        assert_eq!(result, TerminalExecutionResult::Unit);
                        completed = true;
                        break;
                    }
                    TerminalExecutionStatus::Crashed(crash) => {
                        panic!("unexpected crash: {crash:?}")
                    }
                }
            }
            assert!(
                completed,
                "finite fixture must finish; this is not a published termination certificate"
            );
            let expected = (0..=length)
                .map(|written| {
                    let mut prefix = initial.clone();
                    prefix[..written].fill(byte);
                    prefix
                })
                .collect::<Vec<_>>();
            assert_eq!(
                prefixes, expected,
                "each write advances exactly one prefix byte"
            );
            let mut generous = start();
            let mut generous_fuel = TerminalFuelMeter::with_allowance(10_000);
            assert_eq!(
                generous.resume(&mut generous_fuel).unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                generous.structural_byte_array(73, &path),
                execution.structural_byte_array(73, &path)
            );
            assert_eq!(
                generous.structural_byte_array(73, &sibling_path),
                Some(sibling.as_slice())
            );
            assert!(generous.effects().is_empty());
            assert_eq!(
                fuel.usage(),
                generous_fuel.usage(),
                "chunking preserves every semantic site's work"
            );
        }
    }
}

#[test]
fn byte_write_loop_empty_initialized_view_never_writes() {
    // Existing UTF-8 literal initialization supplies a genuinely empty live
    // field view. This is not a zero-length fixed-array declaration.
    let checked = checked_source(&format!(
        r#"
        {FILL}
        domain [u8; 3]::Utf8 requires valid_utf8(self);
        data Record {{ out: [u8; 3] in Utf8; other: [u8; 3] in Utf8; }}
        machine Record::run(&mut self) {{
            self.out = "";
            self.other = "QQ";
            fill(&mut self.out, 165);
        }}
    "#
    ));
    let artifact =
        produce_terminal_artifact(&checked, "Record::run").expect("empty initialized view caller");
    let argument = entry_argument(&artifact);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == argument.structural_type)
        .unwrap();
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        panic!("record caller");
    };
    let [out, other] = ["out", "other"].map(|identity| {
        fields
            .iter()
            .find(|field| field.identity == identity)
            .unwrap()
            .id
    });
    let writes = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::ByteSequenceWrite { .. }))
        .map(|operation| operation.id)
        .collect::<Vec<_>>();
    assert!(
        !writes.is_empty(),
        "the untaken write must remain in the verified artifact"
    );
    let start = || {
        TerminalExecution::start_artifact_with_structural_arguments(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
            std::slice::from_ref(&argument),
        )
        .unwrap()
    };
    let mut execution = start();
    let mut fuel = TerminalFuelMeter::with_allowance(0);
    let mut completed = false;
    for _ in 0..128 {
        match execution.resume(&mut fuel).unwrap() {
            TerminalExecutionStatus::SponsorExhausted(_) => {
                let before = fuel.clone();
                let storage = [out, other].map(|field| {
                    execution
                        .structural_byte_sequence_field(73, &[], field)
                        .map(<[u8]>::to_vec)
                });
                assert!(matches!(
                    execution.resume(&mut fuel).unwrap(),
                    TerminalExecutionStatus::SponsorExhausted(_)
                ));
                assert_eq!(fuel, before);
                assert_eq!(
                    [out, other].map(|field| execution
                        .structural_byte_sequence_field(73, &[], field)
                        .map(<[u8]>::to_vec)),
                    storage
                );
                assert!(execution.effects().is_empty());
                fuel.replenish(1).unwrap();
            }
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, TerminalExecutionResult::Unit);
                completed = true;
                break;
            }
            TerminalExecutionStatus::Crashed(crash) => panic!("unexpected crash: {crash:?}"),
        }
    }
    assert!(completed);
    assert_eq!(
        execution.structural_byte_sequence_field(73, &[], out),
        Some(&[][..])
    );
    assert_eq!(
        execution.structural_byte_sequence_field(73, &[], other),
        Some(b"QQ".as_slice())
    );
    assert!(execution.effects().is_empty());
    assert!(writes.iter().all(|operation| {
        fuel.usage()
            .at(FuelChargeSite::Operation(*operation))
            .is_none()
    }));
    let mut generous = start();
    let mut generous_fuel = TerminalFuelMeter::with_allowance(10_000);
    assert_eq!(
        generous.resume(&mut generous_fuel).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(
        generous.structural_byte_sequence_field(73, &[], out),
        Some(&[][..])
    );
    assert_eq!(
        generous.structural_byte_sequence_field(73, &[], other),
        Some(b"QQ".as_slice())
    );
    assert!(generous.effects().is_empty());
    assert_eq!(fuel.usage(), generous_fuel.usage());
}
