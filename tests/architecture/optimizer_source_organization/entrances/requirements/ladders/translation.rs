use super::super::SemanticLadder;

pub(crate) const LADDERS: &[SemanticLadder] = &[
    SemanticLadder {
        family: "pre-physical optimization manifest custody tests",
        paths: &[
            "tests/native-differential/tests/abstract_publication/manifests/mod.rs",
            "tests/native-differential/tests/abstract_publication/manifests/fixture.rs",
            "tests/native-differential/tests/abstract_publication/manifests/positive.rs",
            "tests/native-differential/tests/abstract_publication/manifests/fields.rs",
            "tests/native-differential/tests/abstract_publication/manifests/wire.rs",
            "tests/native-differential/tests/abstract_publication/manifests/wire_offsets.rs",
            "tests/native-differential/tests/abstract_publication/manifests/multipass.rs",
        ],
    },
    SemanticLadder {
        family: "Terminal operation lowering",
        paths: &[
            "omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/lowering/machine/operation/mod.rs",
            "omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/lowering/machine/operation/routing.rs",
            "omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/lowering/machine/operation/structural_establishment.rs",
            "omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/lowering/machine/operation/calls.rs",
            "omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/lowering/machine/operation/effects.rs",
            "omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/lowering/machine/operation/boolean.rs",
            "omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/lowering/machine/operation/integer_constants_and_relations.rs",
            "omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/lowering/machine/operation/integer_conversion.rs",
            "omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/lowering/machine/operation/integer_bitwise.rs",
            "omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/lowering/machine/operation/shifts.rs",
            "omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/lowering/machine/operation/arithmetic.rs",
        ],
    },
    SemanticLadder {
        family: "projected structural qualification target admission",
        paths: &[
            "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/lowering/coordination.rs",
            "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/lowering/coordination/projected_qualifications/mod.rs",
            "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/model.rs",
        ],
    },
];
