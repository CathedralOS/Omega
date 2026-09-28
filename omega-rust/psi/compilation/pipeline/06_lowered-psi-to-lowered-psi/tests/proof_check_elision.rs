//! Stage-level coverage for the exact `ProofCheckElision` rule through the
//! public `run_psi_optimization` entrance.

use crate::common;

use common::{
    obligation, operation_id, proof_check_fixture, undischarged_proof_check_fixture, value,
};
use lowered_psi_to_lowered_psi::run_psi_optimization;
use optimization::{PsiOptimization, PsiOptimizationSelections};
use semantic_vocabulary::{EvidenceIdentity, RankingRelationId, RecursiveComponentId};
use terminal_psi::{
    EvidenceRoute, OperationKind, PrimitiveJudgment, RecursiveComponentCertificate,
    RecursiveComponentEvidence, RecursiveEdgeCertificate,
};
use terminal_verifier::{ProofCheckElisionRewriteError, validate_proof_check_elision};

fn selections() -> PsiOptimizationSelections {
    PsiOptimizationSelections::new([PsiOptimization::ProofCheckElision]).unwrap()
}

#[test]
fn undischarged_goals_publish_an_identity() {
    let lowered = undischarged_proof_check_fixture();
    let optimized = run_psi_optimization(lowered.clone(), selections()).expect("elision executes");
    assert_eq!(optimized.lowered(), &lowered);
    assert_eq!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
    );
    assert_eq!(
        optimized.execution().input_proof(),
        optimized.execution().output_proof()
    );
}

#[test]
fn certificate_reserved_obligation_keeps_its_check() {
    // An obligation a recursive-component certificate names is consumed by
    // that certificate: the discharged leaf keeps its question.
    let mut lowered = proof_check_fixture();
    lowered
        .proof_bundle
        .recursive_components
        .push(RecursiveComponentEvidence {
            component: RecursiveComponentId::new(1).unwrap(),
            certificate: RecursiveComponentCertificate {
                identity: EvidenceIdentity::new(1).unwrap(),
                ranking_relation: RankingRelationId::new(1).unwrap(),
                well_foundedness: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
                edges: vec![RecursiveEdgeCertificate {
                    obligation: obligation(1),
                    evidence: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
                }],
            },
        });
    let optimized = run_psi_optimization(lowered.clone(), selections()).expect("elision executes");
    let block = &optimized.lowered().semantic_module.machines[0].blocks[0];
    assert_eq!(
        block.operations[2].kind,
        OperationKind::ExactIntegerSubtract {
            left: value(1),
            right: value(1),
            obligation: obligation(1)
        },
        "the certificate-reserved leaf stays checked"
    );
    assert_eq!(
        block.operations[3].kind,
        OperationKind::WrappingIntegerSubtract {
            left: value(1),
            right: value(10)
        },
        "unreserved leaves still fold"
    );
    assert!(
        optimized
            .lowered()
            .proof_bundle
            .evidence
            .iter()
            .any(|row| row.obligation == obligation(1)),
        "the reserved obligation's evidence row survives"
    );
    assert_eq!(
        optimized.lowered().proof_bundle.recursive_components,
        lowered.proof_bundle.recursive_components,
        "the certificate itself is untouched"
    );
}

#[test]
fn elision_preserves_debug_sites_for_unchanged_rows() {
    // The rewrite keeps every operation and value identity, so debug sites
    // survive in place while the sidecar's semantic identity is recomputed.
    let mut lowered = proof_check_fixture();
    common::with_debug_sites(
        &mut lowered,
        &[
            terminal_psi::DebugSubject::Operation(operation_id(11)),
            terminal_psi::DebugSubject::Operation(operation_id(16)),
            terminal_psi::DebugSubject::Value(value(11)),
            terminal_psi::DebugSubject::Value(value(14)),
        ],
    );
    let before_sites = lowered.debug_map.as_ref().unwrap().sites.clone();
    let optimized = run_psi_optimization(lowered, selections()).unwrap();
    let debug = optimized.lowered().debug_map.as_ref().unwrap();
    assert_eq!(
        debug.sites, before_sites,
        "no site drops: every row keeps its identity"
    );
    assert_eq!(
        debug.semantic,
        terminal_codec::terminal_psi_identity(&optimized.lowered().semantic_module).unwrap()
    );
}

