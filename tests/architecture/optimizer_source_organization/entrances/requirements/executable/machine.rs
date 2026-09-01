use super::super::RequiredCoordinationEntrance;

pub(crate) const ENTRANCES: &[RequiredCoordinationEntrance] = &[
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/mod.rs",
        coordination_marker: "pub fn analyze_pre_allocation_machine_effects",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/planning/post_allocation/mod.rs",
        coordination_marker: "pub fn analyze_post_allocation_machine_plan",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/peephole_matching/mod.rs",
        coordination_marker: "pub(crate) fn match_terminal_pair",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/compare_zero_branch_nonzero/mod.rs",
        coordination_marker: "pub fn optimize_aarch64_compare_i64_zero_branch_nonzero_to_cbnz",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/materialize_i64_movn/mod.rs",
        coordination_marker: "pub fn optimize_aarch64_materialize_i64_with_shortest_movn_seed",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/materialize_i64_movn/compute/mod.rs",
        coordination_marker: "pub(crate) fn compute",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/x86_64/materialize_i64_mov_r32_imm32/mod.rs",
        coordination_marker: "pub fn optimize_x86_materialize_i64_with_mov_r32_imm32",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/x86_64/materialize_i64_xor_zero/mod.rs",
        coordination_marker: "pub fn optimize_x86_materialize_i64_zero_with_xor",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/planning/post_allocation/validate/mod.rs",
        coordination_marker: "pub fn validate_post_allocation_machine_plan",
    },
];
