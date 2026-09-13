//! A returned reference retains its caller's referent across call completion.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_fuel::{FuelChargeSite, TerminalFuelMeter};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use terminal_psi::{OperationKind, Terminator};

fn artifact(prefix: &str) -> terminal_codec::CanonicalTerminalArtifact {
    let checked = checked(prefix);
    terminal_production::TerminalProductionRequest::new(&checked, "exercise")
        .produce_artifact()
        .expect("ordinary sequencing must retain the returned reference and its source loan")
}

fn checked(prefix: &str) -> checked_trees::CheckedTrees {
    let source = format!(
        "machine mark(value: &mut i32) {{ value = 11; }}
         machine relay(value: &mut i32) -> &mut i32 {{ {prefix} value }}
         machine replace(value: &mut i32) {{ value = 29; }}
         machine exercise(value: &mut i32) -> i32 {{
             let held: &mut i32 = relay(value);
             replace(held);
             value
         }}"
    );
    typed_trees_to_checked_trees::lower_typed_trees(typed(&source))
        .unwrap_or_else(|diagnostics| panic!("reference-result checking: {diagnostics:#?}"))
}

fn typed(source: &str) -> typed_trees::TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("reference-result tokens");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("reference-result syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("reference-result resolution");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("reference-result typing")
}

fn signed(value: i128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Signed, 32).expect("i32 carrier"),
        value: IntegerValue::Signed(value),
    }
}

#[test]
fn reference_result_preserves_original_storage_through_fuel_resumption() {
    execute(&artifact(""), 1);
}

#[test]
fn reference_result_composes_with_an_ordinary_call_before_return() {
    execute(&artifact("mark(value);"), 2);
}

#[test]
fn stored_reference_result_still_requires_terminal_custody() {
    let source = "data View { body: &mut i32; }
        machine make_view(value: &mut i32) -> View { View { body: value } }
        machine replace(value: &mut i32) { value = 29; }
        machine exercise(value: &mut i32) -> i32 {
            let held: View = make_view(value);
            replace(held.body);
            value
        }";
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed(source))
        .unwrap_or_else(|diagnostics| panic!("stored-reference checking: {diagnostics:#?}"));
    // Source forwarding is legal, but type correctness must not substitute for
    // the missing aggregate leaf transfers in independently verified Terminal.
    // Replace this fence with execute(&artifact, 1) when those transfers exist.
    let error = terminal_production::TerminalProductionRequest::new(&checked, "exercise")
        .produce_artifact()
        .expect_err("stored-reference transport still needs a real Terminal producer");
    assert!(
        matches!(
            error,
            terminal_production::TerminalArtifactProductionError::Lowering(
                checked_trees_to_lowered_psi::LoweringError::Unsupported(
                    "machine has no source-independent checked scalar control plan"
                )
            )
        ),
        "{error:?}"
    );
}

#[test]
fn reference_result_rejects_conflicting_access_before_last_use() {
    let program = typed(
        "machine relay(value: &mut i32) -> &mut i32 { value }
        machine replace(value: &mut i32) { value = 29; }
        machine exercise(value: &mut i32) -> i32 {
            let held: &mut i32 = relay(value);
            replace(value);
            replace(held);
            value
        }",
    );
    assert!(
        typed_trees_to_checked_trees::lower_typed_trees(program).is_err(),
        "a live returned loan excludes direct access to its original referent"
    );
}