#[test]
fn verifier_rejects_a_fold_the_original_goal_does_not_discharge() {
    // Replay is derived from the original module: a producer that folds a
    // symbolic goal is caught, not trusted.
    let lowered = proof_check_fixture();
    let optimized = run_psi_optimization(lowered.clone(), selections()).unwrap();
    let mut forged = optimized.lowered().semantic_module.clone();
    forged.machines[0].blocks[0].operations[5].kind = OperationKind::WrappingIntegerAdd {
        left: value(1),
        right: value(1),
    };
    assert!(matches!(
        validate_proof_check_elision(
            &lowered.semantic_module,
            &forged,
            &lowered.proof_bundle,
            &optimized.lowered().proof_bundle,
        ),
        Err(ProofCheckElisionRewriteError::ChangedMachine(_))
    ));
}

#[test]
fn verifier_rejects_evidence_dropped_for_a_surviving_obligation() {
    let lowered = proof_check_fixture();
    let optimized = run_psi_optimization(lowered.clone(), selections()).unwrap();
    let mut forged_bundle = optimized.lowered().proof_bundle.clone();
    forged_bundle
        .evidence
        .retain(|row| row.obligation != obligation(4));
    assert!(matches!(
        validate_proof_check_elision(
            &lowered.semantic_module,
            &optimized.lowered().semantic_module,
            &lowered.proof_bundle,
            &forged_bundle,
        ),
        Err(ProofCheckElisionRewriteError::ChangedProofEvidence)
    ));
}

#[test]
fn discharged_checks_survive_when_proof_check_elision_is_not_selected() {
    // Disabled coverage: the discharged workload under a CopyPropagation-only
    // selection keeps every goal-bearing row — elision is not selected.
    let lowered = proof_check_fixture();
    let optimized = run_psi_optimization(
        lowered.clone(),
        PsiOptimizationSelections::new([PsiOptimization::CopyPropagation]).unwrap(),
    )
    .expect("the stage executes");
    assert_eq!(optimized.lowered(), &lowered);
    assert_eq!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
    );
    assert_eq!(
        optimized.execution().input_proof(),
        optimized.execution().output_proof()
    );
}

#[test]
fn structurally_invalid_inputs_fail_before_rewrite() {
    // Corruption coverage: each malformed carrier fails closed at the
    // module-validation gate before any rewrite decision.
    let mut no_machines = proof_check_fixture();
    no_machines.semantic_module.machines.clear();
    assert!(matches!(
        run_psi_optimization(no_machines, selections()),
        Err(
            lowered_psi_to_lowered_psi::PsiOptimizationStageError::InvalidModule(
                terminal_verifier::ModuleError::EmptyModule
            )
        )
    ));

    let mut bad_target = undischarged_proof_check_fixture();
    let terminal_psi::Terminator::Return {
        value: returned, ..
    } = &mut bad_target.semantic_module.machines[0].blocks[0].terminator
    else {
        panic!("b1 is a return")
    };
    *returned = value(99);
    assert!(matches!(
        run_psi_optimization(bad_target, selections()),
        Err(lowered_psi_to_lowered_psi::PsiOptimizationStageError::InvalidModule(_))
    ));

    let mut bad_debug = proof_check_fixture();
    common::with_debug_sites(&mut bad_debug, &[]);
    bad_debug
        .debug_map
        .as_mut()
        .unwrap()
        .semantic
        .program_fingerprint = terminal_psi::SemanticFingerprint::from_bytes([0xff; 32]);
    assert!(matches!(
        run_psi_optimization(bad_debug, selections()),
        Err(lowered_psi_to_lowered_psi::PsiOptimizationStageError::InvalidDebugMap(_))
    ));
}

#[test]
fn execution_is_deterministic_across_runs() {
    let first = run_psi_optimization(proof_check_fixture(), selections()).unwrap();
    let second = run_psi_optimization(proof_check_fixture(), selections()).unwrap();
    assert_eq!(first, second);
}
