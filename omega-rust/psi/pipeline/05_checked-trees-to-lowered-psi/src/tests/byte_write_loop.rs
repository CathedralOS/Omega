//! A fresh guard proves each indexed write without a termination claim.
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalStructuralByteArrayValue, TerminalStructuralValue,
};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::{StructuralPathSegment, StructuralTypeShape};

#[test]
fn scalar_case_return_preserves_authored_multifield_identity_and_rejects_plan_drift() {
    let checked = crate::front_end::checked_program(
        r#"
        data PairResult { case Empty; case Pair(left: u64, right: u64); }
        machine pair(left: u64, right: u64) -> PairResult {
            PairResult::Pair { right: right, left: left }
        }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("pair"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact();
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
        TerminalStructuralInputs::default(),
    )
    .unwrap();
    let TerminalExecutionStatus::Complete(TerminalExecutionResult::ScalarCase(result)) = execution
        .resume(
            &mut TerminalFuelMeter::with_allowance(100),
            &mut AcceptTerminalEffects,
        )
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
    let plan = &checked.facts.flow.terminal_unit_effects.composed_machines[0];
    let value = plan.states[0]
        .operations
        .iter()
        .find_map(|operation| match operation {
            checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                value,
                ..
            } => Some(*value),
            _ => None,
        })
        .expect("returned case has a generic structural producer");
    let checked_trees::CheckedStructuralValueKind::Case(construction) =
        &checked.facts.values.structural_values.nodes.get(value).kind
    else {
        panic!("case construction missing");
    };
    let original_fields = construction.fields;
    assert_eq!(original_fields.len(), 2);
    let owner = &checked.data_definitions()[0];
    let empty = checked
        .data_members(owner)
        .iter()
        .find_map(|member| match member {
            checked_trees::data::DataMember::Variant(case)
                if checked.data_payload_fields(case).is_empty() =>
            {
                Some(case.symbol)
            }
            _ => None,
        })
        .expect("same-owner Empty case");
    for corruption in 0..6 {
        let mut changed = checked.clone();
        let checked_trees::CheckedStructuralValueKind::Case(construction) = &mut changed
            .facts
            .values
            .structural_values
            .nodes
            .get_mut(value)
            .kind
        else {
            panic!("case construction missing");
        };
        let fields = changed
            .facts
            .values
            .scalar_computations
            .case_fields
            .span_mut(original_fields)
            .unwrap();
        match corruption {
            0 => fields.swap(0, 1),
            1 => fields[0].symbol = fields[1].symbol,
            2 => fields[0].value = fields[1].value,
            3 => construction.case = empty,
            4 => {
                construction.fields = arena::HandleSpan::from_parts(original_fields.start(), 1);
            }
            _ => {
                changed.facts.flow.terminal_unit_effects.composed_machines[0].result =
                    checked_trees::CheckedControlResultPlan::Unit;
            }
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(
                &changed,
                terminal_production::TerminalMachineSelection::Name("pair")
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default()
            ))
            .is_err(),
            "corruption {corruption}"
        );
    }
}

#[test]
fn scalar_case_return_multistate_borrowed_view_and_ordinary_call_observe_count() {
    let checked = crate::front_end::checked_program(
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
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Record::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("ordinary scalar-case call composes with view and count transfers")
    .into_artifact();
    for full in [false, true] {
        let path = vec![StructuralPathSegment::Field("out".into())];
        let mut execution = TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[terminal_interpreter::TerminalScalarValue::Boolean(full)],
            TerminalStructuralInputs {
                arguments: &[entry_argument(&artifact)],
                byte_arrays: &[TerminalStructuralByteArrayValue {
                    argument_index: 0,
                    path: path.clone(),
                    bytes: vec![19],
                }],
                ..Default::default()
            },
        )
        .unwrap();
        let mut fuel = TerminalFuelMeter::with_allowance(0);
        let mut completed = false;
        for _ in 0..512 {
            match execution
                .resume(&mut fuel, &mut AcceptTerminalEffects)
                .unwrap()
            {
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
fn same_named_case_payloads_preserve_identity_through_calls_and_interpretation() {
    let checked = crate::front_end::checked_program(
        r#"
        data Outcome {
            case Empty;
            case First(count: u64);
            case Second(count: u64);
            case Third(count: u64);
        }
        machine choose(selector: u64, count: u64) -> Outcome {
            transition selector == 0 { true -> empty() false -> first_test(selector, count) }
            state empty() -> Outcome { Outcome::Empty }
            state first_test(selector: u64, count: u64) -> Outcome {
                transition selector == 1 { true -> first(count) false -> second_test(selector, count) }
            }
            state first(count: u64) -> Outcome { Outcome::First { count: count } }
            state second_test(selector: u64, count: u64) -> Outcome {
                transition selector == 2 { true -> second(count) false -> third(count) }
            }
            state second(count: u64) -> Outcome { Outcome::Second { count: count } }
            state third(count: u64) -> Outcome { Outcome::Third { count: count } }
        }
        machine collect(out: &mut [u8], selector: u64, expected: u64) {
            let observed: Outcome = choose(selector, expected);
            transition observed {
                Outcome::Third { count } -> check(out, expected, count, 33)
                Outcome::Empty -> writable(out, 0)
                Outcome::Second { count } -> check(out, expected, count, 22)
                Outcome::First { count } -> check(out, expected, count, 11)
            }
            state check(out: &mut [u8], expected: u64, actual: u64, marker: u8) {
                transition expected == actual { true -> writable(out, marker) false -> writable(out, 255) }
            }
            state writable(out: &mut [u8], marker: u8) {
                transition out.len > 0 { true -> store(out, marker) false -> done() }
            }
            state store(out: &mut [u8], marker: u8) { out[0] = marker; }
            state done() {}
        }
        data Record { out: [u8; 1]; }
        machine Record::run(&mut self, selector: u64, count: u64) {
            collect(&mut self.out, selector, count);
        }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Record::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("same-named payloads retain distinct case identities in Terminal")
    .into_artifact();
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
    let path = vec![StructuralPathSegment::Field("out".into())];
    for (selector, marker) in [(0, 0), (1, 11), (2, 22), (3, 33)] {
        for count in [0, 7, u64::MAX] {
            let arguments =
                [selector, count].map(|value| terminal_interpreter::TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    value: IntegerValue::Unsigned(u128::from(value)),
                });
            let mut execution = TerminalExecution::start_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &proof_admission::AdmissionProfile::default(),
                &arguments,
                TerminalStructuralInputs {
                    arguments: &[entry_argument(&artifact)],
                    byte_arrays: &[TerminalStructuralByteArrayValue {
                        argument_index: 0,
                        path: path.clone(),
                        bytes: vec![19],
                    }],
                    ..Default::default()
                },
            )
            .expect("independently verify the reloaded artifact before execution");
            let result = execution
                .resume(
                    &mut TerminalFuelMeter::with_allowance(512),
                    &mut AcceptTerminalEffects,
                )
                .unwrap();
            assert_eq!(
                result,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                execution.structural_byte_array(73, &path),
                Some([marker].as_slice())
            );
        }
    }
}

#[test]
fn byte_input_exact_narrowing_preserves_forwarded_and_reordered_field_ranges() {
    let checked = crate::front_end::checked_program(
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
    let _artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("read_one"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("exact low-field bounds survive reversed case bindings and ordinary forwarding")
    .into_artifact();
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
