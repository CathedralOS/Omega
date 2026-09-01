use super::super::SemanticLadder;

pub(crate) const LADDERS: &[SemanticLadder] = &[
    SemanticLadder {
        family: "offline optimization-policy corpus custody",
        paths: &[
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/corpus/mod.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/corpus/model.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/corpus/capture.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/corpus/validate.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/corpus/identity.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/corpus/split.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/corpus/codec/mod.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/corpus/codec/cursor.rs",
        ],
    },
    SemanticLadder {
        family: "offline CostThresholdV1 reference training",
        paths: &[
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/mod.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/model.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/identity.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/inference.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/training/mod.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/training/compute.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/training/replay.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/codec/mod.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/codec/cursor.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/codec/model.rs",
        ],
    },
    SemanticLadder {
        family: "offline CostThresholdV1 split evaluation",
        paths: &[
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/evaluation/mod.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/evaluation/compute.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/evaluation/replay.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/codec/report.rs",
        ],
    },
];
