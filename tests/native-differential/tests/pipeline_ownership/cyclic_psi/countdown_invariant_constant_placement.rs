//! Optimizer module role: test leaf. Exact countdown constant-placement custody.

use super::*;

use abstract_operations_to_abstract_operations::validation::validate_transformed_psi_optimization_unit;
use abstract_operations_to_abstract_operations::{
    CountdownInvariantConstantPlacementAnalysisError,
    CountdownInvariantConstantPlacementAnalysisSnapshot, CountdownInvariantConstantRole,
};
use optimization_unit::recompute_psi_optimization_unit_identity;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, MachineId, ScalarType};

#[test]
fn source_countdown_yields_exact_preheader_destinations_and_consumers() {
    let (_, verified) = countdown_unit();
    let session =
        VerifiedPsiOptimizationSession::new(verified).expect("verified countdown session");
    let analysis = session
        .countdown_invariant_constant_placement_analysis()
        .expect("authenticated placement facts");
    let [loop_placements] = analysis.loops() else {
        panic!("one countdown placement row")
    };
    let [zero, one] = loop_placements.placements.as_slice() else {
        panic!("exact zero and one placement rows")
    };
    let certificate = &loop_placements.counted_loop.certificate;
    let preheader = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == certificate.component.machine)
        .unwrap()
        .blocks
        .iter()
        .find(|block| block.id == loop_placements.counted_loop.preheader_edge.source)
        .unwrap();

    assert_eq!(loop_placements.component, certificate.component);
    assert_eq!(
        zero.constant.role,
        CountdownInvariantConstantRole::PositiveGuardZero
    );
    assert_eq!(
        one.constant.role,
        CountdownInvariantConstantRole::BackedgeDecrementOne
    );
    assert_eq!(zero.destination, one.destination);
    assert_eq!(
        zero.destination.entry_edge,
        loop_placements.counted_loop.preheader_edge
    );
    assert_eq!(
        zero.destination.before.machine,
        certificate.component.machine
    );
    assert_eq!(zero.destination.before.block, preheader.id);
    assert_eq!(
        usize::try_from(zero.destination.before.node).unwrap(),
        preheader.nodes.len() - 1
    );
    assert_eq!(
        zero.consumer.psi_operation,
        certificate.guard.comparison_operation
    );
    assert_eq!(zero.consumer.location.block, certificate.header);
    assert_eq!(zero.consumer.value_use.value, zero.constant.result);
    assert_eq!(
        one.consumer.psi_operation,
        certificate.descent.subtract_operation
    );
    assert_eq!(
        one.consumer.location.block,
        certificate.descent.backedge.source
    );
    assert_eq!(one.consumer.value_use.value, one.constant.result);
    session
        .validate_countdown_invariant_constant_placement_analysis(analysis.snapshot())
        .expect("exact placement snapshot replays");
}

