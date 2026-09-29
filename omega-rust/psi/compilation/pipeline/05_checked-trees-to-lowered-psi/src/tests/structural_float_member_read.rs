//! Clone-family statement-sequence boundary witnesses.
//!
//! A record literal whose tail members read through a borrowed structural
//! parameter (`source.field`) admits an ordinary statement-sequence plan at
//! the checked stage. Lowering walks the authored member chain's receiver
//! type: `unwrapped_type_reference` strips the `&`, so a
//! lifetime-parameterized record arrives as a `Generic` node whose
//! `base_symbol` names the same data declaration the `Named` route resolves
//! — `walk_exact_place` admits it, and both integer and `f32` member clones
//! produce and independently verify. A literal that nests calls inside its
//! tail fields instead declines at the statement-sequence call-ordering roster.
// `TerminalMachineSelection` must come through `terminal_production`'s
// re-export, not `crate::`: in the lib test target `crate` is the test
// instance of this crate while `terminal_production` holds the library
// instance, and the two types never unify.
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

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

/// The sweep-ledger clone shape: shared view members plus an `f32` member
/// read through a `&'a` parameter. The `Generic` receiver resolves its
/// member walk against `base_symbol`; the float member retains its primitive
/// format while the adjacent views retain their borrowed custody.
#[test]
fn clone_literal_on_borrowed_generic_receiver_verifies_with_float_leaf() {
    let outcome = produce(
        r#"
        pub data TrackableTaskHandle<'a> {
            name: &'a [u8];
            progress: f32;
            task_identifier: &'a [u8];
        }
        pub machine TrackableTaskHandle::clone<'a>(source: &'a TrackableTaskHandle<'a>) -> TrackableTaskHandle<'a> {
            TrackableTaskHandle { name: source.name, progress: source.progress, task_identifier: source.task_identifier }
        }
        "#,
        "TrackableTaskHandle::clone",
    );
    outcome.expect("a borrowed generic clone with an f32 member must produce and verify");
}

/// The admission is not float-specific: an integer-member clone on a
/// `<'a>`-parameterized record walks the same `Generic` receiver.
#[test]
fn integer_member_clone_shares_the_generic_receiver_admission() {
    let outcome = produce(
        r#"
        pub data ThreeU<'a> { name: &'a [u8]; tag: u64; ident: &'a [u8]; }
        pub machine ThreeU::clone<'a>(source: &'a ThreeU<'a>) -> ThreeU<'a> {
            ThreeU { name: source.name, tag: source.tag, ident: source.ident }
        }
        "#,
        "ThreeU::clone",
    );
    outcome.expect("an integer-member clone must produce and verify");
}

/// Calls nested inside a record literal's tail fields are not ordered
/// statement calls: the statement-sequence roster only walks the tail
/// `Expression` node's own `Call`, not calls authored inside literal
/// members, so the machine's checked unit never forms.
#[test]
fn nested_calls_in_record_tail_decline_at_ordered_statement_call() {
    let outcome = produce(
        r#"
        pub data Handle<'a> { name: &'a [u8]; progress: f32; ident: &'a [u8]; }
        pub data Task { name_len: u64; }
        pub machine Task::get_name<'a>(&self) -> &'a [u8] { "task" }
        pub machine Task::get_task_identifier<'a>(&self) -> &'a [u8] { "id" }
        pub machine Task::get_task_handle<'a>(&self) -> Handle<'a> {
            Handle { name: self.get_name(), progress: 0.5, ident: self.get_task_identifier() }
        }
        "#,
        "Task::get_task_handle",
    );
    let error = outcome.expect_err("nested tail calls must decline");
    assert!(
        error.contains("outer calls: ordered statement call"),
        "expected the ordered-statement-call wall, got {error}"
    );
}
