//! Source-reader controls for the retained allocation's constructors.

#[test]
fn every_retained_constructor_checks_the_recovery_selection_before_capture() {
    let source = include_str!("../retained.rs");
    let constructors = source.split("impl TryFrom<").skip(1).collect::<Vec<_>>();
    assert_eq!(constructors.len(), 7);
    for constructor in &constructors {
        assert!(constructor.contains("Self::capture("));
    }
    let capture = source
        .split("fn capture(")
        .nth(1)
        .expect("the shared capture");
    let policy = capture
        .find("validate_recovery_selection(current")
        .expect("the recovery-selection check");
    let owned = capture
        .find("CurrentAllocation::capture(current)")
        .expect("the owned current facts");
    assert!(policy < owned);
    // Only the runtime-spill constructor may bind a recorded prefix's
    // declared selection; every other recovery family has no prefix to
    // report.
    assert_eq!(
        source
            .matches("Self::capture(&source.project_allocation(), None)")
            .count(),
        6
    );
    assert!(source.contains("source.recovery_prefix_selection(),"));
    // Allocation is consumed as produced: no constructor replays a source.
    assert!(!source.contains("replay_allocation"));
}
