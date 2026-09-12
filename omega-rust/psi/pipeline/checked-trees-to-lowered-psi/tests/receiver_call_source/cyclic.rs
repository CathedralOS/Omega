//! Ordinary projected calls retain a looping callee and the caller continuation.
use super::*;

const SOURCE: &str = r#"
boundary trait Observe { machine record(value: u64) reaches Observe; }
data Child { value: u64; }
data Root { child: Child; }
machine identity(value: u64) -> u64 { value }

machine Child::walk(&mut self, remaining: u64)
RANKING
{
    self.value = 7;
    transition remaining > 0 {
        true -> walk(remaining - 1)
        false -> done()
    }
    state done(&mut self) {}
}

machine Child::observe(&mut self) reaches Observe {
    Observe::record(self.value);
}

machine Root::enter(&mut self) reaches Observe {
    self.child.walk(3);
    self.child.observe();
}
"#;

fn produce(source: &str) -> terminal_codec::CanonicalTerminalArtifact {
    let checked = checked_from_source(source);
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Root::enter")
        .produce_artifact()
        .expect("projected looping callee and caller continuation publish");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("ordinary source-produced cyclic receiver verifies");
    artifact
}

#[test]
fn projected_unranked_callee_preserves_caller_observation() {
    for (expression, expected) in [
        ("self.value", 7),
        ("self.value ^ 1", 6),
        ("identity(self.value)", 7),
    ] {
        let artifact = produce(
            &SOURCE
                .replace("RANKING", "")
                .replace("record(self.value)", &format!("record({expression})")),
        );
        assert_observations(&artifact, &[vec![integer(expected)]]);
    }
}

#[test]
fn receiver_free_observer_keeps_attachment_erasure() {
    let source = SOURCE
        .replace("RANKING", "")
        .replace("record(self.value)", "record(9)");
    let checked = checked_from_source(&source);
    let observer = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Child::observe")
        .unwrap();
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == observer.symbol)
        .unwrap();
    assert!(plan.structural_parameters.is_empty());
    assert_observations(&produce(&source), &[vec![integer(9)]]);
}

#[test]
fn projected_boolean_observer_retains_receiver() {
    let source = SOURCE
        .replace("RANKING", "")
        .replace("record(value: u64)", "record(value: bool)")
        .replace("value: u64;", "value: bool;")
        .replace("self.value = 7", "self.value = true")
        .replace("record(self.value)", "record(self.value == true)");
    assert_observations(
        &produce(&source),
        &[vec![TerminalScalarValue::Boolean(true)]],
    );
}

#[test]
fn erased_observed_receiver_is_rejected_after_checking() {
    let mut checked = checked_from_source(&SOURCE.replace("RANKING", ""));
    let observer = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Child::observe")
        .unwrap()
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.machine == observer)
        .unwrap();
    assert_eq!(plan.structural_parameters.len(), 1);
    plan.structural_parameters.clear();
    assert!(
        terminal_production::TerminalProductionRequest::new(&checked, "Root::enter")
            .produce_artifact()
            .is_err()
    );
}

#[test]
fn natural_ranked_unit_callee_preserves_ordinary_projected_calls() {
    for (primitive, bits) in [("u8", 8), ("u16", 16), ("u32", 32), ("u64", 64)] {
        let artifact = produce(
            &SOURCE
                .replace("RANKING", "terminates by remaining -> Nat::Descending;")
                .replace("remaining: u64", &format!("remaining: {primitive}")),
        );
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
        assert_eq!(proof.control_cycles.len(), 1);
        assert!(module.machines.iter().any(|machine| matches!(
            machine.ranked_scc,
            Some(terminal_psi::TerminalRankedScc::Natural(_))
        )));
        assert!(module.machines.iter().all(|machine| {
            machine
                .ranked_scc
                .as_ref()
                .is_none_or(|rank| rank.as_unsigned_countdown().is_none())
        }));
        for machine in &module.machines {
            if let Some(terminal_psi::TerminalRankedScc::Natural(components)) = &machine.ranked_scc
            {
                assert!(components.iter().all(|component| {
                    component.rank_type
                        == semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            bits,
                        )
                        .unwrap()
                }));
            }
        }
        assert_observations(&artifact, &[vec![integer(7)]]);
    }
}

