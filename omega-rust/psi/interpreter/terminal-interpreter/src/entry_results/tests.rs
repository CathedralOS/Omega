//! Focused execution/ownership controls. Source-produced public-instantiation
//! coverage lives in compiler/tests/terminal_entry_writeback.rs.
use super::*;
use crate::calls::frames::tests::empty_program;
use crate::execution::SuspendedCallResult;
use crate::{TerminalEffect, TerminalEffectHandler, TerminalEffectRejection};
use crate::{TerminalExecutionResult, TerminalExecutionStatus};
use semantic_vocabulary::{EdgeId, OperationId};
use terminal_fuel::TerminalFuelMeter;
use terminal_psi::{
    Operation, OperationKind, OperationResult, TerminalMachineResult, Terminator, ValueDeclaration,
};

struct NoEffects;
impl TerminalEffectHandler for NoEffects {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new("unexpected effect"))
    }
}

fn execution() -> TerminalExecution<'static> {
    let mut program = empty_program();
    let program_mut = std::sync::Arc::get_mut(&mut program).unwrap();
    program_mut.fixture_module_mut().machines[0].result =
        TerminalMachineResult::Scalar(ValueDeclaration {
            id: ValueId::new(1).unwrap(),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        });
    program_mut.fixture_module_mut().machines[0].blocks[0].terminator = returned();
    program_mut.fixture_module_mut().machines[0].blocks[0]
        .operations
        .push(Operation::unsuspended(
            OperationId::new(1).unwrap(),
            OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Boolean,
                qualifications: Default::default(),
            }),
            OperationKind::BooleanConstant { value: true },
        ));
    TerminalExecution::start_with_program(
        program,
        &[],
        &[],
        &[],
        &[],
        &[],
        &[],
        &[],
        &[],
        None,
        None,
    )
    .unwrap()
}

fn request(execution: &mut TerminalExecution<'_>) {
    execution
        .request_entry_results(TerminalEntryResultRequest {
            machine: MachineId::new(1).unwrap(),
            results: &[(ValueId::new(1).unwrap(), ScalarType::Boolean)],
        })
        .unwrap();
}

#[test]
fn entry_results_capture_successful_operation_once_across_fuel_pauses() {
    let mut execution = execution();
    request(&mut execution);
    let machine = execution.current_machine;
    let result = ValueId::new(1).unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution.resume(&mut meter, &mut NoEffects).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert!(execution.observed_entry_result(machine, result).is_err());
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution.resume(&mut meter, &mut NoEffects).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert!(
        execution.observed_entry_result(machine, result).is_err(),
        "captured does not mean completed"
    );
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution.resume(&mut meter, &mut NoEffects).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Boolean(true)
        ))
    ));
    assert_eq!(
        execution.observed_entry_result(machine, result).unwrap(),
        TerminalScalarValue::Boolean(true)
    );
    // A later mutation of the ordinary value map cannot rewrite the observation.
    execution
        .values
        .insert(result, TerminalScalarValue::Boolean(false))
        .unwrap();
    assert_eq!(
        execution.observed_entry_result(machine, result).unwrap(),
        TerminalScalarValue::Boolean(true)
    );
}

#[test]
fn entry_results_ignore_recursive_activation_and_capture_restored_caller() {
    let mut execution = execution();
    request(&mut execution);
    let result = ValueId::new(1).unwrap();
    execution
        .suspend_with(SuspendedCallResult::Scalar(result), Vec::new())
        .unwrap();
    // Recursion reuses machine/result coordinates but not the root activation.
    execution
        .values
        .insert(result, TerminalScalarValue::Boolean(false))
        .unwrap();
    execution.observe_completed_entry_result(result).unwrap();
    assert!(execution.require_entry_results_complete().is_err());
    execution
        .values
        .insert(result, TerminalScalarValue::Boolean(true))
        .unwrap();
    assert!(
        execution
            .settle_return(&returned(), &mut TerminalFuelMeter::unbounded())
            .is_ok()
    );
    execution.require_entry_results_complete().unwrap();
    assert!(
        execution
            .settle_return(&returned(), &mut TerminalFuelMeter::unbounded())
            .is_ok()
    );
    assert_eq!(
        execution
            .observed_entry_result(MachineId::new(1).unwrap(), result)
            .unwrap(),
        TerminalScalarValue::Boolean(true)
    );
}

#[test]
fn entry_results_repeated_completion_is_a_sticky_execution_failure() {
    let mut execution = execution();
    request(&mut execution);
    let mut meter = TerminalFuelMeter::with_allowance(1);
    execution.resume(&mut meter, &mut NoEffects).unwrap();
    // Model revisiting the same operation through a state backedge. Even the
    // identical value must not become last-writer-wins occurrence evidence.
    execution.next_operation = 0;
    meter.replenish(1).unwrap();
    let failure = execution.resume(&mut meter, &mut NoEffects).unwrap_err();
    assert!(format!("{failure:?}").contains("completed more than once"));
    assert!(execution.failure.is_some());
    assert!(
        execution
            .resume(&mut TerminalFuelMeter::unbounded(), &mut NoEffects)
            .is_err()
    );
    assert!(
        execution
            .observed_entry_result(MachineId::new(1).unwrap(), ValueId::new(1).unwrap())
            .is_err()
    );
}

#[test]
fn entry_results_missing_occurrence_cannot_use_final_return() {
    let mut execution = execution();
    request(&mut execution);
    // A branch skips the requested operation but returns an equal scalar.
    execution.next_operation = 1;
    execution
        .values
        .insert(ValueId::new(1).unwrap(), TerminalScalarValue::Boolean(true))
        .unwrap();
    let failure = execution
        .resume(&mut TerminalFuelMeter::unbounded(), &mut NoEffects)
        .unwrap_err();
    assert!(format!("{failure:?}").contains("without completing"));
    assert!(execution.result.is_none());
}

#[test]
fn entry_results_require_exact_root_operation_carrier_and_unique_request() {
    for (machine, results) in [
        (
            MachineId::new(2).unwrap(),
            vec![(ValueId::new(1).unwrap(), ScalarType::Boolean)],
        ),
        (
            MachineId::new(1).unwrap(),
            vec![(ValueId::new(2).unwrap(), ScalarType::Boolean)],
        ),
        (
            MachineId::new(1).unwrap(),
            vec![(
                ValueId::new(1).unwrap(),
                ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .unwrap(),
                ),
            )],
        ),
        (
            MachineId::new(1).unwrap(),
            vec![(ValueId::new(1).unwrap(), ScalarType::Boolean); 2],
        ),
    ] {
        let mut execution = execution();
        assert!(
            execution
                .request_entry_results(TerminalEntryResultRequest {
                    machine,
                    results: &results
                })
                .is_err()
        );
        assert!(
            execution.entry_results.is_none(),
            "failed request leaves no partial roster"
        );
    }
}

fn returned() -> Terminator {
    Terminator::Return {
        edge: EdgeId::new(1).unwrap(),
        value: ValueId::new(1).unwrap(),
        cleanup_actions: Vec::new(),
    }
}
