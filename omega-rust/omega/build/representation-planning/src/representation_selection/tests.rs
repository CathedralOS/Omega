//! Custody mutation matrix for retained opaque-representation selections.
//!
//! [`rederive_opaque_representation_selections`] is the independent replay of
//! the build-time `select_representation` custody: it reruns the harvest over
//! the final typed graph and compares the derived rows against the retained
//! build-evaluation custody before downstream property-receipt and
//! boundary-plan consumers may trust them. `OpaqueRepresentationSelection` is
//! an in-memory custody record, not a wire codec: its authenticated identity
//! is the domain-separated `selected_application_commitment`, recomputed by
//! `rederived_selected_application_commitment` from exactly the committed
//! fields (the closed application's commitment, lifecycle, copy disposition,
//! and origin). Every remaining field -- `opaque`, `carrier`,
//! `selecting_machine`, `source_span`, and the un-hashed application members
//! -- is joined by exact structural equality: `derived != retained` rejects.
//!
//! The one-field substitution matrix runs through the shared
//! `run_one_field_substitution_matrix` driver over the declared
//! `OpaqueRepresentationSelectionFieldForTest` inventory in `test_support`:
//! each leg mutates one lane through `from_validated_application` (recomputing
//! the containing commitment honestly) and the leg's outcome names the custody
//! arm that must reject it -- the moved-commitment row join, the
//! provenance-lane row join, or the stale build-machine custody arm.
//! `lifecycle` and `origin` have exactly one representable variant each and
//! are pinned by exhaustive matches rather than mutations. A substituted
//! stored `selected_application_commitment` is only forgeable inside the
//! representation crate; `representation_selections`' own tests pin that
//! stored-vs-rederived divergence, while this matrix exercises the
//! stale-custody arm through `selecting_machine`.

use super::rederive_opaque_representation_selections;
use super::test_support::{
    OpaqueRepresentationSelectionFieldForTest, RetainedSelectionCustodyCase, SelectionParts,
    check_retained_selection_custody, corrupt_retained_selection_custody_for_test,
    expect_rejection, fixture, foreign_retained_selection_donor,
    honest_retained_selection_custody_case, replay, retained_selection_custody_outcome,
    retained_selection_joined_replay,
};
use crate::{
    OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION, OpaqueRepresentationApplicationOrigin,
    OpaqueRepresentationCopyDisposition, OpaqueRepresentationLifecycleDisposition,
};
use arena::HandleSpan;
use optimization_core::OneFieldSubstitutionMatrix;
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::typed_trees::ClosedConformanceApplicationCommitment;

#[test]
fn honest_fixture_replays_retained_custody() {
    let fixture = fixture();
    let [token, marker] = fixture.selections.as_slice() else {
        unreachable!("fixture guarantees two selections");
    };
    assert_eq!(
        token.copy_disposition(),
        OpaqueRepresentationCopyDisposition::PlacementOnly
    );
    assert_eq!(
        marker.copy_disposition(),
        OpaqueRepresentationCopyDisposition::CheckedSemanticCopy
    );
    for selection in &fixture.selections {
        assert_eq!(
            selection.selected_application_commitment(),
            selection.rederived_selected_application_commitment(),
            "stored commitment must equal the honest recomputation",
        );
        assert_eq!(
            selection.schema_version(),
            OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION
        );
        // `lifecycle` and `origin` each have exactly one representable value,
        // so no substitution axis exists. These exhaustive matches pin that
        // contract: a new variant fails compilation until its mutation
        // coverage lands.
        match selection.lifecycle() {
            OpaqueRepresentationLifecycleDisposition::Inert => {}
        }
        match selection.origin() {
            OpaqueRepresentationApplicationOrigin::NamedConformance => {}
        }
    }
    // Forward direction: independent replay rederives the identical roster.
    let derived = replay(&fixture, &fixture.selections).expect("honest custody must replay");
    assert_eq!(derived, fixture.selections);
}

#[test]
fn retained_row_rejects_every_one_field_substitution() {
    optimization_core::run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "retained opaque representation selection",
        fields: OpaqueRepresentationSelectionFieldForTest::INVENTORY,
        honest: &honest_retained_selection_custody_case,
        donor: foreign_retained_selection_donor(),
        custody: &|case: &RetainedSelectionCustodyCase| case.row.clone(),
        substitute: &corrupt_retained_selection_custody_for_test,
        check: &check_retained_selection_custody,
        outcome: &retained_selection_custody_outcome,
        joined_replay: Some(&retained_selection_joined_replay),
    });
}

