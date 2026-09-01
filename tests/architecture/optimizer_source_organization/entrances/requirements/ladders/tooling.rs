use super::super::SemanticLadder;

pub(crate) const LADDERS: &[SemanticLadder] = &[
    SemanticLadder {
        family: "offline optimization-policy artifact commands",
        paths: &[
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/main.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/arguments.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/capture.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/inputs.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/training.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/evaluation.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/regression_manifest.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/publication.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/error.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/tests/mod.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/tests/arguments.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/tests/capture.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/tests/fixture.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/tests/reference.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/tests/regression_manifest.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/tests/publication.rs",
        ],
    },
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
    SemanticLadder {
        family: "offline CostThresholdV1 checked regression baseline",
        paths: &[
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/regression_manifest/mod.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/regression_manifest/model.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/regression_manifest/identity.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/regression_manifest/codec.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/regression_manifest/validate.rs",
            "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/tests/regression_manifest.rs",
        ],
    },
];
