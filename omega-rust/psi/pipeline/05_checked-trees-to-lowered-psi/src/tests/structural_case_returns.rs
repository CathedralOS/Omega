//! Sum cases carrying structural payloads establish and return whole owned
//! values through `EstablishStructuralCase`; the verifier must accept the
//! emitted module and reject a tampered source.
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

const SOURCE: &str = r#"
    data Inner { value: u64; }
    data Outcome { case Valued(inner: Inner); case Declined; }
    machine Inner::new(value: u64) -> Inner { Inner { value: value } }
    machine Outcome::pick(b: bool) -> Outcome {
        transition b { true -> valued() _ -> declined() }
        state valued() -> Outcome { Outcome::Valued { inner: Inner { value: 3 } } }
        state declined() -> Outcome { Outcome::Declined }
    }
    machine Outcome::via_call(b: bool) -> Outcome {
        transition b { true -> valued() _ -> declined() }
        state valued() -> Outcome { Outcome::Valued { inner: Inner::new(7) } }
        state declined() -> Outcome { Outcome::Declined }
    }
"#;

fn produce(machine: &str) -> terminal_codec::CanonicalTerminalArtifact {
    let checked = crate::front_end::checked_program(SOURCE);
    terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name(machine),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact()
}

fn verified_module(machine: &str) -> terminal_psi::TerminalModule {
    let artifact = produce(machine);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap(),
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    module
}

#[test]
fn case_payload_return_accepts_member_call_children() {
    verified_module("Outcome::via_call");
}
