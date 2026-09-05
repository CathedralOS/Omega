//! Independently replayed rejection of content-authenticated MOVN action corruption.

use omega_post_allocation_machine_to_optimized_machine::{
    Aarch64MovnMaterializationAction, Aarch64MovnMaterializationError,
    aarch64_movn_materialization_identity, validate_aarch64_movn_materialization,
};

use crate::tests::*;

#[test]
fn authenticated_action_corruption_rejects_after_plan_reauthentication() {
    assert_action_corruption_rejects(|action| action.iteration += 1);
    assert_action_corruption_rejects(|action| action.literal_bits ^= 1);
    assert_action_corruption_rejects(|action| action.baseline_word_count -= 1);
    assert_action_corruption_rejects(|action| action.recipe.seed_immediate ^= 1);
    assert_action_corruption_rejects(|action| action.destination.write_units.clear());
}

fn assert_action_corruption_rejects(corrupt: impl FnOnce(&mut Aarch64MovnMaterializationAction)) {
    let realization = super::fixture::staged_realization();
    let allocation = realization.allocation().current();
    assert!(matches!(
        allocation.evidence(),
        omega_selected_instructions_to_register_homes::AllocationEvidence::RegisterHomes(_)
    ));
    let homes = &allocation;
    let StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn(materialization) =
        realization.optimization()
    else {
        panic!("the exact AArch64 selection must produce the MOVN rule result")
    };
    let mut plan = materialization.materialization().plan().clone();
    corrupt(
        plan.actions
            .first_mut()
            .expect("the MOVN fixture must produce one action"),
    );
    plan.identity = aarch64_movn_materialization_identity(&plan);

    let selected_stage = homes;
    assert_eq!(
        validate_aarch64_movn_materialization(
            selected_stage.selected(),
            realization.machine().machine(),
            selected_stage.register_environment().physical(),
            plan,
        ),
        Err(Aarch64MovnMaterializationError::ArtifactMismatch),
        "independent replay must reject action corruption after identity reauthentication",
    );
}