#[test]
fn placement_snapshot_replay_rejects_every_retained_axis() {
    let (_, verified) = countdown_unit();
    let session =
        VerifiedPsiOptimizationSession::new(verified).expect("verified countdown session");
    let baseline = session
        .countdown_invariant_constant_placement_analysis()
        .expect("derive placement facts")
        .snapshot()
        .clone();
    let corruptions: Vec<Box<dyn Fn(&mut CountdownInvariantConstantPlacementAnalysisSnapshot)>> = vec![
        Box::new(|snapshot| {
            snapshot.terminal_psi.program_fingerprint =
                terminal_psi::SemanticFingerprint::from_bytes([0xB7; 32]);
        }),
        Box::new(|snapshot| snapshot.loops.clear()),
        Box::new(|snapshot| {
            let duplicate = snapshot.loops[0].clone();
            snapshot.loops.push(duplicate);
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].component.internal_edges.pop();
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].counted_loop.region.header = None;
        }),
        Box::new(|snapshot| snapshot.loops[0].placements.clear()),
        Box::new(|snapshot| {
            let duplicate = snapshot.loops[0].placements[0].clone();
            snapshot.loops[0].placements.push(duplicate);
        }),
        Box::new(|snapshot| snapshot.loops[0].placements.reverse()),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0].constant.role =
                CountdownInvariantConstantRole::BackedgeDecrementOne;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0].constant.location.machine =
                MachineId::new(97_001).unwrap();
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0].constant.psi_operation = snapshot.loops[0]
                .counted_loop
                .certificate
                .guard
                .comparison_operation;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0].constant.result =
                snapshot.loops[0].placements[1].constant.result;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0].constant.scalar_type =
                IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0].constant.value = IntegerValue::Unsigned(1);
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0]
                .constant
                .definition
                .scalar_type =
                ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0].constant.provenance.clear();
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0].constant.fuel[0].units += 1;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0].constant.effect.input += 1;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0].destination.before.machine =
                MachineId::new(97_002).unwrap();
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0].destination.before.block =
                snapshot.loops[0].counted_loop.certificate.header;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0].destination.before.node += 1;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0].destination.entry_edge.edge =
                snapshot.loops[0].counted_loop.exit_edge.edge;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0]
                .destination
                .entry_edge
                .source = snapshot.loops[0].counted_loop.certificate.header;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0]
                .destination
                .entry_edge
                .target = snapshot.loops[0].counted_loop.exit_edge.target;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0].consumer.location.machine =
                MachineId::new(97_003).unwrap();
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0].consumer.location.block =
                snapshot.loops[0].placements[1].consumer.location.block;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0].consumer.location.node += 1;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0].consumer.psi_operation =
                snapshot.loops[0].placements[0].constant.psi_operation;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0].consumer.value_use.value =
                snapshot.loops[0].placements[1].constant.result;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0].consumer.value_use.block =
                snapshot.loops[0].placements[1].consumer.value_use.block;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].placements[0].consumer.value_use.node += 1;
        }),
    ];
    assert_eq!(corruptions.len(), 31);
    for mutate in corruptions {
        let mut corrupted = baseline.clone();
        mutate(&mut corrupted);
        assert_eq!(
            session.validate_countdown_invariant_constant_placement_analysis(&corrupted),
            Err(CountdownInvariantConstantPlacementAnalysisError::SnapshotMismatch)
        );
    }
}

#[test]
fn stale_placement_revision_fails_typed() {
    let (_, verified) = countdown_unit();
    let session =
        VerifiedPsiOptimizationSession::new(verified).expect("verified countdown session");
    let mut stale = session
        .countdown_invariant_constant_placement_analysis()
        .expect("derive placement facts")
        .snapshot()
        .clone();
    stale.revision = optimization_core::OptimizationUnitIdentity::from_canonical_bytes(
        b"stale countdown placement revision",
    );
    assert_eq!(
        session.validate_countdown_invariant_constant_placement_analysis(&stale),
        Err(
            CountdownInvariantConstantPlacementAnalysisError::CandidateRevisionMismatch {
                candidate: stale.revision,
                current: session.unit().identity,
            }
        )
    );
}

#[test]
fn acyclic_session_has_no_countdown_constant_placements() {
    let verified = super::countdown_invariant_constants::acyclic_unit();
    let session = VerifiedPsiOptimizationSession::new(verified).expect("verified acyclic session");
    let analysis = session
        .countdown_invariant_constant_placement_analysis()
        .expect("empty ranked custody has empty placements");
    assert!(analysis.loops().is_empty());
    session
        .validate_countdown_invariant_constant_placement_analysis(analysis.snapshot())
        .expect("empty placement snapshot replays");
}

#[test]
fn placement_analysis_does_not_bypass_the_ranked_component_freeze() {
    let (_, verified) = countdown_unit();
    let session =
        VerifiedPsiOptimizationSession::new(verified).expect("verified countdown session");
    let placement = session
        .countdown_invariant_constant_placement_analysis()
        .expect("analysis-only placement custody")
        .loops()[0]
        .placements[0]
        .clone();
    let (input, mut unit) = session.into_parts();
    let node = &mut unit
        .functions
        .iter_mut()
        .find(|function| function.machine == placement.constant.location.machine)
        .unwrap()
        .blocks
        .iter_mut()
        .find(|block| block.id == placement.constant.location.block)
        .unwrap()
        .nodes[usize::try_from(placement.constant.location.node).unwrap()];
    node.fuel[0].units += 1;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);

    assert!(matches!(
        validate_transformed_psi_optimization_unit(&input, &unit),
        Err(OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
            machine,
            block,
        }) if machine == placement.constant.location.machine
            && block == placement.constant.location.block
    ));
}
