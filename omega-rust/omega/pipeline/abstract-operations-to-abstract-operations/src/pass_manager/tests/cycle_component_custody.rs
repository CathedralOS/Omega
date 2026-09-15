//! Validated Terminal-SCC custody for loop consumers on uncertified components.
//!
//! The unranked fixture enters the validated SCC roster without any countdown
//! certificate. Every loop consumer must then key its countdown custody on the
//! certificate-bearing subset of that roster instead of requiring the roster
//! to be the exact countdown slice.

use super::super::VerifiedPsiOptimizationSession;
use super::{AbstractOperation, OptimizationUnitIdentity, verified_unranked_cycle_unit};
use crate::{
    CountedLoopAnalysisError, apply_loop_invariant_scalar_motion,
    propose_loop_invariant_scalar_motion, validate_loop_invariant_scalar_motion,
};
use optimization_unit::{ProvenanceDisposition, PsiProvenance, PsiRealizationSite};
use semantic_vocabulary::{BlockId, MachineId, OperationId};

#[test]
fn unranked_component_enters_the_validated_scc_roster_without_certificates() {
    let session = VerifiedPsiOptimizationSession::new(verified_unranked_cycle_unit()).unwrap();
    let [component] = session.cycle_components().components() else {
        panic!("one validated Terminal SCC")
    };
    assert_eq!(component.id.machine, MachineId::new(501).unwrap());
    assert_eq!(component.members, [BlockId::new(505).unwrap()]);
    assert_eq!(component.id.internal_edges.len(), 1);
    let [entry] = component.entries.as_slice() else {
        panic!("unique entry")
    };
    assert_eq!(entry.source, BlockId::new(503).unwrap());
    let [exit] = component.exits.as_slice() else {
        panic!("single exit")
    };
    assert_eq!(exit.target, BlockId::new(511).unwrap());
    assert!(session.ranking_certificates().certificates().is_empty());
}

#[test]
fn counted_loop_chain_covers_only_certificate_bearing_components() {
    let session = VerifiedPsiOptimizationSession::new(verified_unranked_cycle_unit()).unwrap();
    let counted = session.counted_loop_analysis().unwrap();
    assert!(counted.loops().is_empty());
    session
        .validate_counted_loop_analysis(counted.snapshot())
        .unwrap();
    let invariants = session.countdown_invariant_constant_analysis().unwrap();
    assert!(invariants.loops().is_empty());
    let placements = session
        .countdown_invariant_constant_placement_analysis()
        .unwrap();
    assert!(placements.loops().is_empty());
}

#[test]
fn counted_loop_replay_rejects_forged_rows_on_an_uncertified_roster() {
    let session = VerifiedPsiOptimizationSession::new(verified_unranked_cycle_unit()).unwrap();
    let counted = session.counted_loop_analysis().unwrap();
    let mut forged = counted.snapshot().clone();
    forged.revision = OptimizationUnitIdentity::from_canonical_bytes(b"forged-revision");
    assert_eq!(
        session.validate_counted_loop_analysis(&forged),
        Err(CountedLoopAnalysisError::SnapshotMismatch)
    );
}

#[test]
fn scalar_leaf_motion_relocates_through_validated_scc_custody() {
    let session = VerifiedPsiOptimizationSession::new(verified_unranked_cycle_unit()).unwrap();
    let components = session.cycle_components().components().to_vec();
    let input = session.unit().identity;
    let candidates = propose_loop_invariant_scalar_motion(&session, 4).unwrap();
    let [candidate] = candidates.as_slice() else {
        panic!("one candidate for the single-entry unranked component")
    };
    assert_eq!(candidate.component(), &components[0].id);
    let [relocation] = candidate.relocations() else {
        panic!("the dead integer constant is the one admissible leaf")
    };
    assert_eq!(
        relocation.node().psi_operation(),
        OperationId::new(507).unwrap()
    );
    assert_eq!(
        relocation.node().location().block,
        BlockId::new(505).unwrap()
    );
    assert_eq!(
        relocation.node().provenance().first(),
        Some(&PsiProvenance::Operation(OperationId::new(507).unwrap()))
    );
    assert_eq!(relocation.destination().block, BlockId::new(503).unwrap());

    let validated = validate_loop_invariant_scalar_motion(&session, candidate).unwrap();
    let applied = apply_loop_invariant_scalar_motion(session, validated).unwrap();
    let [record] = applied.ledger().records() else {
        panic!("one transformation record")
    };
    assert_eq!(record.candidate, candidate.identity());
    assert_eq!(record.input, input);
    assert_eq!(record.output, applied.session().unit().identity);
    assert!(record.provenance.iter().any(|row| {
        row.input == PsiRealizationSite::Node(relocation.node().location())
            && row.disposition
                == ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(
                    relocation.destination(),
                ))
            && row.sources.as_slice() == relocation.node().provenance()
            && row.fuel.as_slice() == relocation.node().fuel()
    }));

    // The transformed session keeps the same validated SCC roster and still
    // carries no countdown certificates; the leaf now sits in the preheader.
    let next = applied.session();
    assert_eq!(next.cycle_components().components(), components.as_slice());
    assert!(next.ranking_certificates().certificates().is_empty());
    assert!(next.counted_loop_analysis().unwrap().loops().is_empty());
    let function = &next.unit().functions[0];
    let preheader = function
        .blocks
        .iter()
        .find(|block| block.id == BlockId::new(503).unwrap())
        .unwrap();
    assert!(matches!(
        preheader.nodes[0].operation,
        AbstractOperation::IntegerConstant { .. }
    ));
    let header = function
        .blocks
        .iter()
        .find(|block| block.id == BlockId::new(505).unwrap())
        .unwrap();
    assert!(
        header
            .nodes
            .iter()
            .all(|node| !matches!(node.operation, AbstractOperation::IntegerConstant { .. }))
    );
}
