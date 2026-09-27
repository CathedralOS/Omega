//! Checked state-graph providers retain their exact body and ordinary call closure.

use language_semantics::Multiplicity;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

const SOURCE: &str = r#"
    data ReadResult { case Empty; case Bytes(count: u64); }
    boundary trait Host {
        machine read(flag: bool) -> ReadResult reaches Host invokes Host;
        machine mark(value: u64) reaches Host;
    }
    data Provider {}
    machine Provider::read(flag: bool) -> ReadResult satisfies Host::read {
        transition flag { true -> bytes() false -> empty() }
        state bytes() { ReadResult::Bytes { count: 7 } }
        state empty() { ReadResult::Empty }
    }
    machine run(flag: bool) reaches Host {
        Host::mark(1);
        _ = Host::read(flag);
        Host::mark(2);
    }
"#;

#[test]
fn composed_provider_candidate_rejects_result_and_body_roster_corruption() {
    let source = SOURCE.to_owned() + "machine identity(value: ReadResult) -> ReadResult { value }";
    let baseline = crate::front_end::checked_program(&source);
    let _ = terminal_production::TerminalProductionRequest::new(
        &baseline,
        terminal_production::TerminalMachineSelection::Name("run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("unmodified exact provider")
    .into_artifact();
    let provider = baseline.facts.flow.terminal_unit_effects.composed_machines[0].machine;
    for corruption in 0..6 {
        let mut changed = baseline.clone();
        let plans = &mut changed.facts.flow.terminal_unit_effects;
        match corruption {
            0 | 1 => {
                let checked_trees::CheckedControlResultPlan::Structural(result) =
                    &mut plans.composed_machines[0].result
                else {
                    panic!("provider owns a structural sum result")
                };
                if corruption == 0 {
                    result.type_identity = "named(name(OtherResult))".to_owned();
                } else {
                    result.multiplicity = Multiplicity::Linear;
                }
            }
            2 => plans
                .composed_machines
                .push(plans.composed_machines[0].clone()),
            3 => {
                let mut ordinary = plans.machines[0].clone();
                ordinary.machine = provider;
                plans.machines.push(ordinary);
            }
            4 => {
                let mut affine = changed
                    .facts
                    .flow
                    .terminal_structural_returns
                    .claim_free_affine_machines[0]
                    .clone();
                affine.machine = provider;
                changed
                    .facts
                    .flow
                    .terminal_structural_returns
                    .claim_free_affine_machines
                    .push(affine);
            }
            _ => plans.composed_machines.clear(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(
                &changed,
                terminal_production::TerminalMachineSelection::Name("run")
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default()
            ))
            .is_err(),
            "provider corruption {corruption}"
        );
    }
}
