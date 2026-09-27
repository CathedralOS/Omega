//! Case-payload path-addressability boundary witnesses.
//!
//! A case-payload binding's canonical place spells
//! `subject ++ [Case{selected}, Field{member}]` — the facts vocabulary can
//! name a payload member, but neither transferred structural path vocabulary
//! (`CheckedUnitStructuralPathSegment` nor terminal `StructuralPathSegment`)
//! carries a `Case` segment, so no operation path reaches inside a payload.
//! The only payload consumer, `case_payload_transfer`, admits plain-owned
//! members moved to `Owned` targets; reference members and borrowed subjects
//! decline at the sites these tests pin.
use checked_trees_to_lowered_psi::*;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

fn produce(source: &str) -> Result<(), String> {
    let checked = crate::front_end::checked_program(source);
    match terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("H::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    )) {
        Ok(artifact) => {
            let artifact = artifact.into_artifact();
            let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
            let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
            terminal_verifier::verify_module(
                &module,
                &proof,
                &proof_admission::AdmissionProfile::default(),
            )
            .map(|_| ())
            .map_err(|error| format!("verify error: {error:?}"))
        }
        Err(rejection) => Err(format!("{rejection}")),
    }
}

/// Even with the `SharedBorrow` transfer arm, an owned `s: S` subject can
/// never reach it: the entry signature refuses owned custody of a record
/// containing view contents, so only borrowed subjects exercise the arm.
#[test]
fn owned_subject_reference_payload_still_declines_at_signature() {
    let outcome = produce(
        r#"
        pub data S { case A(payload: &[u32], n: u64); case B; }
        pub data H { marker: u64; }
        pub machine H::run(s: S) -> u64 {
            transition s {
                S::A { payload, n } -> use_it(payload, n)
                _ -> done()
            }
            state use_it(payload: &[u32], n: u64) -> u64 { n }
            state done() -> u64 { 9 }
        }
    "#,
    );
    let error =
        outcome.expect_err("an owned param holding a view payload must decline at the signature");
    assert!(
        error.contains("owned non-linear record contents"),
        "{error}"
    );
}

/// A shared-borrow payload member off a borrowed subject re-seats its loan
/// into a `SharedBorrow` successor target: the `CasePayload` transfer mints
/// a `StructuralCaseLeafCopy` whose canonical `Case`-segmented path reads
/// the view descriptor without disturbing the subject's custody, and the
/// successor takes `SharedBorrow` custody of the same view.
#[test]
fn borrowed_subject_shared_view_payload_transfer_produces() {
    let outcome = produce(
        r#"
        pub data S { case A(payload: &[u32]); case B; }
        pub data H { p: S; }
        pub machine H::run(&mut self) -> u64 {
            transition self.p {
                S::A { payload } -> use_it(payload)
                _ -> done()
            }
            state use_it(payload: &[u32]) -> u64 { 7 }
            state done(&mut self) -> u64 { 9 }
        }
    "#,
    );
    assert!(outcome.is_ok(), "{outcome:?}");
}
