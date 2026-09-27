//! Ordinary projected calls retain a looping callee and the caller continuation.

use super::{
    IntegerSign, IntegerType, IntegerValue, OperationKind, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue, TerminalStructuralValue,
};
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};
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
    let checked = crate::front_end::checked_program(source);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("projected looping callee and caller continuation publish")
    .into_artifact();
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
    let mut checked = crate::front_end::checked_program(&SOURCE.replace("RANKING", ""));
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
        terminal_production::TerminalProductionRequest::new(
            &checked,
            TerminalMachineSelection::Name("Root::enter")
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default()
        ))
        .is_err()
    );
}

#[test]
fn natural_ranked_unit_callee_preserves_ordinary_projected_calls() {
    // A signed countdown ranks its own subject in its signed carrier; the
    // verifier's fixed-integer order is well founded for either sign.
    use semantic_vocabulary::IntegerSign::{Signed, Unsigned};
    for (primitive, sign, bits) in [
        ("u8", Unsigned, 8),
        ("u16", Unsigned, 16),
        ("u32", Unsigned, 32),
        ("u64", Unsigned, 64),
        ("i8", Signed, 8),
        ("i32", Signed, 32),
        ("i64", Signed, 64),
    ] {
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
        for machine in &module.machines {
            if let Some(terminal_psi::TerminalRankedScc::Natural(components)) = &machine.ranked_scc
            {
                assert!(components.iter().all(|component| {
                    component.rank_type
                        == semantic_vocabulary::IntegerType::new(sign, bits).unwrap()
                }));
            }
        }
        assert_observations(&artifact, &[vec![integer(7)]]);
    }
}

/// A climbing cursor: `walk(index, limit)` advances `index` under
/// `index < limit`. Both distance views rank it.
fn distance_source(ranking: &str) -> String {
    SOURCE
        .replace(
            "walk(&mut self, remaining: u64)",
            "walk(&mut self, index: u64, limit: u64)",
        )
        .replace("RANKING", ranking)
        .replace("transition remaining > 0", "transition index < limit")
        .replace("walk(remaining - 1)", "walk(index + 1, limit)")
        .replace("self.child.walk(3)", "self.child.walk(0, 3)")
}

const DISTANCE_RANKINGS: [&str; 2] = [
    "terminates by (index, limit) -> Nat::BoundedDistance;",
    "terminates by index -> Nat::IncreasingTo(limit);",
];

/// The ranked component whose ranks are computed `MAX - index` differences.
fn distance_component(
    module: &mut terminal_psi::TerminalModule,
) -> &mut terminal_psi::TerminalMachine {
    module
        .machines
        .iter_mut()
        .find(|machine| machine.ranked_scc.is_some())
        .expect("the distance-ranked callee")
}

#[test]
fn distance_ranked_unit_callee_verifies_as_a_ceiling_distance() {
    for ranking in DISTANCE_RANKINGS {
        let artifact = produce(&distance_source(ranking));
        let mut module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
        assert_eq!(proof.control_cycles.len(), 1);
        let machine = distance_component(&mut module);
        let Some(terminal_psi::TerminalRankedScc::Natural(components)) = &machine.ranked_scc else {
            panic!("natural ranks");
        };
        let ranked = components[0]
            .ranks
            .iter()
            .map(|rank| rank.value)
            .chain(components[0].edges.iter().map(|edge| edge.successor_rank))
            .collect::<Vec<_>>();
        // Every rank and successor rank is one `MAX - index` subtraction.
        assert!(ranked.iter().all(|value| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| {
                    operation
                        .result
                        .scalar()
                        .is_some_and(|result| result.id == *value)
                        && matches!(operation.kind, OperationKind::ExactIntegerSubtract { .. })
                })
        }));
        assert_observations(&artifact, &[vec![integer(7)]]);
    }
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
        TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs {
                arguments: &[TerminalStructuralValue {
                    opaque_identity: 73,
                    structural_type: receiver.structural_type,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                }],
                ..Default::default()
            },
        )
        .unwrap()
    };
    let mut execution = start();
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(1000);
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
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
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        meter.replenish(work - allowance).unwrap();
        assert_eq!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        observed(&execution);
        assert_eq!(meter.usage().total_units(), work);
    }
}
