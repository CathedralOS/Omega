use super::super::RequiredCoordinationEntrance;

pub(crate) const ENTRANCES: &[RequiredCoordinationEntrance] = &[
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/representations/physical-instructions/src/physical_instructions/costs/mod.rs",
        coordination_marker: "pub fn target_cost_model",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/machine_effects/facts/mod.rs",
        coordination_marker: "pub fn analyze_pre_allocation_machine_effects",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/register-homes-to-post-allocation-machine/src/plan/mod.rs",
        coordination_marker: "pub fn analyze_post_allocation_machine_plan",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/register-homes-to-post-allocation-machine/src/plan/validate/mod.rs",
        coordination_marker: "pub fn validate_post_allocation_machine_plan",
    },
];
