//! Explicit result discard belongs to the boundary's immediate normal continuation.
use super::{CheckedTrees, SymbolHandle, lower_machine};
use crate::TerminalMachineSelection;
use checked_trees::CheckedUnitEffectOperationPlan;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
    TerminalEffectResult, TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalScalarValue, TerminalStructuralValue,
};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

const SOURCE: &str = r#"
    data ReadResult { case Empty; case Bytes(count: u64); }
    boundary trait Host {
        machine read(buffer: &mut [u8]) -> ReadResult reaches Host;
        machine mark(value: u64) reaches Host;
    }
    machine run(buffer: &mut [u8]) reaches Host {
        Host::mark(1);
        _ = Host::read(buffer);
        Host::mark(2);
    }
"#;

fn run_symbol(checked: &CheckedTrees) -> SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .expect("authored run machine")
        .symbol
}

#[test]
fn discarded_boundary_result_rejects_missing_authored_discard_marker() {
    let mut checked = crate::front_end::checked_program(SOURCE);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == run_symbol(&checked))
        .unwrap();
    let statements = checked.machine_states(machine)[0].statement_nodes;
    let typed_trees::statement::StatementNode::Call(call) =
        &mut checked.typed.statement_table.statements_mut(statements)[1]
    else {
        panic!("authored discard call");
    };
    assert!(call.discards_result);
    call.discards_result = false;
    assert!(lower_machine(&checked, TerminalMachineSelection::Name("run")).is_err());
    assert!(
        terminal_production::TerminalProductionRequest::new(
            &checked,
            terminal_production::TerminalMachineSelection::Name("run")
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default()
        ))
        .is_err()
    );
}

#[test]
fn discarded_boundary_result_rejects_result_cleanup_and_later_operand_drift() {
    let original = crate::front_end::checked_program(SOURCE);
    for mutation in [
        "result type",
        "result ordinal",
        "result statement",
        "return discard",
        "missing cleanup",
        "replace immediate cleanup with return discard",
        "duplicate cleanup",
        "delayed cleanup",
        "cleanup ordinal",
        "cleanup type",
        "cleanup coordinate",
        "later operand",
    ] {
        let mut checked = original.clone();
        let symbol = run_symbol(&checked);
        let operations = &mut checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == symbol)
            .unwrap()
            .operations;
        match mutation {
            "result type" | "result ordinal" | "result statement" | "return discard" => {
                let CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    result,
                    discard_result_on_return,
                    ..
                } = &mut operations[1]
                else {
                    panic!("structural read");
                };
                match mutation {
                    "result type" => result.type_identity = "Host".to_owned(),
                    "result ordinal" => result.binding_ordinal += 1,
                    "result statement" => result.statement_index += 1,
                    "return discard" => *discard_result_on_return = true,
                    _ => unreachable!(),
                }
            }
            "missing cleanup" => {
                operations.remove(2);
            }
            "replace immediate cleanup with return discard" => {
                operations.remove(2);
                let CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    discard_result_on_return,
                    ..
                } = &mut operations[1]
                else {
                    panic!("structural read");
                };
                *discard_result_on_return = true;
            }
            "duplicate cleanup" => {
                operations.insert(3, operations[2].clone());
            }
            "delayed cleanup" => {
                operations.swap(2, 3);
            }
            "cleanup ordinal" | "cleanup type" | "cleanup coordinate" => {
                let CheckedUnitEffectOperationPlan::CallContinuationCleanup {
                    coordinate,
                    affine_discards,
                } = &mut operations[2]
                else {
                    panic!("immediate cleanup");
                };
                match mutation {
                    "cleanup ordinal" => {
                        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal,
                        } = &mut affine_discards[0].source else { panic!("result root"); };
                        *binding_ordinal += 1;
                    }
                    "cleanup type" => affine_discards[0].type_identity = "Host".to_owned(),
                    "cleanup coordinate" => coordinate.statement_index += 1,
                    _ => unreachable!(),
                }
            }
            "later operand" => {
                let CheckedUnitEffectOperationPlan::BoundaryCall {
                    scalar_arguments, ..
                } = &operations[0]
                else {
                    panic!("first marker");
                };
                let substituted = scalar_arguments.clone();
                let CheckedUnitEffectOperationPlan::BoundaryCall {
                    scalar_arguments, ..
                } = &mut operations[3]
                else {
                    panic!("last marker");
                };
                *scalar_arguments = substituted;
            }
            _ => unreachable!(),
        }
        assert!(
            lower_machine(&checked, TerminalMachineSelection::Name("run")).is_err(),
            "source drift: {mutation}"
        );
        assert!(
            terminal_production::TerminalProductionRequest::new(
                &checked,
                terminal_production::TerminalMachineSelection::Name("run")
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default()
            ))
            .is_err(),
            "publication drift: {mutation}"
        );
    }
}

