use super::super::SemanticLadder;

pub(crate) const LADDERS: &[SemanticLadder] = &[
    SemanticLadder {
        family: "offline optimization-policy artifact commands",
        paths: &[
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/main.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/arguments.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/capture.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/inputs.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/training.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/evaluation.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/regression_manifest.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/publication.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/error.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/tests/mod.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/tests/arguments.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/tests/capture.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/tests/fixture.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/tests/reference.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/tests/regression_manifest.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/tests/publication.rs",
        ],
    },
    SemanticLadder {
        family: "offline optimization-policy corpus custody",
        paths: &[
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/corpus/mod.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/corpus/model.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/corpus/capture.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/corpus/validate.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/corpus/identity.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/corpus/split.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/corpus/codec/mod.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/corpus/codec/cursor.rs",
        ],
    },
    SemanticLadder {
        family: "offline CostThresholdV1 reference training",
        paths: &[
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/mod.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/model.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/identity.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/inference.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/training/mod.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/training/compute.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/training/replay.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/codec/mod.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/codec/cursor.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/codec/model.rs",
        ],
    },
    SemanticLadder {
        family: "offline CostThresholdV1 split evaluation",
        paths: &[
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/evaluation/mod.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/evaluation/compute.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/evaluation/replay.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/codec/report.rs",
        ],
    },
    SemanticLadder {
        family: "offline CostThresholdV1 checked regression baseline",
        paths: &[
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/regression_manifest/mod.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/regression_manifest/model.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/regression_manifest/identity.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/regression_manifest/codec.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/regression_manifest/validate.rs",
            "omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/tests/regression_manifest.rs",
        ],
    },
];
