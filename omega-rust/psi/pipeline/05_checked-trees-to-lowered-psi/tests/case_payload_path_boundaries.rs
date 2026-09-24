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

/// Positive control: a scalar payload bound and transferred off an owned
/// subject produces and verifies (the established dispatch route).
#[test]
fn owned_subject_scalar_payload_transfer_produces() {
    let outcome = produce(
        r#"
        pub data P { case A(n: u64); case B; }
        pub data H { marker: u64; }
        pub machine H::run(p: P) -> u64 {
            transition p {
                P::A { n } -> use_it(n)
                _ -> done()
            }
            state use_it(n: u64) -> u64 { n }
            state done() -> u64 { 9 }
        }
    "#,
    );
    assert!(outcome.is_ok(), "{outcome:?}");
}

/// An owned `s: S` parameter whose sum carries a reference payload member
/// declines at the signature: the param's owned custody shape cannot hold
/// view contents, so a subject that merely binds the payload never reaches
/// any transfer gate.
#[test]
fn owned_subject_with_reference_payload_declines_at_custody_shape() {
    let outcome = produce(
        r#"
        pub data S { case A(payload: &[u32], n: u64); case B; }
        pub data H { marker: u64; }
        pub machine H::run(s: S) -> u64 {
            transition s {
                S::A { payload, n } -> use_it(n)
                _ -> done()
            }
            state use_it(n: u64) -> u64 { n }
            state done() -> u64 { 9 }
        }
    "#,
    );
    let error = outcome.expect_err("an owned param holding a view payload must decline");
    assert!(
        error.contains("owned non-linear record contents"),
        "{error}"
    );
}

/// When the bound reference payload is actually offered as a successor
/// argument, `case_payload_transfer` declines it: the payload field is not
/// plain-owned contents and a view target needs `SharedBorrow`, not the
/// `Owned` custody the plan requires — the fallback parameter transfer then
/// finds no state parameter named `payload`.
#[test]
fn reference_payload_transfer_declines_at_parameter_transfer() {
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
    let error = outcome.expect_err("a reference payload transfer must decline");
    assert!(error.contains("parameter transfer"), "{error}");
}

/// On a borrowed subject the case binding itself is admitted, but reading a
/// bound payload member names the canonical place
/// `self.s ++ [Case{A}, Field{payload}]`, and runtime field observation
/// declines the `Case` segment: only record field paths are readable. This
/// walls off even scalar payloads under `self.` subjects.
#[test]
fn borrowed_subject_payload_read_declines_at_record_only_field_path() {
    let outcome = produce(
        r#"
        pub data P { case A(n: u64); case B; }
        pub data H { p: P; }
        pub machine H::run(&mut self) -> u64 {
            transition self.p {
                P::A { n } -> use_it(n)
                _ -> done()
            }
            state use_it(&mut self, n: u64) -> u64 { n }
            state done(&mut self) -> u64 { 9 }
        }
    "#,
    );
    let error = outcome.expect_err("a bound payload read on a borrowed subject must decline");
    assert!(error.contains("record-only field path"), "{error}");
}
