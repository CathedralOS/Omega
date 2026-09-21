//! Embedding probes for the existing scalar-only effect sink, not a host SDK.
//! No borrowed host resource, service installation, or cancellation is modeled.

use super::{
    AdmissionProfile, ProofBundle, TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
    TerminalEffectResult, TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalFuelMeter, TerminalInterpretError, TerminalScalarValue, TerminalStructuralInputs,
    boundary_id, edge_id, encode_module, encode_proof_section, operation_id,
    scalar_boundary_effect_module, value_id,
};
use semantic_vocabulary::ScalarType;
use terminal_psi::{
    BoundaryMachineResult, OperationResult, TerminalMachineResult, Terminator, ValueDeclaration,
};

#[derive(Default)]
struct HostState {
    observations: Vec<(bool, bool)>,
}

struct BorrowedHost<'host> {
    state: &'host mut HostState,
}

impl TerminalEffectHandler for BorrowedHost<'_> {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            boundary,
            arguments,
            ..
        } = effect
        else {
            return Err(TerminalEffectRejection::new("unsupported host operation"));
        };
        if *boundary != boundary_id(1) {
            return Err(TerminalEffectRejection::new("unbound requirement"));
        }
        let [
            TerminalScalarValue::Boolean(first),
            TerminalScalarValue::Boolean(second),
        ] = arguments.as_slice()
        else {
            return Err(TerminalEffectRejection::new("wrong argument schema"));
        };
        self.state.observations.push((*first, *second));
        Ok(())
    }
}

fn start_two_calls() -> TerminalExecution {
    let mut module = scalar_boundary_effect_module();
    let mut second_call = module.machines[0].blocks[0].operations[2].clone();
    second_call.id = operation_id(4);
    module.machines[0].blocks[0].operations.push(second_call);
    let semantic = encode_module(&module).expect("embedding fixture encodes");
    let proof = encode_proof_section(&module, &ProofBundle::default())
        .expect("empty evidence is bound to this fixture");
    TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("source-free fixture is independently checked")
    // The module, input bytes, proof bytes and admission profile all die here.
}

#[test]
fn execution_owns_code_after_loading_buffers_are_dropped() {
    let mut execution = start_two_calls();
    let mut state = HostState::default();
    assert_eq!(
        execution
            .resume(
                &mut TerminalFuelMeter::unbounded(),
                &mut BorrowedHost { state: &mut state },
            )
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(state.observations, [(true, false), (true, false)]);
}

#[test]
fn scalar_only_fuel_pauses_release_handler_borrow_without_repeating_calls() {
    let mut execution = start_two_calls();
    let mut state = HostState::default();
    let mut meter = TerminalFuelMeter::with_allowance(0);
    let mut pauses = 0;
    let mut paused_after_first_call = false;
    loop {
        let status = execution
            .resume(&mut meter, &mut BorrowedHost { state: &mut state })
            .unwrap();
        // The handler's borrow ended; no host-backed value escaped this fixture.
        assert!(state.observations.len() <= 2);
        match status {
            TerminalExecutionStatus::SponsorExhausted(_) => {
                pauses += 1;
                paused_after_first_call |= state.observations.len() == 1;
                assert!(pauses < 32, "fixture must make bounded progress");
                meter.replenish(1).unwrap();
            }
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => break,
            other => panic!("unexpected scalar fixture outcome: {other:?}"),
        }
    }
    assert!(paused_after_first_call);
    assert_eq!(state.observations, [(true, false), (true, false)]);
    assert_eq!(
        execution
            .resume(&mut meter, &mut BorrowedHost { state: &mut state })
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(state.observations.len(), 2, "completion is not re-entry");
}

#[test]
fn executions_can_use_independent_host_state() {
    let mut first = start_two_calls();
    let mut second = start_two_calls();
    let mut first_state = HostState::default();
    let mut second_state = HostState::default();
    first
        .resume(
            &mut TerminalFuelMeter::unbounded(),
            &mut BorrowedHost {
                state: &mut first_state,
            },
        )
        .unwrap();
    assert!(second_state.observations.is_empty());
    second
        .resume(
            &mut TerminalFuelMeter::unbounded(),
            &mut BorrowedHost {
                state: &mut second_state,
            },
        )
        .unwrap();
    assert_eq!(first_state.observations, second_state.observations);
    assert_eq!(first_state.observations.len(), 2);
}

#[test]
fn raw_effect_sinks_do_not_pin_host_provider_identity() {
    let mut execution = start_two_calls();
    let mut first_state = HostState::default();
    let mut second_state = HostState::default();
    let mut meter = TerminalFuelMeter::with_allowance(0);
    for _ in 0..32 {
        execution
            .resume(
                &mut meter,
                &mut BorrowedHost {
                    state: &mut first_state,
                },
            )
            .unwrap();
        if first_state.observations.len() == 1 {
            break;
        }
        meter.replenish(1).unwrap();
    }
    assert_eq!(first_state.observations.len(), 1);
    assert_eq!(
        execution
            .resume(
                &mut TerminalFuelMeter::unbounded(),
                &mut BorrowedHost {
                    state: &mut second_state
                },
            )
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(second_state.observations.len(), 1);
    // Legal for this oracle hook, not evidence of a stable Service binding.
}

#[test]
fn host_rejection_is_not_rollback_of_host_state() {
    struct RejectAfterEffect<'host>(&'host mut usize);
    impl TerminalEffectHandler for RejectAfterEffect<'_> {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            *self.0 += 1;
            Err(TerminalEffectRejection::new(
                "host failed after its side effect",
            ))
        }
    }
    let mut execution = start_two_calls();
    let mut effects = 0;
    assert!(matches!(
        execution.resume(
            &mut TerminalFuelMeter::unbounded(),
            &mut RejectAfterEffect(&mut effects),
        ),
        Err(TerminalInterpretError::EffectRejected { .. })
    ));
    assert_eq!(effects, 1);
    assert!(
        execution.effects().is_empty(),
        "no successful receipt was committed"
    );
    // A public embedding owner must not automatically retry this invocation.
}

