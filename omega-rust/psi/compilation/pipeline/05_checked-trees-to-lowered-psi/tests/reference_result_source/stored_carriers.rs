//! Stored loans use the same reference operation at every construction depth.

use super::{AcceptTerminalEffects, TerminalExecution, TerminalExecutionResult};
use super::{TerminalExecutionStatus, TerminalFuelMeter, TerminalMachineSelection};
use super::{
    TerminalProductionCustody, TerminalProductionTimings, TerminalStructuralInputs, signed,
};

#[test]
fn stored_reference_carriers_preserve_projected_backing_and_access() {
    let checked = crate::front_end::checked_program(
        "data Cell { value: i32; }
         data Storage { cell: Cell; neighbor: Cell; }
         data Message { body: &Cell; }
         data Envelope { message: Message; }
         machine read(cell: &Cell) -> i32 { cell.value }
         machine exercise() -> i32 {
             let storage: Storage = Storage {
                 cell: Cell { value: 70 }, neighbor: Cell { value: 71 }
             };
             let envelope: Envelope = Envelope {
                 message: Message { body: &storage.cell }
             };
             read(envelope.message.body)
         }",
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("exercise"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("projected storage establishes the declared reference carrier")
    .into_artifact();
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("independently verify nested stored-reference custody");
    assert_eq!(
        execution
            .resume(
                &mut TerminalFuelMeter::with_allowance(1_000),
                &mut AcceptTerminalEffects,
            )
            .expect("execute reference field read"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(signed(70))),
    );

    let module = terminal_codec::decode_module(artifact.semantic_bytes()).expect("decode");
    for corrupt_path in [false, true] {
        let mut changed = module.clone();
        let mut count = 0;
        for operation in changed
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
        {
            if let terminal_psi::OperationKind::EstablishReference { source } = &mut operation.kind
            {
                count += 1;
                if corrupt_path {
                    source.path.clear();
                } else {
                    source.access = terminal_psi::StructuralAccess::Owned;
                }
            }
        }
        assert_eq!(count, 1, "one exact reference formation");
        assert!(
            terminal_verifier::validate_module(&changed).is_err(),
            "a stored carrier cannot substitute its source path or own its referent: corrupt_path={corrupt_path}"
        );
    }
}