#[test]
fn natural_rank_subject_measure_and_carrier_cannot_be_substituted() {
    let original = checked_from_source(
        &SOURCE.replace("RANKING", "terminates by remaining -> Nat::Descending;"),
    );
    for corruption in ["missing", "measure", "carrier", "subject", "position"] {
        let mut checked = original.clone();
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter_mut()
            .find(|plan| !plan.natural_ranks.is_empty())
            .unwrap();
        match corruption {
            "missing" => plan.natural_ranks.clear(),
            "measure" => {
                plan.natural_ranks[0].measure =
                    checked_trees::CheckedNaturalRankMeasure::ByteSequenceLength
            }
            "carrier" => {
                plan.natural_ranks[0].measure =
                    checked_trees::CheckedNaturalRankMeasure::UnsignedParameter {
                        primitive_type: typed_trees::types::PrimitiveType::U32,
                    }
            }
            "subject" => plan.natural_ranks[0].parameter = symbols::SymbolHandle::invalid(),
            "position" => plan.natural_ranks[0].parameter_position = 0,
            _ => unreachable!(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&checked, "Root::enter")
                .produce_artifact()
                .is_err(),
            "{corruption}"
        );
    }
}

#[test]
fn natural_ranked_callee_rejects_missing_descent() {
    let source = SOURCE
        .replace("RANKING", "terminates by remaining -> Nat::Descending;")
        .replace("walk(remaining - 1)", "walk(remaining)");
    let tokens = Lexer::new(&source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    assert!(typed_trees_to_checked_trees::lower_typed_trees(typed).is_err());
}

#[test]
fn ranked_loop_retains_private_argument_evaluation_edges() {
    for ranking in ["", "terminates by remaining -> Nat::Descending;"] {
        let source = SOURCE
            .replace("record(value: u64)", "record(value: u64, selected: bool)")
            .replace("record(self.value)", "record(self.value, true)")
            .replace("RANKING", &format!("{ranking}\nreaches Observe"))
            .replace(
                "self.value = 7;",
                "self.value = 7;\nObserve::record(self.value, remaining > 0 && remaining < 3);",
            );
        let observations = [false, true, true, false, true]
            .map(|selected| vec![integer(7), TerminalScalarValue::Boolean(selected)]);
        assert_observations(&produce(&source), &observations);
    }
}

fn integer(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

fn assert_observations(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    expected: &[Vec<TerminalScalarValue>],
) {
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let [receiver] = entry.structural_parameters.as_slice() else {
        panic!("one retained parent receiver");
    };
    assert!(module.machines.iter().any(|machine| {
        !terminal_verifier::control_cycle_members(machine)
            .unwrap()
            .is_empty()
    }));
    let stores = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::StructuralScalarFieldStore { .. }
            )
        })
        .collect::<Vec<_>>();
    let [store] = stores.as_slice() else {
        panic!("one authored callee store");
    };
    let start = || {
        TerminalExecution::start_artifact_with_structural_arguments(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
            &[TerminalStructuralValue {
                opaque_identity: 73,
                structural_type: receiver.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
        )
        .unwrap()
    };
    let mut execution = start();
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(1000);
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    let observed = |execution: &TerminalExecution| {
        assert_eq!(execution.effects().len(), expected.len());
        for (effect, expected) in execution.effects().iter().zip(expected) {
            let terminal_interpreter::TerminalEffect::BoundaryCall { arguments, .. } = effect
            else {
                panic!("ordered boundary observation");
            };
            assert_eq!(arguments, expected);
        }
    };
    observed(&execution);
    assert_eq!(
        meter
            .usage()
            .at(terminal_fuel::FuelChargeSite::Operation(store.id))
            .unwrap()
            .executions(),
        4
    );
    let work = meter.usage().total_units();
    for allowance in 0..work {
        let mut execution = start();
        let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(allowance);
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        meter.replenish(work - allowance).unwrap();
        assert_eq!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        observed(&execution);
        assert_eq!(meter.usage().total_units(), work);
    }
}
