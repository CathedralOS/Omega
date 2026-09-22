//! A linear parameter consumed by a by-value `self` callee lowers as an
//! ordinary claim transfer and verifies independently; a plan that drops the
//! transfer its consumer needs is refused rather than lowered.
use super::lower_machine;
use crate::TerminalMachineSelection;
use crate::front_end::checked_program;
use checked_trees::CheckedUnitEffectOperationPlan;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

const RECEIPT: &str = "data Receipt [linear] { code: i32; }\nmachine Receipt::ack(self) {}\n";

fn produce_and_verify(source: &str, entry: &str, label: &str) {
    let artifact = produce(source, entry).unwrap_or_else(|error| panic!("{label} lowers: {error}"));
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap(),
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap_or_else(|error| panic!("{label} verifies: {error:?}"));
}

fn produce(source: &str, entry: &str) -> Result<terminal_codec::CanonicalTerminalArtifact, String> {
    let checked = checked_program(source);
    terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name(entry),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .map(|produced| produced.into_artifact())
    .map_err(|error| format!("{error:?}"))
}

#[test]
fn a_linear_parameter_receiver_lowers_and_verifies() {
    produce_and_verify(
        &format!("{RECEIPT}machine run(issued: Receipt) -> i32 {{ issued.ack(); 0 }}"),
        "run",
        "parameter",
    );
}

#[test]
fn a_consumer_call_without_its_claim_transfer_is_refused() {
    let mut checked = checked_program(&format!(
        "{RECEIPT}machine run(issued: Receipt) -> i32 {{ issued.ack(); 0 }}"
    ));
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| {
            plan.operations.iter().any(|operation| {
                matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. })
            })
        })
        .expect("`run` plans its consumer call");
    for operation in &mut plan.operations {
        if let CheckedUnitEffectOperationPlan::CallUnit {
            claim_transfers, ..
        } = operation
        {
            assert_eq!(claim_transfers.len(), 1);
            claim_transfers.clear();
        }
    }
    lower_machine(&checked, TerminalMachineSelection::Name("run"))
        .expect_err("a linear claim that never leaves the caller cannot lower");
}
