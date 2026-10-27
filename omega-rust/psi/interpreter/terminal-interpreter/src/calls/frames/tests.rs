//! Frame ownership tests, not artifact-admission evidence. Source-produced
//! embedding tests in `compiler/tests/terminal_entry_writeback.rs` cover entry
//! admission, execution, exact depth budgets, sticky faults and close.

use super::*;
use crate::byte_sequences::ByteSequenceView;
use crate::byte_sequences::storage::ByteSequenceStorage;
use crate::execution::{CleanupContinuation, CompletedReturn};
use crate::program::AdmittedTerminalProgram;
use crate::results::{TerminalExecutionResult, TerminalStructuralResult};
use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationAccountingError,
    EvaluationMeasurementIdentity, EvaluationResource, TerminalStorageScheduleIdentity,
};
use semantic_vocabulary::{BlockId, ClaimId, ContractId, EdgeId, StructuralTypeId};
use std::borrow::Cow;
use std::sync::Arc;
use terminal_psi::{
    Block, MachineContract, NominalAffineCleanup, RegisterTargetApplicability,
    StructuralResultClaimBinding, TerminalMachine, TerminalModule, Terminator,
};

pub(crate) fn empty_program() -> Arc<AdmittedTerminalProgram<'static>> {
    let machine = MachineId::new(1).unwrap();
    let entry = BlockId::new(1).unwrap();
    let mut module = TerminalModule::for_entry(machine);
    module.machines.push(TerminalMachine {
        assembly_target_requirement: RegisterTargetApplicability::Any,
        id: machine,
        attachment: None,
        declaration: None,
        nominal_cleanup_subject: None,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        content_entry_claims: Vec::new(),
        result: TerminalMachineResult::Unit,
        entry,
        blocks: vec![Block {
            id: entry,
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                cleanup_actions: Vec::new(),
            },
        }],
        ranked_scc: None,
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        contract: MachineContract::empty(ContractId::new(1).unwrap()),
    });
    Arc::new(AdmittedTerminalProgram {
        identity: terminal_codec::TerminalExecutionIdentity::portable(
            terminal_codec::terminal_psi_identity(&module).unwrap(),
        ),
        catalogs: Arc::new(Cow::Owned(module)),
        target_layouts: None,
    })
}

#[test]
fn refused_cleanup_frame_retains_outgoing_backing_and_claims() {
    let sponsor = BuildEvaluationSponsor::new_with_measurement(
        BuildEvaluationSponsorLimits::new(100, 8, 2, 2, 1, 100, 100, 100).unwrap(),
        EvaluationMeasurementIdentity::TerminalPsi {
            fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
            storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
        },
    )
    .unwrap();
    let account = sponsor
        .begin_invocation(sponsor.measurement(), 100)
        .unwrap();
    let mut execution = TerminalExecution::start_with_program(
        empty_program(),
        &[],
        &[],
        &[],
        &[],
        &[],
        &[],
        &[],
        &[],
        None,
        Some(account.clone()),
    )
    .unwrap();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let value = TerminalStructuralValue::minted(42, structural_type, Vec::new());
    let claim = ClaimId::new(1).unwrap();
    let mut outgoing = CompletedReturn::plain(TerminalExecutionResult::Structural(
        TerminalStructuralResult {
            value: value.clone(),
            claims: vec![claim],
        },
    ));
    outgoing.returned_claims = vec![StructuralResultClaimBinding {
        claim,
        path: Vec::new(),
    }];
    outgoing.byte_sequence = Some(ByteSequenceBinding::Immutable(ByteSequenceView::new(
        ByteSequenceStorage::copy(b"retained", Some(&account)).unwrap(),
    )));
    let cleanup = NominalAffineCleanup {
        retains_descendants: false,
        place: PlaceId::new(2).unwrap(),
        path: Vec::new(),
        structural_type,
        cleanup_machine: MachineId::new(1).unwrap(),
        consumed_claims: Vec::new(),
        cleanup_receiver: None,
        requirement_obligations: Vec::new(),
    };
    assert_eq!(sponsor.live_payload_bytes(), 8);
    assert!(matches!(
        execution.suspend_with(
            SuspendedCallResult::NominalCleanups {
                completed: (cleanup, value),
                remaining: Vec::new().into_iter(),
                continuation: CleanupContinuation::Return(outgoing),
            },
            Vec::new(),
        ),
        Err(TerminalInterpretError::Storage(
            EvaluationAccountingError::AllowanceExhausted(EvaluationResource::LiveStorage)
        ))
    ));
    assert_eq!(
        sponsor.live_payload_bytes(),
        8,
        "failed activation retains outgoing storage"
    );
    assert_eq!(sponsor.live_storage_cells(), 1);
    assert!(execution.call_stack.is_empty());
    assert_eq!(execution.next_operation, 0);
    let Some(SuspendedCallResult::NominalCleanups {
        continuation: CleanupContinuation::Return(retained),
        ..
    }) = &execution.failed_call_result
    else {
        panic!("refused capture must retain the exact cleanup continuation");
    };
    assert_eq!(
        retained.returned_claims,
        [StructuralResultClaimBinding {
            claim,
            path: Vec::new()
        }]
    );
    let TerminalExecutionResult::Structural(result) = &retained.value else {
        panic!("outgoing structural result was lost");
    };
    assert_eq!(result.claims, [claim]);
    assert_eq!(result.value.opaque_identity, 42);
    let Some(ByteSequenceBinding::Immutable(bytes)) = &retained.byte_sequence else {
        panic!("outgoing bytes were lost");
    };
    assert_eq!(bytes.bytes(), b"retained");
    drop(execution);
    assert_eq!(sponsor.live_payload_bytes(), 0);
    assert_eq!(sponsor.live_storage_cells(), 0);
}
