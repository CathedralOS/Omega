//! Source-reader controls complement native retained/fresh and corruption tests.
//! No fixture-only input may acquire the immutable owner's admission shortcut.

#[test]
fn every_retained_constructor_fully_replays_fresh_source_before_capture() {
    let source = include_str!("../retained.rs");
    let constructors = source.split("impl TryFrom<").skip(1).collect::<Vec<_>>();
    assert_eq!(constructors.len(), 6);
    for constructor in constructors {
        let replay = constructor
            .find("let replayed = source.replay_allocation()?;")
            .unwrap();
        let policy = constructor
            .find("validate_recovery_selection(&replayed)?;")
            .unwrap();
        let capture = constructor
            .find("CurrentAllocation::from_replayed(&replayed)")
            .unwrap();
        assert!(replay < policy && policy < capture);
    }
}

#[test]
fn immutable_retained_reads_rejoin_all_facts_without_reexecuting_source_replay() {
    let source = include_str!("../retained.rs");
    let retained = source
        .split("impl AllocationSource for RetainedAllocation {")
        .nth(1)
        .unwrap()
        .split("impl TryFrom<")
        .next()
        .unwrap();
    assert_eq!(retained.matches("source.project_allocation()").count(), 5);
    assert_eq!(
        retained
            .matches("source.project_replayed_allocation()?")
            .count(),
        1
    );
    assert!(!retained.contains("source.replay_allocation()"));
    assert!(retained.contains("validate_recovery_selection(&current)?;"));
    assert!(retained.contains("self.current.validate_against(&current)?;"));

    let runtime = include_str!("../runtime_spill.rs");
    let fresh = runtime
        .split("impl RuntimeSpillAllocation {")
        .next()
        .unwrap();
    assert!(
        fresh.contains("replay::validate(self).map_err(AllocationReplayError::RuntimeSpill)?;")
    );
    assert!(fresh.contains("self.project_replayed_allocation()"));
    assert!(runtime.contains(".ok_or(AllocationReplayError::ReceiptMismatch)?"));
}
