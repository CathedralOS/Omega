//! Default-path route controls: entry-fixed-view transitions recorded in
//! legality are the only direct-assignment failure the leaf-local
//! fixed/precolored sequence may recover.

use selected_instructions_to_selected_instructions::test_support::exercise_single_use_rematerialization;

use crate::RegisterHomeError;
use register_homes::EntryFixedViewTransition;
use register_model::{RegisterOperandAccess, RegisterViewId};
use selected_instructions::{
    LiveRangePoint, LivenessPosition, SelectedInstructionId, VirtualFixedConstraintSite,
};

#[test]
fn entry_transitions_are_the_only_leaf_local_recovery_gate() {
    exercise_single_use_rematerialization(|legality, ranges, physical| {
        // The fixture legality has no transitions, so direct assignment
        // succeeds — the gate is the recorded transition evidence, not the
        // surrounding function shape.
        let before = crate::assignment::home_assignment::compute::compute_function(
            0, legality, ranges, physical,
        );
        assert!(before.is_ok());

        let mut transitioned = legality.clone();
        transitioned.virtual_registers[0]
            .entry_transitions
            .push(EntryFixedViewTransition {
                from_view: RegisterViewId(0),
                to_view: RegisterViewId(1),
                to_site: VirtualFixedConstraintSite::Operand {
                    position: LivenessPosition(3),
                    point: LiveRangePoint(6),
                    instruction: SelectedInstructionId(3),
                    operand: 0,
                    access: RegisterOperandAccess::Use,
                },
            });
        assert!(matches!(
            crate::assignment::home_assignment::compute::compute_function(
                0,
                &transitioned,
                ranges,
                physical,
            ),
            Err(RegisterHomeError::UnresolvedEntryTransitions {
                function: 0,
                register: 0,
                count: 1,
            })
        ));
    });
}

#[test]
fn default_path_routes_entry_transitions_into_the_leaf_local_fixed_view_sequence() {
    let source = include_str!("../register_allocation.rs");
    let entrance = source
        .split("fn stage_register_allocation")
        .nth(1)
        .expect("the stage entrance body");
    let gate = entrance
        .find("UnresolvedEntryTransitions { .. }")
        .expect("the entry-transition recovery gate");
    let sequence = entrance
        .find("stage_leaf_local_fixed_view_register_allocation")
        .expect("the leaf-local fixed-view sequence call");
    let pressure = entrance
        .find("NoCompatibleHome { .. }")
        .expect("the runtime-spill pressure arm");
    let fallback = entrance
        .rfind("OptimizedRegisterHomeCustodyError::Assignment(error)")
        .expect("the unrecovered assignment error surface");
    assert!(gate < sequence && sequence < fallback);
    assert!(pressure < fallback);
}

#[test]
fn shared_entry_route_probes_segment_homes_before_committing_the_sequence() {
    let source = include_str!("../assignment/recovery.rs");
    let entrance = source
        .split("fn fixed_view_allocation")
        .nth(1)
        .expect("the composing sequence body");
    let gate = entrance
        .find("SharedEntryAfterCompareBeforeBranchV1")
        .expect("the shared-entry policy gate");
    let probe = entrance
        .find("probe_optimized_fixed_precolored_segment_homes")
        .expect("the borrow-only segment-home probe");
    let decline = entrance
        .find("recover_after_declined_fixed_view_probe")
        .expect("the declined-sequence spill hand-off");
    let sequence = entrance
        .find("fixed_view_reanalysis(legality, policy)")
        .expect("the custody-consuming sequence");
    let post_copy = entrance
        .find("recover_after_fixed_view_copies")
        .expect("the post-copy spill arm");
    // The probe runs before custody is consumed and the decline hand-off
    // precedes the staged sequence; the post-copy arm stays the later leg.
    assert!(gate < probe && probe < decline && decline < sequence);
    assert!(sequence < post_copy);
}

#[test]
fn declined_fixed_view_recovery_reproves_the_segment_home_probe() {
    let source = include_str!("../assignment/runtime_spill/model.rs");
    let manifest = source
        .split("fn upstream_manifest")
        .nth(1)
        .expect("the source-manifest revalidation");
    let arm = manifest
        .find("Self::DeclinedFixedView")
        .expect("the declined-source arm");
    let probe = manifest
        .find("probe_optimized_fixed_precolored_segment_homes")
        .expect("the replayed probe");
    let verdict = manifest
        .find("capacity_decline()")
        .expect("the required capacity-verdict reproduction");
    let mismatch = manifest
        .find("RuntimeSpillAllocationError::ProbeMismatch")
        .expect("the unearned-policy rejection");
    // Retained replay re-runs the front-end probe and admits only the same
    // recorded capacity verdict; any other outcome rejects the recorded
    // policy.
    assert!(arm < probe && probe < verdict && verdict < mismatch);
}

#[test]
fn active_resident_route_composes_residual_pressure_into_runtime_spill() {
    let source = include_str!("../assignment/recovery.rs");
    let entrance = source
        .split("fn stage_active_resident_register_allocation")
        .nth(1)
        .expect("the active-resident route body");
    let prefix = entrance
        .find("stage_optimized_active_resident_rematerialization_pressure")
        .expect("the proven rematerialization prefix");
    let assignment = entrance
        .find("crate::assign_register_homes(")
        .expect("the post-prefix assignment probe");
    let completion = entrance
        .find("complete_optimized_active_resident_rematerialization")
        .expect("the terminal completion");
    let spill = entrance
        .find("recover_after_active_resident_rematerialization")
        .expect("the residual-pressure spill arm");
    let typed = entrance
        .rfind("OptimizedActiveResidentRematerializationError::Homes(error)")
        .expect("the unrecovered assignment error surface");
    // The sweep is proven before assignment runs; a residual `NoCompatibleHome`
    // hands the same prefix to runtime spill; every other failure keeps the
    // rematerialization-wrapped homes error the one-shot sweep produced.
    assert!(prefix < assignment && assignment < completion && completion < spill);
    assert!(spill < typed);
}

#[test]
fn retained_replay_binds_leaf_local_evidence_to_no_declared_recovery_selection() {
    let source = include_str!("../output/retained.rs");
    let check = source
        .split("fn validate_recovery_selection")
        .nth(1)
        .expect("the recovery-selection check");
    let shared_entry = check
        .find("SharedEntryAfterCompareBeforeBranchV1")
        .expect("the shared-entry policy arm");
    let leaf_local = check
        .find("LeafLocalBeforeFixedUseV1")
        .expect("the leaf-local policy arm");
    assert!(shared_entry < leaf_local);
    assert!(check[..leaf_local].contains("source().source().policy()"));
}