#[test]
fn discarded_boundary_result_preserves_effect_order_across_every_fuel_pause() {
    // Exercise result custody independently of host buffer realization. The
    // artifact test above retains the borrowed-buffer signature; arbitrary host
    // callbacks over raw arrays are not the selected checked-provider route.
    let source = SOURCE
        .replace("buffer: &mut [u8]", "")
        .replace("Host::read(buffer)", "Host::read()");
    let checked = crate::front_end::checked_program(&source);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("source discard caller")
    .into_artifact();
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .unwrap();
    struct Host {
        events: Vec<u64>,
    }
    impl TerminalEffectHandler for Host {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            panic!("exact boundary result handler required")
        }
        fn handle_effect_with_byte_buffers(
            &mut self,
            effect: &TerminalEffect,
            buffers: &mut [TerminalBoundaryByteBuffer],
        ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
            let TerminalEffect::BoundaryCall {
                arguments, result, ..
            } = effect
            else {
                panic!("Host boundary");
            };
            match result {
                terminal_psi::BoundaryMachineResult::Unit => {
                    assert!(buffers.is_empty());
                    let [
                        TerminalScalarValue::Integer {
                            value: semantic_vocabulary::IntegerValue::Unsigned(value),
                            ..
                        },
                    ] = arguments.as_slice()
                    else {
                        panic!("u64 marker");
                    };
                    self.events.push(u64::try_from(*value).unwrap());
                    Ok(TerminalEffectResult::Unit)
                }
                terminal_psi::BoundaryMachineResult::Structural(result) => {
                    assert!(arguments.is_empty());
                    assert!(buffers.is_empty());
                    self.events.push(3);
                    // Discard observes no tag/payload. Opaque structural custody is enough.
                    Ok(TerminalEffectResult::Structural(TerminalStructuralValue {
                        opaque_identity: 99,
                        structural_type: result.structural_type,
                        qualifications: result.qualifications.clone(),
                        path: Vec::new(),
                    }))
                }
                _ => panic!("no scalar read result"),
            }
        }
    }
    let mut host = Host { events: Vec::new() };
    let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    let mut completed = false;
    for _ in 0..256 {
        match execution
            .resume(&mut fuel, &mut host)
            .unwrap_or_else(|error| panic!("{error:?}, observed host events: {:?}", host.events))
        {
            TerminalExecutionStatus::SponsorExhausted(_) => {
                let events = host.events.clone();
                let effects = execution.effects().to_vec();
                assert!(matches!(
                    execution.resume(&mut fuel, &mut host).unwrap(),
                    TerminalExecutionStatus::SponsorExhausted(_)
                ));
                assert_eq!(
                    host.events, events,
                    "exhausted resume cannot replay host work"
                );
                assert_eq!(execution.effects(), effects.as_slice());
                fuel.replenish(1).unwrap();
            }
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
                completed = true;
                break;
            }
            other => panic!("unexpected execution status: {other:?}"),
        }
    }
    assert!(completed, "bounded test execution must complete");
    assert_eq!(host.events, [1, 3, 2]);
    assert_eq!(execution.effects().len(), 3);
}
