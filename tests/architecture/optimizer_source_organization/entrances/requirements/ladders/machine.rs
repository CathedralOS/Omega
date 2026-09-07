use super::super::SemanticLadder;

pub(crate) const LADDERS: &[SemanticLadder] = &[
    SemanticLadder {
        family: "non-authoritative target cost model",
        paths: &[
            "omega-rust/omega/representations/physical-instructions/src/physical_instructions/costs/mod.rs",
            "omega-rust/omega/representations/physical-instructions/src/physical_instructions/costs/model.rs",
            "omega-rust/omega/representations/physical-instructions/src/physical_instructions/costs/identity.rs",
            "omega-rust/omega/representations/physical-instructions/src/physical_instructions/costs/tests.rs",
        ],
    },
    SemanticLadder {
        family: "pre-allocation machine-effect codec",
        paths: &[
            "omega-rust/omega/representations/selected-instructions/src/selected_instructions/effects/program/encoding/mod.rs",
            "omega-rust/omega/representations/selected-instructions/src/selected_instructions/effects/program/encoding/cursor.rs",
            "omega-rust/omega/representations/selected-instructions/src/selected_instructions/effects/program/encoding/error.rs",
            "omega-rust/omega/representations/selected-instructions/src/selected_instructions/effects/program/encoding/v6/mod.rs",
            "omega-rust/omega/representations/selected-instructions/src/selected_instructions/effects/program/encoding/v6/framing.rs",
            "omega-rust/omega/representations/selected-instructions/src/selected_instructions/effects/program/encoding/v6/instruction.rs",
            "omega-rust/omega/representations/selected-instructions/src/selected_instructions/effects/program/encoding/v6/ownership.rs",
            "omega-rust/omega/representations/selected-instructions/src/selected_instructions/effects/program/encoding/v6/values.rs",
        ],
    },
];
