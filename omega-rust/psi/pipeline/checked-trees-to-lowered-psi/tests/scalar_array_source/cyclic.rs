//! A ranked cyclic machine may re-establish an unrestricted scalar array on
//! every traversal and pass the member-produced payload to a call as an
//! owned argument. The verifier admits the cycle only through the
//! unrestricted/claim-free establishment arm, and the interpreter replaces
//! the stored payload each traversal instead of treating the place as
//! established once per activation.

use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};
use terminal_psi::{OperationKind, StructuralAccess, StructuralMultiplicity};

use super::checked_source;

/// `scan` re-enters its body through the recursive edge, so `first` reads
/// whichever payload the deepest traversal established: `[0, scale]` at the
/// base, not the entry iteration's `[3, scale]`. A stale or rejected
/// re-establishment changes the result or fails interpretation outright.
const CYCLE_SOURCE: &str = r#"
    machine first(row: [u64; 2]) -> u64 { row[0] }
    machine scan(remaining: u64 [0..=5], scale: u64 [0..=10]) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let v: u64 = first([remaining, scale]);
        transition remaining > 0 {
            true -> scan(remaining - 1, scale)
            _ -> v
        }
    }
"#;

#[test]
fn cyclic_unrestricted_scalar_array_reestablishes_and_feeds_an_owned_argument() {
    let checked = checked_source(CYCLE_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "scan")
        .expect("ranked cycle carrying a scalar-array establishment lowers");
    let module = &lowered.semantic_module;
    let scan = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry scan");
    let establishment = scan
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::EstablishScalarArray { .. }))
        .expect("the cyclic body retains one scalar-array establishment");
    let place = establishment
        .result
        .structural()
        .expect("array establishment produces a structural result")
        .place;
    assert_eq!(
        establishment.result.structural().unwrap().multiplicity,
        StructuralMultiplicity::Unrestricted
    );
    let call = scan
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::CallStructuralScalar { .. }))
        .expect("the cyclic body retains the owned-argument scalar call");
    let OperationKind::CallStructuralScalar {
        structural_arguments,
        ..
    } = &call.kind
    else {
        unreachable!()
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].place, place);
    assert_eq!(structural_arguments[0].access, StructuralAccess::Owned);

    terminal_verifier::verify_module(module, &lowered.proof_bundle, &AdmissionProfile::default())
        .expect("the cyclic scalar-array machine passes independent verification");

    let semantic = terminal_codec::encode_module(module).expect("encode semantics");
    let proof =
        terminal_codec::encode_proof_section(module, &lowered.proof_bundle).expect("encode proof");
    let unsigned = |value: u64| TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value.into()),
    };
    for remaining in 0..=5u64 {
        let TerminalExecutionResult::Scalar(result) = interpret_terminal_artifact(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[unsigned(remaining), unsigned(7)],
        )
        .expect("re-established cyclic array payload interprets") else {
            panic!("scan returns a scalar")
        };
        // The base traversal's establishment stores `[0, 7]`; `first` reads
        // element 0, so the result is always the deepest `remaining` — zero.
        // Any earlier payload surviving across traversals would leak the
        // entry `remaining` into the result instead.
        assert_eq!(
            result,
            unsigned(0),
            "remaining={remaining}: the deepest traversal's payload wins",
        );
    }
}