#[test]
fn reference_result_rejects_changed_source_loan_and_weakening() {
    use checked_trees::{
        CheckedUnitEffectOperationPlan as Operation,
        CheckedUnitStructuralArgumentSourcePlan as Source,
    };
    let original = checked("");
    let _ = terminal_production::TerminalProductionRequest::new(&original, "exercise")
        .produce_artifact()
        .expect("untampered reference custody");
    for mutation in 0..7 {
        let mut changed = original.clone();
        let plans = &mut changed.facts.flow.terminal_unit_effects.machines;
        match mutation {
            0 => {
                let machine = plans
                    .iter_mut()
                    .find(|machine| machine.structural_result.is_some())
                    .expect("reference-returning helper");
                let result = machine.structural_result.as_mut().unwrap();
                result.reference_sources[0].source.source =
                    Source::Parameter { parameter_index: 1 };
                let Operation::EstablishReference { source, .. } = machine
                    .operations
                    .iter_mut()
                    .find(|operation| matches!(operation, Operation::EstablishReference { .. }))
                    .unwrap()
                else {
                    unreachable!()
                };
                source.source = Source::Parameter { parameter_index: 1 };
            }
            1 => {
                let Operation::StructuralCall { custody, .. } = plans.iter_mut().flat_map(|machine| &mut machine.operations)
                    .find(|operation| matches!(operation, Operation::StructuralCall { custody, .. } if custody.reference_loan.is_valid())).unwrap() else { unreachable!() };
                custody.reference_loan = arena::Handle::invalid();
            }
            2 => {
                let Operation::ReleaseReference {
                    statement_index, ..
                } = plans
                    .iter_mut()
                    .flat_map(|machine| &mut machine.operations)
                    .find(|operation| matches!(operation, Operation::ReleaseReference { .. }))
                    .unwrap()
                else {
                    unreachable!()
                };
                *statement_index -= 1;
            }
            3 => {
                let machine = plans
                    .iter_mut()
                    .find(|machine| {
                        machine.operations.iter().any(|operation| {
                            matches!(operation, Operation::ReleaseReference { .. })
                        })
                    })
                    .unwrap();
                machine
                    .operations
                    .retain(|operation| !matches!(operation, Operation::ReleaseReference { .. }));
            }
            4 => {
                let loan = plans
                    .iter()
                    .flat_map(|machine| &machine.operations)
                    .find_map(|operation| match operation {
                        Operation::StructuralCall { custody, .. }
                            if custody.reference_loan.is_valid() =>
                        {
                            Some(custody.reference_loan)
                        }
                        _ => None,
                    })
                    .unwrap();
                changed.facts.borrow.loans.get_mut(loan).root_symbol =
                    symbols::SymbolHandle::invalid();
            }
            5 => {
                let loan = plans
                    .iter()
                    .flat_map(|machine| &machine.operations)
                    .find_map(|operation| match operation {
                        Operation::StructuralCall { custody, .. }
                            if custody.reference_loan.is_valid() =>
                        {
                            Some(custody.reference_loan)
                        }
                        _ => None,
                    })
                    .unwrap();
                let weakening = changed
                    .facts
                    .flow
                    .borrow_lifetimes
                    .weakenings
                    .iter()
                    .find_map(|(handle, weakening)| (weakening.loan == loan).then_some(handle))
                    .unwrap();
                changed
                    .facts
                    .flow
                    .borrow_lifetimes
                    .weakenings
                    .get_mut(weakening)
                    .reason = checked_trees::FlowBorrowWeakeningReason::LocalReassigned;
            }
            6 => {
                let machine = plans
                    .iter_mut()
                    .find(|machine| {
                        machine.operations.iter().any(|operation| {
                            matches!(operation, Operation::ReleaseReference { .. })
                        })
                    })
                    .unwrap();
                let release_index = machine
                    .operations
                    .iter()
                    .position(|operation| matches!(operation, Operation::ReleaseReference { .. }))
                    .unwrap();
                let release = machine.operations.remove(release_index);
                machine.operations.insert(release_index - 1, release);
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "exercise")
                .produce_artifact()
                .is_err(),
            "source-custody mutation {mutation} must reject"
        );
    }
}

