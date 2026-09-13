use super::super::SemanticLadder;

pub(crate) const LADDERS: &[SemanticLadder] = &[
    SemanticLadder {
        family: "offline optimization-policy artifact commands",
        paths: &[
            "omega-rust/omega/tooling/optimization-policy-offline/src/bin/optimization-policy-offline/main.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/bin/optimization-policy-offline/arguments.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/bin/optimization-policy-offline/capture.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/bin/optimization-policy-offline/inputs.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/bin/optimization-policy-offline/training.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/bin/optimization-policy-offline/evaluation.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/bin/optimization-policy-offline/regression_manifest.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/bin/optimization-policy-offline/publication.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/bin/optimization-policy-offline/error.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/bin/optimization-policy-offline/tests/mod.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/bin/optimization-policy-offline/tests/arguments.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/bin/optimization-policy-offline/tests/capture.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/bin/optimization-policy-offline/tests/fixture.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/bin/optimization-policy-offline/tests/reference.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/bin/optimization-policy-offline/tests/regression_manifest.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/bin/optimization-policy-offline/tests/publication.rs",
        ],
    },
    SemanticLadder {
        family: "offline optimization-policy corpus custody",
        paths: &[
            "omega-rust/omega/tooling/optimization-policy-offline/src/corpus/mod.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/corpus/model.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/corpus/capture.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/corpus/validate.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/corpus/identity.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/corpus/split.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/corpus/codec/mod.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/corpus/codec/cursor.rs",
        ],
    },
    SemanticLadder {
        family: "offline CostThresholdV1 reference training",
        paths: &[
            "omega-rust/omega/tooling/optimization-policy-offline/src/cost_threshold_policy.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/cost_threshold_policy/model.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/cost_threshold_policy/identity.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/cost_threshold_policy/inference.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/cost_threshold_policy/training.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/cost_threshold_policy/training/replay.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/cost_threshold_policy/codec/mod.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/cost_threshold_policy/codec/cursor.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/cost_threshold_policy/codec/model.rs",
        ],
    },
    SemanticLadder {
        family: "offline CostThresholdV1 split evaluation",
        paths: &[
            "omega-rust/omega/tooling/optimization-policy-offline/src/cost_threshold_policy/evaluation.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/cost_threshold_policy/evaluation/replay.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/cost_threshold_policy/codec/report.rs",
        ],
    },
    SemanticLadder {
        family: "offline CostThresholdV1 checked regression baseline",
        paths: &[
            "omega-rust/omega/tooling/optimization-policy-offline/src/cost_threshold_policy/regression_manifest.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/cost_threshold_policy/regression_manifest/model.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/cost_threshold_policy/regression_manifest/identity.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/cost_threshold_policy/regression_manifest/codec.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/cost_threshold_policy/regression_manifest/validate.rs",
            "omega-rust/omega/tooling/optimization-policy-offline/src/cost_threshold_policy/tests/regression_manifest.rs",
        ],
    },
];