#[test]
fn retained_roster_rejects_omission_duplication_reorder_and_extension() {
    let fixture = fixture();
    let [first, second] = fixture.selections.as_slice() else {
        unreachable!("fixture guarantees two selections");
    };
    let expected = "rederivation disagrees with retained build custody";

    // Omission: the whole roster and a single dropped row.
    expect_rejection(replay(&fixture, &[]), expected);
    expect_rejection(replay(&fixture, &[first.clone()]), expected);
    expect_rejection(replay(&fixture, &[second.clone()]), expected);

    // Duplication: one retained row appearing twice.
    expect_rejection(
        replay(&fixture, &[first.clone(), second.clone(), first.clone()]),
        expected,
    );

    // Reorder: retained custody binds statement order.
    expect_rejection(replay(&fixture, &[second.clone(), first.clone()]), expected);

    // Foreign row substitution and extension: a syntactically valid row that
    // never occurred in the build cannot join either position or the tail.
    let mut foreign_parts = SelectionParts::of(second);
    foreign_parts.opaque = fixture.foreign_data;
    foreign_parts.carrier = fixture.foreign_data;
    foreign_parts.application.declaration = fixture.foreign_trait;
    foreign_parts.application.commitment =
        ClosedConformanceApplicationCommitment::from_digest([0x5f; 32]);
    foreign_parts.source_span = fixture.other_source_span;
    let foreign = foreign_parts.assemble();
    assert_eq!(
        foreign.selected_application_commitment(),
        foreign.rederived_selected_application_commitment(),
    );
    expect_rejection(
        replay(&fixture, &[foreign.clone(), second.clone()]),
        expected,
    );
    expect_rejection(
        replay(&fixture, &[first.clone(), second.clone(), foreign.clone()]),
        expected,
    );
}

#[test]
fn replay_requires_exactly_one_authoritative_build_machine() {
    let fixture = fixture();

    // No authoritative build machine: retained custody cannot survive.
    expect_rejection(
        rederive_opaque_representation_selections(&fixture.typed, None, &fixture.selections),
        "selections exist without an authoritative build machine",
    );

    // The settlement selects a different machine than the retained rows claim:
    // the stale-custody arm fires before the machine lookup runs.
    expect_rejection(
        rederive_opaque_representation_selections(
            &fixture.typed,
            Some(fixture.foreign_machine),
            &fixture.selections,
        ),
        "stale build-machine or application custody",
    );

    // Rows honestly rebuilt under a machine absent from the typed graph map
    // to zero final declarations.
    let foreign_owned = fixture
        .selections
        .iter()
        .map(|selection| {
            let mut parts = SelectionParts::of(selection);
            parts.selecting_machine = fixture.foreign_machine;
            parts.assemble()
        })
        .collect::<Vec<_>>();
    expect_rejection(
        rederive_opaque_representation_selections(
            &fixture.typed,
            Some(fixture.foreign_machine),
            &foreign_owned,
        ),
        "map to 0 final build-machine declarations; expected one",
    );

    // A duplicated authoritative machine maps to two final declarations.
    let mut typed = fixture.typed.clone();
    typed.push_machine(Machine {
        symbol: fixture.build_machine,
        name: Identifier::generated("build-duplicate"),
        ..Default::default()
    });
    expect_rejection(
        rederive_opaque_representation_selections(
            &typed,
            Some(fixture.build_machine),
            &fixture.selections,
        ),
        "map to 2 final build-machine declarations; expected one",
    );
}

#[test]
fn replay_recomputes_from_current_program_evidence() {
    let fixture = fixture();
    let expected = "rederivation disagrees with retained build custody";

    // Erased evidence: strip the selection statements from the authoritative
    // machine. The retained roster has no surviving program support, so the
    // mutated-to-original direction rejects too.
    let mut typed = fixture.typed.clone();
    let states = typed.machines()[0].states;
    typed.machine_states.span_mut_or_empty(states)[0].statement_nodes = HandleSpan::empty();
    expect_rejection(
        rederive_opaque_representation_selections(
            &typed,
            Some(fixture.build_machine),
            &fixture.selections,
        ),
        expected,
    );

    // Reordered evidence: the derived roster follows statement order, so a
    // swapped program cannot satisfy the retained ordering either.
    let mut typed = fixture.typed.clone();
    let states = typed.machines()[0].states;
    let statement_nodes = typed.machine_states.span_or_empty(states)[0].statement_nodes;
    let reordered = typed
        .statement_table
        .statements(statement_nodes)
        .iter()
        .rev()
        .cloned()
        .collect::<Vec<_>>();
    let mut reversed = HandleSpan::empty();
    for statement in reordered {
        typed
            .statement_table
            .push_statement(&mut reversed, statement);
    }
    typed.machine_states.span_mut_or_empty(states)[0].statement_nodes = reversed;
    expect_rejection(
        rederive_opaque_representation_selections(
            &typed,
            Some(fixture.build_machine),
            &fixture.selections,
        ),
        expected,
    );
}
