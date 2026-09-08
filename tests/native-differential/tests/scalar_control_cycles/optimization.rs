//! Verified unranked source retains frozen-body custody without progress claims.

use abstract_operations::AbstractOperation;
use abstract_operations_to_abstract_operations::validation::{
    validate_psi_cycle_component_snapshot, validate_transformed_psi_cycle_components,
    validate_verified_psi_cycle_components,
};
use optimization_unit::recompute_psi_optimization_unit_identity;
use optimization_unit_semantics::{
    OptimizationUnitValidationError, validate_psi_optimization_unit,
};
use proof_admission::AdmissionProfile;
use terminal_psi_to_abstract_operations::{
    ArtifactLoweringError, VerifiedPsiOptimizationUnit, build_verified_psi_optimization_unit,
    lower_artifact_sections_for_optimization,
};

fn verified_source(source: &str) -> VerifiedPsiOptimizationUnit {
    let artifact = super::produce(source, false);
    let input = lower_artifact_sections_for_optimization(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
    )
    .expect("unranked artifact has independently verified safety and source custody");
    build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}

fn verified_countdown() -> VerifiedPsiOptimizationUnit {
    verified_source(&super::unranked_selected_call())
}

#[test]
fn unranked_optimizer_admits_verified_source_without_ranking_and_rejects_raw_cycle() {
    let verified = verified_countdown();
    assert!(matches!(
        validate_psi_optimization_unit(verified.unit()),
        Err(OptimizationUnitValidationError::ControlCycle { machine, .. })
            if machine == verified.unit().entry
    ));
    let custody = validate_verified_psi_cycle_components(&verified)
        .expect("verified unranked source admits its exact ordinary control component");
    assert_eq!(custody.components().len(), 1);
    assert!(custody.ranking_certificates().certificates().is_empty());
    assert!(
        verified
            .input()
            .context()
            .proof_bundle()
            .control_cycles
            .is_empty()
    );
    assert_eq!(
        validate_transformed_psi_cycle_components(verified.input(), verified.unit()).unwrap(),
        custody,
    );
    assert_eq!(
        validate_psi_cycle_component_snapshot(
            verified.input(),
            verified.unit(),
            custody.snapshot()
        )
        .unwrap(),
        custody,
    );
}

#[test]
fn unranked_optimizer_freezes_prefix_backedge_call_arithmetic_and_return() {
    let verified = verified_countdown();
    let custody = validate_verified_psi_cycle_components(&verified).unwrap();
    let component = &custody.components()[0];
    let header = component.entries[0].target;
    let backedge = component
        .id
        .internal_edges
        .iter()
        .find(|edge| edge.target == header)
        .unwrap();
    for mutation in ["prefix", "backedge", "call", "arithmetic", "return"] {
        let mut changed = verified.unit().clone();
        let function = changed
            .functions
            .iter_mut()
            .find(|function| function.machine == component.id.machine)
            .unwrap();
        let original_parameter = function.parameters[0].value;
        match mutation {
            "prefix" | "backedge" => {
                let block_id = if mutation == "prefix" {
                    function.entry
                } else {
                    backedge.source
                };
                let node = function
                    .blocks
                    .iter_mut()
                    .find(|block| block.id == block_id)
                    .unwrap()
                    .nodes
                    .last_mut()
                    .unwrap();
                let AbstractOperation::Jump { bindings, .. } = &mut node.operation else {
                    panic!("source countdown stages parameters through a Jump");
                };
                assert!(bindings.len() >= 2);
                assert_ne!(bindings[0].argument, bindings[1].argument);
                let first = bindings[0].argument;
                bindings[0].argument = bindings[1].argument;
                bindings[1].argument = first;
            }
            "call" => {
                let arguments = function
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.nodes)
                    .find_map(|node| match &mut node.operation {
                        AbstractOperation::Call { arguments, .. } => Some(arguments),
                        _ => None,
                    })
                    .unwrap();
                assert_ne!(arguments[0], original_parameter);
                arguments[0] = original_parameter;
            }
            "arithmetic" => {
                let (left, right) = function
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.nodes)
                    .find_map(|node| match &mut node.operation {
                        AbstractOperation::ExactIntegerSubtract { left, right, .. } => {
                            Some((*left, right))
                        }
                        _ => None,
                    })
                    .unwrap();
                assert_ne!(*right, left);
                *right = left;
            }
            "return" => {
                let value = function
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.nodes)
                    .find_map(|node| match &mut node.operation {
                        AbstractOperation::Return { value, .. } => Some(value),
                        _ => None,
                    })
                    .unwrap();
                assert_ne!(*value, original_parameter);
                *value = original_parameter;
            }
            _ => unreachable!(),
        }
        changed.identity = recompute_psi_optimization_unit_identity(&changed);
        assert!(
            matches!(
                validate_transformed_psi_cycle_components(verified.input(), &changed),
                Err(OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch { machine, .. })
                    if machine == component.id.machine
            ),
            "changed {mutation} must reject before revision identity can supply custody"
        );
    }
}

