use super::CompilerRequestOutcome;
use crate::{BatchChildOutcome, CompileOutcomes, CompileReport, CompileTargetOutcome};
use diagnostics::{Diagnostic, ExceededProvision};
use target::TargetProfile;

fn roster_rounds(obligation: u64) -> ExceededProvision {
    ExceededProvision {
        resource: "proof_search_roster_rounds",
        limit: 128,
        requested: 129,
        coordinate: obligation,
    }
}

fn capacity_refusal(obligation: u64) -> Diagnostic {
    Diagnostic::incomplete(
        format!("incomplete: search stopped with obligation {obligation} unproved"),
        roster_rounds(obligation),
    )
}

/// A refusal is `Incomplete` only when every error names an exceeded
/// provision; one ordinary error keeps it a rejection. The halt values are
/// the compiler request's outcome tags.
#[test]
fn refusal_outcome_is_incomplete_only_when_every_error_names_a_provision() {
    assert_eq!(
        CompilerRequestOutcome::of_refusal(&[capacity_refusal(41)]),
        CompilerRequestOutcome::Incomplete(roster_rounds(41)),
    );
    // The first provision in diagnostic order is the one the outcome names;
    // a warning neither decides nor blocks the class.
    assert_eq!(
        CompilerRequestOutcome::of_refusal(&[
            Diagnostic::warning("unused binding"),
            capacity_refusal(7),
            capacity_refusal(9),
        ]),
        CompilerRequestOutcome::Incomplete(roster_rounds(7)),
    );
    assert_eq!(
        CompilerRequestOutcome::of_refusal(&[
            capacity_refusal(41),
            Diagnostic::error("cannot prove the index is in bounds"),
        ]),
        CompilerRequestOutcome::Reject,
    );
    assert_eq!(
        CompilerRequestOutcome::of_refusal(&[Diagnostic::error(
            "incomplete: a message spelling is not a provision"
        )]),
        CompilerRequestOutcome::Reject,
    );
    assert_eq!(
        CompilerRequestOutcome::of_refusal(&[]),
        CompilerRequestOutcome::Reject
    );
    assert_eq!(CompilerRequestOutcome::Complete.halt_value(), 0);
    assert_eq!(CompilerRequestOutcome::Reject.halt_value(), 1);
    assert_eq!(
        CompilerRequestOutcome::Incomplete(roster_rounds(41)).halt_value(),
        2
    );
}

/// The batch manifest records a capacity-exhausted child as `Incomplete`
/// with its named provision, distinct from a rejection carrying the same
/// wording and from the same refusal against a different obligation.
#[test]
fn batch_manifest_records_an_incomplete_child_beside_its_siblings() {
    let outcomes = CompileOutcomes::new(vec![
        CompileTargetOutcome::new(
            Some(TargetProfile::LinuxX64),
            Ok(CompileReport::check_only("first.omg".into(), 1).unwrap()),
        ),
        CompileTargetOutcome::new(
            Some(TargetProfile::MacosArm64),
            Err(vec![capacity_refusal(41)]),
        ),
        CompileTargetOutcome::new(
            Some(TargetProfile::WindowsX64),
            Err(vec![Diagnostic::error("second target rejected")]),
        ),
    ])
    .unwrap();
    assert!(outcomes.batch_manifest().validate());
    assert_eq!(
        outcomes.outcomes()[0].outcome(),
        CompilerRequestOutcome::Complete
    );
    assert_eq!(
        outcomes.outcomes()[1].outcome(),
        CompilerRequestOutcome::Incomplete(roster_rounds(41)),
    );
    assert_eq!(
        outcomes.outcomes()[2].outcome(),
        CompilerRequestOutcome::Reject
    );
    let rows = outcomes.batch_manifest().rows();
    assert!(matches!(
        rows[1].outcome(),
        BatchChildOutcome::Incomplete { provision, .. } if *provision == roster_rounds(41)
    ));
    assert!(matches!(
        rows[2].outcome(),
        BatchChildOutcome::Rejected { .. }
    ));

    let identity = |diagnostic: Diagnostic| {
        CompileOutcomes::new(vec![CompileTargetOutcome::new(None, Err(vec![diagnostic]))])
            .unwrap()
            .batch_manifest()
            .identity()
    };
    let incomplete = identity(capacity_refusal(41));
    let mut same_words = capacity_refusal(41);
    same_words.exceeded_provision = None;
    assert_ne!(incomplete, identity(same_words));
    let mut other_obligation = capacity_refusal(41);
    other_obligation.exceeded_provision = Some(roster_rounds(42));
    assert_ne!(incomplete, identity(other_obligation));
    assert_eq!(incomplete, identity(capacity_refusal(41)));
}
