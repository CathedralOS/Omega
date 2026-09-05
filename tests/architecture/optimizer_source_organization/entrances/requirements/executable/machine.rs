use super::super::RequiredCoordinationEntrance;

pub(crate) const ENTRANCES: &[RequiredCoordinationEntrance] = &[
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/representations/omega-physical-instructions/src/physical_instructions/costs/mod.rs",
        coordination_marker: "pub fn target_cost_model",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-machine-effects/src/facts/mod.rs",
        coordination_marker: "pub fn analyze_pre_allocation_machine_effects",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-register-homes-to-post-allocation-machine/src/plan/mod.rs",
        coordination_marker: "pub fn analyze_post_allocation_machine_plan",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-optimized-machine/src/rules/peephole_matching/mod.rs",
        coordination_marker: "pub(crate) fn match_instruction_pair",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-optimized-machine/src/rules/aarch64/compare_zero_branch_nonzero/mod.rs",
        coordination_marker: "pub fn optimize_aarch64_compare_i64_zero_branch_nonzero_to_cbnz",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-optimized-machine/src/rules/aarch64/elide_same_view_copy_before_return/mod.rs",
        coordination_marker: "pub fn optimize_aarch64_same_view_copy_i64_before_return",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-optimized-machine/src/rules/aarch64/elide_same_view_copy_before_return/validate/mod.rs",
        coordination_marker: "pub fn validate_aarch64_same_view_copy_elision",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-optimized-machine/src/rules/aarch64/elide_same_view_copy_before_compare_zero/mod.rs",
        coordination_marker: "pub fn optimize_aarch64_same_view_copy_i64_before_compare_zero",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-optimized-machine/src/rules/aarch64/elide_same_view_copy_before_compare_zero/validate.rs",
        coordination_marker: "pub fn validate_aarch64_same_view_copy_i64_before_compare_zero",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-optimized-machine/src/rules/aarch64/elide_same_view_copy_before_compare_i64_left_operand/mod.rs",
        coordination_marker: "pub fn optimize_aarch64_same_view_copy_i64_before_compare_i64_left_operand",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-optimized-machine/src/rules/aarch64/elide_same_view_copy_before_compare_i64_left_operand/validate.rs",
        coordination_marker: "pub fn validate_aarch64_same_view_copy_i64_before_compare_i64_left_operand",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-optimized-machine/src/rules/aarch64/elide_same_view_copy_before_compare_i64_right_operand/mod.rs",
        coordination_marker: "pub fn optimize_aarch64_same_view_copy_i64_before_compare_i64_right_operand",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-optimized-machine/src/rules/aarch64/elide_same_view_copy_before_compare_i64_right_operand/validate.rs",
        coordination_marker: "pub fn validate_aarch64_same_view_copy_i64_before_compare_i64_right_operand",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-optimized-machine/src/rules/aarch64/materialize_i64_movn/mod.rs",
        coordination_marker: "pub fn optimize_aarch64_materialize_i64_with_shortest_movn_seed",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-optimized-machine/src/rules/aarch64/materialize_i64_movn/compute/mod.rs",
        coordination_marker: "pub(crate) fn compute",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-optimized-machine/src/rules/x86_64/materialize_i64_mov_r32_imm32/mod.rs",
        coordination_marker: "pub fn optimize_x86_materialize_i64_with_mov_r32_imm32",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-optimized-machine/src/rules/x86_64/materialize_i64_mov_r64_imm32_sign_extended/mod.rs",
        coordination_marker: "pub fn optimize_x86_materialize_i64_with_mov_r64_imm32_sign_extended",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-optimized-machine/src/rules/x86_64/materialize_i64_xor_zero/mod.rs",
        coordination_marker: "pub fn optimize_x86_materialize_i64_zero_with_xor",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-register-homes-to-post-allocation-machine/src/plan/validate/mod.rs",
        coordination_marker: "pub fn validate_post_allocation_machine_plan",
    },
];