fn execute(artifact: &terminal_codec::CanonicalTerminalArtifact, expected_stores: usize) {
    let module = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("decode reference-result module");
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes())
        .expect("decode reference-result proof");
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile)
        .expect("independently verify decoded reference custody");
    assert_eq!(
        terminal_codec::encode_module(&module).expect("canonical reference-result encoding"),
        artifact.semantic_bytes()
    );
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("caller entry");
    let [parameter] = entry.structural_parameters.as_slice() else {
        panic!("one original primitive referent");
    };
    let consumer_calls = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::CallUnit { .. }))
        .map(|operation| operation.id)
        .collect::<Vec<_>>();
    let [consumer_call] = consumer_calls.as_slice() else {
        panic!("one ordinary caller-side write through the returned reference");
    };
    let reference_returns = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .filter_map(|block| match block.terminator {
            Terminator::ReturnStructural { edge, .. } => Some(edge),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [reference_return] = reference_returns.as_slice() else {
        panic!("relay must retain one real structural-result return");
    };
    let stores = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::WriteOnlyPrimitiveStore { .. }
            )
        })
        .map(|operation| operation.id)
        .collect::<Vec<_>>();
    assert_eq!(stores.len(), expected_stores, "retain every authored store");

    let initial = TerminalStructuralPrimitiveValue {
        argument_index: 0,
        value: signed(7),
    };
    let written = TerminalStructuralPrimitiveValue {
        argument_index: 0,
        value: signed(29),
    };
    let mut execution =
        TerminalExecution::start_artifact_with_structural_arguments_and_primitive_values(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &profile,
            &[],
            &[TerminalStructuralValue {
                opaque_identity: 91,
                structural_type: parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            &[initial],
        )
        .expect("install original initialized referent");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution.resume(&mut meter).expect("initial suspension"),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(execution.structural_primitive_values(), vec![initial]);
    let mut complete = false;
    for _ in 0..256 {
        meter.replenish(1).expect("one more execution unit");
        let status = execution.resume(&mut meter).expect("resume reference call");
        if meter
            .usage()
            .at(FuelChargeSite::Operation(*consumer_call))
            .is_some()
        {
            assert!(
                meter
                    .usage()
                    .at(FuelChargeSite::Edge(*reference_return))
                    .is_some(),
                "the returned reference is unavailable to its consumer before successful return"
            );
        }
        let executions = stores
            .iter()
            .map(|store| {
                meter
                    .usage()
                    .at(FuelChargeSite::Operation(*store))
                    .map_or(0, |attribution| attribution.executions())
            })
            .collect::<Vec<_>>();
        assert!(
            executions.iter().all(|count| *count <= 1),
            "no replayed store"
        );
        if executions.iter().all(|count| *count == 1) {
            assert!(
                meter
                    .usage()
                    .at(FuelChargeSite::Edge(*reference_return))
                    .is_some(),
                "the result consumer cannot execute before relay returns"
            );
            assert_eq!(execution.structural_primitive_values(), vec![written]);
        }
        match status {
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, TerminalExecutionResult::Scalar(signed(29)));
                assert!(executions.iter().all(|count| *count == 1));
                complete = true;
                break;
            }
            TerminalExecutionStatus::SponsorExhausted(_) => {
                let usage = meter.usage().clone();
                let storage = execution.structural_primitive_values();
                assert!(matches!(
                    execution
                        .resume(&mut meter)
                        .expect("retry exhausted execution"),
                    TerminalExecutionStatus::SponsorExhausted(_)
                ));
                assert_eq!(meter.usage(), &usage, "an unpaid retry performs no work");
                assert_eq!(execution.structural_primitive_values(), storage);
            }
            other => panic!("unexpected reference-result execution status: {other:?}"),
        }
    }
    assert!(complete, "finite reference-result program must complete");
    assert_eq!(execution.structural_primitive_values(), vec![written]);
    let usage = meter.usage().clone();
    assert_eq!(
        execution
            .resume(&mut meter)
            .expect("completed execution is stable"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(signed(29)))
    );
    assert_eq!(meter.usage(), &usage);
    assert_eq!(execution.structural_primitive_values(), vec![written]);
}
