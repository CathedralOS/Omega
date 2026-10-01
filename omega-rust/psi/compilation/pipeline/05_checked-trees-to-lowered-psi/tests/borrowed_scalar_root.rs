//! Scalar-result roots share primitive referents with their computed callees.

use checked_trees::CheckedTrees;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};
use terminal_psi::{OperationKind, StructuralAccess, TerminalMachineResult};

const SOURCE: &str = r#"
machine stamp(value: &mut u64, number: u64) -> u64 { value = number; number }
machine first(left: u64, right: u64) -> u64 { left }
machine enter(before: u64, slot: &mut u64, after: u64) -> u64 {
    let answer: u64 = first(stamp(&mut slot, before), stamp(&mut slot, after));
    answer
}
"#;

#[test]
fn mutable_primitive_state_loans_store_and_forward_with_exact_access() {
    let source = r#"
        data Owner { value: u64; }
        machine Owner::enter(&mut self) {
            transition { _ -> first(&mut self.value) }
            state first(&mut self, value: &mut u64) {
                value = ((value as u64 in Wrapping) + 1) as u64;
                transition { _ -> second(value) }
            }
            state second(&mut self, value: &mut u64) {
                value = ((value as u64 in Wrapping) + 1) as u64;
                transition value < 70 {
                    true -> first(value)
                    false -> done()
                }
            }
            state done(&mut self) {}
        }
    "#;
    let checked = crate::front_end::checked_program(source);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("Owner::enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("primitive state loans retain their mutable storage")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile)
        .expect("decoded mutable loans independently verify");

    // Neither an invocation nor a block may lend authority it does not hold.
    for invocation in [true, false] {
        let mut changed = module.clone();
        let parameter = if invocation {
            changed
                .machines
                .iter_mut()
                .flat_map(|machine| &mut machine.structural_parameters)
                .find(|parameter| parameter.access == StructuralAccess::MutableBorrow)
        } else {
            changed
                .machines
                .iter_mut()
                .flat_map(|machine| &mut machine.blocks)
                .flat_map(|block| &mut block.structural_parameters)
                .find(|parameter| parameter.access == StructuralAccess::MutableBorrow)
        }
        .unwrap_or_else(|| panic!("source retains loan declaration (invocation={invocation})"));
        parameter.access = StructuralAccess::SharedBorrow;
        assert!(
            terminal_verifier::verify_module(&changed, &proof, &profile).is_err(),
            "shared storage cannot be written or forwarded exclusively"
        );
    }
}

#[test]
fn state_prefix_reads_primitive_loans_and_local_windows_keep_custody() {
    let source = r#"
        boundary trait Output { machine count(value: u64) reaches Output; }
        machine consume(bytes: &[u8]) reaches Output { Output::count(bytes.len); }
        machine enter(value: &u64) reaches Output {
            let bytes: [u8; 3];
            consume(&bytes[..]);
            transition { _ -> snapshot(value) }
            state snapshot(value: &u64) {
                let saved: u64 = value;
                Output::count(saved);
            }
        }
    "#;
    let checked = crate::front_end::checked_program(source);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("local windows and primitive state parameters compose")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile)
        .expect("decoded artifact independently verifies");
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let mut changed = module.clone();
        let argument = changed
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
            .filter_map(|operation| match &mut operation.kind {
                OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } => Some(structural_arguments),
                _ => None,
            })
            .flatten()
            .find(|argument| {
                argument.path.iter().any(|segment| {
                    matches!(
                        segment,
                        terminal_psi::StructuralPathSegment::FixedByteRange { .. }
                    )
                })
            })
            .expect("the call retains its local byte-window path");
        argument.access = access;
        if access == StructuralAccess::SharedBorrow {
            *argument.path.last_mut().unwrap() =
                terminal_psi::StructuralPathSegment::FixedByteRange { start: 0, end: 4 };
        }
        assert!(
            terminal_verifier::verify_module(&changed, &proof, &profile).is_err(),
            "a local window cannot extend its backing or change its formal access"
        );
    }
}

