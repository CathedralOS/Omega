//! Optimizer module role: test leaf. Ranked-certificate counted-loop custody and corruption.

use super::*;

use omega_psi_optimizer::{CountedLoopAnalysisError, CountedLoopAnalysisSnapshot};

#[test]
fn source_countdown_yields_one_revision_bound_exact_trip_count() {
    let (module, verified) = countdown_unit();
    let session =
        VerifiedPsiOptimizationSession::new(verified).expect("verified countdown session");
    let analysis = session
        .counted_loop_analysis()
        .expect("analysis-only counted loop");
    let [summary] = analysis.loops() else {
        panic!("one counted loop")
    };
    let ranked = module.machines[0].ranked_scc.as_ref().unwrap();
    assert_eq!(analysis.snapshot().revision, session.unit().identity);
    assert_eq!(analysis.snapshot().terminal_psi, session.unit().psi);
    assert_eq!(
        summary.certificate.component,
        session.cycle_components().components()[0].id
    );
    assert_eq!(summary.certificate.header, ranked.header);
    assert_eq!(
        summary.members,
        session.cycle_components().components()[0].members
    );
    assert_eq!(
        summary.preheader_edge,
        session.cycle_components().components()[0].entries[0]
    );
    assert_eq!(
        summary.exit_edge,
        session.cycle_components().components()[0].exits[0]
    );
    assert_eq!(summary.trip_count.scalar_type, ranked.rank_type);
    assert_ne!(summary.trip_count.initial_value, ranked.rank_parameter);
}

#[test]
fn counted_loop_snapshot_replay_rejects_every_summary_axis() {
    let (_, verified) = countdown_unit();
    let session =
        VerifiedPsiOptimizationSession::new(verified).expect("verified countdown session");
    let baseline = session
        .counted_loop_analysis()
        .expect("derive counted-loop snapshot")
        .snapshot()
        .clone();
    session
        .validate_counted_loop_analysis(&baseline)
        .expect("exact counted-loop snapshot replays");

    let corruptions: Vec<Box<dyn Fn(&mut CountedLoopAnalysisSnapshot)>> = vec![
        Box::new(|snapshot| {
            snapshot.revision =
                omega_optimization_core::OptimizationUnitIdentity::from_canonical_bytes(
                    b"stale counted-loop revision",
                );
        }),
        Box::new(|snapshot| {
            snapshot.terminal_psi.program_fingerprint =
                psi_terminal::SemanticFingerprint::from_bytes([0xD3; 32]);
        }),
        Box::new(|snapshot| snapshot.loops.clear()),
        Box::new(|snapshot| {
            let duplicate = snapshot.loops[0].clone();
            snapshot.loops.push(duplicate);
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].certificate.component.internal_edges.pop();
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].members.pop();
        }),
        Box::new(|snapshot| snapshot.loops[0].members.reverse()),
        Box::new(|snapshot| {
            snapshot.loops[0].preheader_edge.edge = snapshot.loops[0].exit_edge.edge;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].preheader_edge.source = snapshot.loops[0].certificate.header;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].preheader_edge.target = snapshot.loops[0].exit_edge.target;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].exit_edge.edge = snapshot.loops[0].preheader_edge.edge;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].exit_edge.source = snapshot.loops[0].preheader_edge.source;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].exit_edge.target = snapshot.loops[0].certificate.header;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].trip_count.initial_value =
                snapshot.loops[0].certificate.rank_parameter;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].trip_count.scalar_type =
                psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap();
        }),
    ];
    for mutate in corruptions {
        let mut corrupted = baseline.clone();
        mutate(&mut corrupted);
        assert_eq!(
            session.validate_counted_loop_analysis(&corrupted),
            Err(CountedLoopAnalysisError::SnapshotMismatch)
        );
    }
}