#[test]
fn host_result_schema_mismatch_is_not_success() {
    struct WrongResult;
    impl TerminalEffectHandler for WrongResult {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            panic!("result-aware entry expected")
        }

        fn handle_effect_result(
            &mut self,
            _: &TerminalEffect,
        ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
            Ok(TerminalEffectResult::Scalar(TerminalScalarValue::Boolean(
                true,
            )))
        }
    }
    let mut execution = start_two_calls();
    assert!(
        execution
            .resume(&mut TerminalFuelMeter::unbounded(), &mut WrongResult)
            .is_err()
    );
    assert!(execution.effects().is_empty());
}

#[test]
fn native_result_flows_back_through_the_guest_to_the_embedding_caller() {
    struct HostComparison;
    impl TerminalEffectHandler for HostComparison {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            panic!("result-aware entry expected")
        }

        fn handle_effect_result(
            &mut self,
            effect: &TerminalEffect,
        ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
            let TerminalEffect::BoundaryCall {
                boundary,
                arguments,
                ..
            } = effect
            else {
                return Err(TerminalEffectRejection::new("unsupported host operation"));
            };
            assert_eq!(*boundary, boundary_id(1));
            let [
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
            ] = arguments.as_slice()
            else {
                return Err(TerminalEffectRejection::new("wrong argument schema"));
            };
            Ok(TerminalEffectResult::Scalar(TerminalScalarValue::Boolean(
                first != second,
            )))
        }
    }
    let mut module = scalar_boundary_effect_module();
    module.boundary_machines[0].result = BoundaryMachineResult::Scalar(ScalarType::Boolean);
    let returned = ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(3),
        scalar_type: ScalarType::Boolean,
    };
    module.machines[0].result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(4),
        ..returned
    });
    module.machines[0].blocks[0].operations[2].result = OperationResult::Scalar(returned);
    module.machines[0].blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: value_id(3),
        cleanup_actions: Vec::new(),
    };
    let semantic = encode_module(&module).unwrap();
    let proof = encode_proof_section(&module, &ProofBundle::default()).unwrap();
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .unwrap();
    assert_eq!(
        execution
            .resume(&mut TerminalFuelMeter::unbounded(), &mut HostComparison)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Boolean(true)
        ))
    );
}
