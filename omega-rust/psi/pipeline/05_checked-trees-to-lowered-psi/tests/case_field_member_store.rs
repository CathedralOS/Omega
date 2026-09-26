//! Case-field member stores: `self.fmt = source.fmt` where the field is a
//! `[copy]` sum and the value is a member read below a borrowed structural
//! parameter. The store lowers through a structural leaf copy plus the
//! destination's window move/store pair, and the produced artifact verifies.
use checked_trees_to_lowered_psi::*;
use lowered_psi_to_terminal_psi::terminal_production;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

fn produce(source: &str, machine: &str) -> Result<(), String> {
    let checked = crate::front_end::checked_program(source);
    match terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name(machine),
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

#[test]
fn borrowed_member_read_replaces_the_receiver_case_field() {
    let outcome = produce(
        r#"
        pub data Fmt [copy] { case Decimal; case Hex; case Octal; }
        pub data Box { fmt: Fmt; tag: u64; }
        pub machine Box::set_fmt<'a>(&mut self, source: &'a Box) {
            self.fmt = source.fmt;
        }
        "#,
        "Box::set_fmt",
    );
    assert_eq!(
        outcome,
        Ok(()),
        "borrowed member store should produce+verify"
    );
}

/// The destination need not be `self`: a member read below a borrowed source
/// can replace a case field of another borrowed carrier.
#[test]
fn borrowed_member_read_replaces_another_parameter_case_field() {
    let outcome = produce(
        r#"
        pub data Fmt [copy] { case Decimal; case Hex; case Octal; }
        pub data Box { fmt: Fmt; tag: u64; }
        pub machine Box::copy_fmt<'a>(source: &'a Box, sink: &'a mut Box) {
            sink.fmt = source.fmt;
        }
        "#,
        "Box::copy_fmt",
    );
    assert_eq!(outcome, Ok(()), "sink member store should produce+verify");
}