#[test]
fn unranked_optimizer_rejects_redirected_current_backedge() {
    let verified = verified_countdown();
    let custody = validate_verified_psi_cycle_components(&verified).unwrap();
    let component = &custody.components()[0];
    let header = component.entries[0].target;
    let backedge = component
        .id
        .internal_edges
        .iter()
        .find(|edge| edge.target == header)
        .unwrap();
    let mut changed = verified.unit().clone();
    let function = changed
        .functions
        .iter_mut()
        .find(|function| function.machine == component.id.machine)
        .unwrap();
    let node = function
        .blocks
        .iter_mut()
        .find(|block| block.id == backedge.source)
        .unwrap()
        .nodes
        .last_mut()
        .unwrap();
    let AbstractOperation::Jump { target, .. } = &mut node.operation else {
        panic!("source countdown backedge is a Jump");
    };
    *target = function.entry;
    node.successors[0].target = function.entry;
    changed.identity = recompute_psi_optimization_unit_identity(&changed);
    assert!(matches!(
        validate_transformed_psi_cycle_components(verified.input(), &changed),
        Err(OptimizationUnitValidationError::RankedCycleTopologyMismatch { machine })
            if machine == component.id.machine
    ));
}

#[test]
fn unranked_optimizer_rejects_forged_component_members_entries_exits_and_edges() {
    let verified = verified_countdown();
    let custody = validate_verified_psi_cycle_components(&verified).unwrap();
    for mutation in ["members", "entries", "exits", "edges"] {
        let mut changed = custody.snapshot().clone();
        let component = &mut changed.components[0];
        match mutation {
            "members" => component.members.clear(),
            "entries" => component.entries.clear(),
            "exits" => component.exits.clear(),
            "edges" => component.id.internal_edges.clear(),
            _ => unreachable!(),
        }
        assert_ne!(&changed, custody.snapshot(), "{mutation}");
        assert_eq!(
            validate_psi_cycle_component_snapshot(verified.input(), verified.unit(), &changed),
            Err(OptimizationUnitValidationError::RankedCycleComponentSnapshotMismatch),
            "{mutation}",
        );
    }
}

#[test]
fn unranked_optimizer_rejects_a_different_verified_source_under_original_custody() {
    let source = super::unranked_selected_call();
    let verified = verified_source(&source);
    assert_eq!(source.matches("false -> marker").count(), 1);
    let other = verified_source(&source.replacen("false -> marker", "false -> remaining", 1));
    validate_verified_psi_cycle_components(&other).unwrap();
    assert_ne!(verified.unit().psi, other.unit().psi);
    assert!(validate_transformed_psi_cycle_components(verified.input(), other.unit()).is_err());
}

#[test]
fn unranked_optimizer_rejects_missing_safety_proof_and_changed_proof_catalog() {
    let artifact = super::produce(&super::unranked_selected_call(), false);
    let mut proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    assert!(proof.control_cycles.is_empty());
    assert!(
        !proof.evidence.is_empty(),
        "unranked decrement still requires safety evidence"
    );
    proof.evidence.clear();
    let changed_proof = terminal_codec::encode_proof_bundle(&proof).unwrap();
    assert!(matches!(
        lower_artifact_sections_for_optimization(
            artifact.semantic_bytes(),
            &changed_proof,
            &AdmissionProfile::default(),
        ),
        Err(ArtifactLoweringError::Verification(_))
    ));
    let verified = verified_countdown();
    let mut changed = verified.unit().clone();
    assert!(!changed.proof_questions.is_empty());
    changed.proof_questions.clear();
    changed.identity = recompute_psi_optimization_unit_identity(&changed);
    assert!(matches!(
        validate_transformed_psi_cycle_components(verified.input(), &changed),
        Err(OptimizationUnitValidationError::ProofQuestionIndexMismatch)
    ));
}