#[test]
fn scalar_root_known_guard_result_preserves_left_effects() {
    let source = r#"
        machine stamp(value: &mut u64, number: u64, answer: bool) -> bool {
            value = number;
            answer
        }
        machine first(left: bool, right: bool) -> bool { left }
        machine enter(enabled: bool, slot: &mut u64, number: u64) -> bool {
            let answer: bool = RESULT;
            TAIL
            answer
        }
    "#;
    for (guard, known_result, writes_guard) in [
        ("(stamp(&mut slot, number, enabled) && false)", false, true),
        ("(stamp(&mut slot, number, enabled) || true)", true, true),
        ("!(stamp(&mut slot, number, enabled) && false)", true, true),
        ("!(stamp(&mut slot, number, enabled) || true)", false, true),
        (
            "((stamp(&mut slot, number, enabled) && false) && enabled)",
            false,
            true,
        ),
        (
            "((stamp(&mut slot, number, enabled) || true) || enabled)",
            true,
            true,
        ),
        ("(enabled && false)", false, false),
        ("(enabled || true)", true, false),
        ("!(enabled && false)", true, false),
        ("!(enabled || true)", false, false),
    ] {
        for conjunction in [false, true] {
            let invokes_right = known_result == conjunction;
            let result = if conjunction { known_result } else { true };
            for (result_expression, tail, machines) in [
                (
                    "GUARD OPERATOR stamp(&mut slot, 11u64, true)",
                    "let written: bool = stamp(&mut slot, 13u64, true);",
                    2,
                ),
                (
                    "first(GUARD OPERATOR stamp(&mut slot, 11u64, true), stamp(&mut slot, 13u64, true))",
                    "",
                    3,
                ),
            ] {
                let source = source
                    .replace("RESULT", result_expression)
                    .replace("TAIL", tail)
                    .replace("GUARD", guard)
                    .replace("OPERATOR", if conjunction { "&&" } else { "||" });
                for enabled in [false, true] {
                    execute(
                        &source,
                        &[TerminalScalarValue::Boolean(enabled), unsigned(7)],
                        ExecutionExpectations {
                            result: TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(
                                result,
                            )),
                            observations: match (writes_guard, invokes_right) {
                                (true, true) => &[201, 7, 11, 13],
                                (true, false) => &[201, 7, 13],
                                (false, true) => &[201, 11, 13],
                                (false, false) => &[201, 13],
                            },
                            stamp_calls: 1 + usize::from(writes_guard) + usize::from(invokes_right),
                            stamp_invocations: 1
                                + u64::from(writes_guard)
                                + u64::from(invokes_right),
                            machines,
                        },
                    );
                }
            }
        }
    }
}

#[test]
fn scalar_root_strict_guard_retains_flow_call_correspondence() {
    let source = r#"
        machine stamp(value: &mut u64, number: u64, answer: bool) -> bool {
            value = number;
            answer
        }
        machine enter(enabled: bool, slot: &mut u64, number: u64) -> bool {
            let answer: bool = ((stamp(&mut slot, number, enabled) && false) == false)
                || stamp(&mut slot, 11u64, true);
            let written: bool = stamp(&mut slot, 13u64, true);
            answer
        }
    "#;
    for enabled in [false, true] {
        execute(
            source,
            &[TerminalScalarValue::Boolean(enabled), unsigned(7)],
            ExecutionExpectations {
                result: TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true)),
                observations: &[201, 7, 13],
                stamp_calls: 3,
                stamp_invocations: 2,
                machines: 2,
            },
        );
    }
}

#[test]
fn scalar_root_comparison_guards_preserve_skipped_and_evaluated_calls() {
    for (condition, constant) in [
        ("1u64 > 2u64", false),
        ("2u64 >= 1u64", true),
        ("2u64 < 1u64", false),
        ("1u64 <= 2u64", true),
        ("1u64 == 2u64", false),
        ("1u64 != 2u64", true),
        ("(1u64 as u8) > 2u8", false),
        ("(1u64 + 1u64) == 2u64", true),
        ("!((1u64 > 2u64) && enabled)", true),
        ("!((1u64 < 2u64) || enabled)", false),
    ] {
        for conjunction in [false, true] {
            let operator = if conjunction { "&&" } else { "||" };
            let source = format!(
                r#"
                machine stamp(value: &mut u64, number: u64) -> bool {{ value = number; true }}
                machine enter(enabled: bool, slot: &mut u64, number: u64) -> bool {{
                    let answer: bool = (({condition}) {operator} enabled)
                        {operator} stamp(&mut slot, number);
                    let written: bool = stamp(&mut slot, number);
                    answer
                }}
                "#,
            );
            for enabled in [false, true] {
                let invokes_first = if conjunction {
                    constant && enabled
                } else {
                    !constant && !enabled
                };
                execute(
                    &source,
                    &[TerminalScalarValue::Boolean(enabled), unsigned(7)],
                    ExecutionExpectations {
                        result: TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(
                            !conjunction || invokes_first,
                        )),
                        observations: &[201, 7],
                        stamp_calls: 1 + usize::from(constant == conjunction),
                        stamp_invocations: 1 + u64::from(invokes_first),
                        machines: 2,
                    },
                );
            }
        }
    }
}

fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

struct ExecutionExpectations<'values> {
    result: TerminalExecutionResult,
    observations: &'values [u128],
    stamp_calls: usize,
    stamp_invocations: u64,
    machines: usize,
}

