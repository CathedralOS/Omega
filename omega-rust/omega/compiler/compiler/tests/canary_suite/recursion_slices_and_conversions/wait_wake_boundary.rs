use super::fixture_roster;
use crate::{compile_reviewed_repository_fixture, pass_canary};
use compiler::CheckedCompileRequest;

#[test]
fn wait_wake_boundary_surface_compiles_and_records_the_reach_row() {
    // WAIT-WAKE-SUBSTRATE surface pin: `core::wait_wake` publishes the
    // shared word/value wait plus wake-one/wake-many boundary — `wait`
    // blocks (never suspends) and reports `Woken`/`Mismatched`, and the
    // `reaches WaitWake` row is the declaration every provider binds its
    // realization beneath.
    let canary = pass_canary(fixture_roster::WAIT_WAKE_BOUNDARY_SURFACE);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .unwrap_or_else(|diagnostics| {
        panic!(
            "wait/wake boundary surface should reach checked trees:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });

    // The substrate itself: `wait` publishes blocking (never suspension);
    // the wakes are ordinary boundary calls that never park.
    let wait_wake = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "WaitWake")
        .expect("checked-in `WaitWake` boundary trait");
    assert!(wait_wake.is_boundary, "WaitWake is a boundary trait");
    let signatures = checked.typed.trait_machine_signatures(wait_wake);
    for (name, suspends, blocks) in [
        ("wait", false, true),
        ("wake_one", false, false),
        ("wake_many", false, false),
    ] {
        let signature = signatures
            .iter()
            .find(|signature| signature.name.as_str() == name)
            .unwrap_or_else(|| panic!("checked-in `WaitWake::{name}`"));
        assert_eq!(signature.suspends, suspends, "WaitWake::{name}");
        assert_eq!(signature.blocks, blocks, "WaitWake::{name}");
    }

    // The outcome sum keeps the mismatch observation distinct from a real
    // wake edge — a stale snapshot is never a lost wakeup.
    let outcome = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "WaitOutcome")
        .expect("checked-in `WaitOutcome` declaration");
    let cases: Vec<&str> = checked
        .typed
        .data_members(outcome)
        .iter()
        .map(|member| match member {
            typed_trees::data::DataMember::Variant(variant) => variant.name.as_str(),
            other => panic!("WaitOutcome member should be a case: {other:?}"),
        })
        .collect();
    assert_eq!(cases, ["Woken", "Mismatched"]);

    // The reach row: both harness machines publish exactly `WaitWake`.
    let wait_wake_service = checked
        .typed
        .service_reaches
        .id_for_name("WaitWake")
        .expect("`WaitWake` is an interned boundary service");
    for name in ["WaitHarness::settle", "WaitHarness::poke"] {
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("checked-in machine `{name}`"));
        let services = checked
            .typed
            .service_reach_rows
            .services(machine.service_reach_row);
        assert_eq!(
            services,
            &[wait_wake_service],
            "{name} must publish exactly the `WaitWake` reach row"
        );
    }
}
