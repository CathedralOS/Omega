//! Source-reader controls complement native retained/fresh and corruption tests.
//! No fixture-only input may acquire the immutable owner's admission shortcut.

#[test]
fn every_retained_constructor_fully_replays_fresh_source_before_capture() {
    let source = include_str!("../retained.rs");
    let constructors = source.split("impl TryFrom<").skip(1).collect::<Vec<_>>();
    assert_eq!(constructors.len(), 7);
    for constructor in &constructors {
        let replay = constructor
            .find("let replayed = source.replay_allocation()?;")
            .unwrap();
        let policy = constructor
            .find("validate_recovery_selection(&replayed")
            .unwrap();
        let capture = constructor
            .find("CurrentAllocation::from_replayed(&replayed)")
            .unwrap();
        assert!(replay < policy && policy < capture);
    }
    // Only the runtime-spill constructor may bind a recorded prefix's
    // declared selection; every other recovery family has no prefix to
    // report.
    assert_eq!(
        source
            .matches("validate_recovery_selection(&replayed, None)?;")
            .count(),
        6
    );
    assert!(
        source.contains(
            "validate_recovery_selection(&replayed, source.recovery_prefix_selection())?;"
        )
    );
}