fn execute(
    source: &str,
    arguments: &[TerminalScalarValue],
    expected: ExecutionExpectations<'_>,
) -> terminal_psi::TerminalModule {
    let checked = crate::front_end::checked_program(source);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("mixed root publishes its complete borrowed scalar call closure")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile)
        .expect("reloaded scalar root artifact verifies independently");
    assert_eq!(module.machines.len(), expected.machines);
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_eq!(caller.parameters.len(), arguments.len());
    assert_eq!(caller.structural_parameters.len(), 1);
    let parameter = &caller.structural_parameters[0];
    assert_eq!(parameter.access, StructuralAccess::MutableBorrow);
    assert_eq!(
        parameter.position, 0,
        "structural inputs have dense positions"
    );
    match &expected.result {
        TerminalExecutionResult::Scalar(_) => {
            assert!(matches!(caller.result, TerminalMachineResult::Scalar(_)));
        }
        TerminalExecutionResult::Unit => assert_eq!(caller.result, TerminalMachineResult::Unit),
        other => panic!("unexpected fixture result: {other:?}"),
    }
    let mut stamp_bodies = module.machines.iter().filter(|machine| {
        matches!(machine.result, TerminalMachineResult::Scalar(_))
            && machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::WriteOnlyPrimitiveStore { .. }
                    )
                })
    });
    let stamp = stamp_bodies.next().expect("scalar mutator body");
    assert!(
        stamp_bodies.next().is_none(),
        "stamp has one shared callee identity"
    );
    let stamp_calls = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(operation.kind,
            OperationKind::CallStructuralScalar { callee, .. } if callee == stamp.id)
        })
        .collect::<Vec<_>>();
    assert_eq!(stamp_calls.len(), expected.stamp_calls);
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &profile,
        arguments,
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 71,
                structural_type: parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            primitive_values: &[TerminalStructuralPrimitiveValue {
                argument_index: 0,
                value: unsigned(201),
            }],
            ..Default::default()
        },
    )
    .unwrap();
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    let mut observations = Vec::new();
    let mut completed = false;
    for _ in 0..128 {
        let status = execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap();
        let referents = execution.structural_primitive_values();
        assert_eq!(referents.len(), 1);
        assert_eq!(referents[0].argument_index, 0);
        observations.push(referents[0].value);
        match status {
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, expected.result);
                completed = true;
                break;
            }
            TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
            other => panic!("unexpected scalar root execution: {other:?}"),
        }
    }
    assert!(
        completed,
        "scalar root must finish with one-unit replenishments"
    );
    observations.dedup();
    assert_eq!(
        observations,
        expected
            .observations
            .iter()
            .copied()
            .map(unsigned)
            .collect::<Vec<_>>()
    );
    let mut invocations = std::collections::BTreeMap::from([(module.entry, 1)]);
    for operation in module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
    {
        let callee = match operation.kind {
            OperationKind::Call { callee, .. }
            | OperationKind::CallUnit { callee, .. }
            | OperationKind::CallStructuralScalar { callee, .. } => callee,
            _ => continue,
        };
        if let Some(usage) = meter
            .usage()
            .at(terminal_fuel::FuelChargeSite::Operation(operation.id))
        {
            *invocations.entry(callee).or_default() += usage.executions();
        }
    }
    assert_eq!(
        invocations.get(&stamp.id).copied().unwrap_or(0),
        expected.stamp_invocations
    );
    for machine in &module.machines {
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            let usage = meter
                .usage()
                .at(terminal_fuel::FuelChargeSite::Operation(operation.id));
            if machine.id == stamp.id {
                assert_eq!(
                    usage.map_or(0, |usage| usage.executions()),
                    expected.stamp_invocations,
                    "each stamp body operation executes once per selected invocation"
                );
            }
            if let Some(usage) = usage {
                assert_eq!(
                    usage.executions(),
                    invocations[&machine.id],
                    "completed operation must not replay across fuel pauses"
                );
                if matches!(
                    operation.kind,
                    OperationKind::CallStructuralScalar { .. }
                        | OperationKind::WriteOnlyPrimitiveStore { .. }
                ) {
                    assert_eq!(
                        usage.units(),
                        usage.executions(),
                        "one unit per call or store"
                    );
                }
            }
        }
    }
    module
}

fn publish_original(source: &str) -> CheckedTrees {
    let checked = crate::front_end::checked_program(source);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("unmodified scalar root must publish before testing custody mutations")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("unmodified artifact verifies independently");
    checked
}

fn reject(checked: &CheckedTrees, mutation: &str) {
    assert!(
        terminal_production::TerminalProductionRequest::new(
            checked,
            TerminalMachineSelection::Name("enter")
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default()
        ))
        .is_err(),
        "publication accepted scalar root custody mutation: {mutation}"
    );
}

#[test]
fn scalar_root_rejects_missing_callee_type_custody() {
    let mut changed = publish_original(SOURCE);
    let types = &mut changed
        .facts
        .flow
        .terminal_structural_scalar_returns
        .structural_types;
    assert!(
        !types.is_empty(),
        "stamp retains its primitive referent type"
    );
    types.clear();
    reject(&changed, "missing selected callee structural types");
}
