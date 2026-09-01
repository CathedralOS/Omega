use super::super::SemanticLadder;

pub(crate) const LADDERS: &[SemanticLadder] = &[
    SemanticLadder {
        family: "non-authoritative target cost model",
        paths: &[
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/costs/mod.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/costs/model.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/costs/identity.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/costs/tests.rs",
        ],
    },
    SemanticLadder {
        family: "pre-allocation machine-effect codec",
        paths: &[
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/codec/mod.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/codec/cursor.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/codec/error.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/codec/v6/mod.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/codec/v6/framing.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/codec/v6/instruction.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/codec/v6/ownership.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/codec/v6/structural.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/codec/v6/values.rs",
        ],
    },
    SemanticLadder {
        family: "declarative terminal-pair matching",
        paths: &[
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/peephole_matching/mod.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/peephole_matching/model.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/peephole_matching/registers.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/peephole_matching/instruction.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/peephole_matching/liveness.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/compare_zero_branch_nonzero/pattern.rs",
        ],
    },
    SemanticLadder {
        family: "AArch64 MOVN proposal",
        paths: &[
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/materialize_i64_movn/compute/mod.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/materialize_i64_movn/compute/budget.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/materialize_i64_movn/compute/materialization.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/materialize_i64_movn/compute/recipe.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/materialize_i64_movn/compute/selection.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/materialize_i64_movn/compute/source.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/materialize_i64_movn/compute/tests.rs",
        ],
    },
];
