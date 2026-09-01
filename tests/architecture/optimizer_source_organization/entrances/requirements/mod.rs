//! Required optimizer entrances and semantic ladders, grouped by compiler domain.
//!
//! Executable inventories prove that each named stage still owns its real join.
//! Semantic ladders prove that the next responsibility-specific leaves remain
//! visible. Both descend through Psi, translation, selection/allocation,
//! machine optimization, and physical/native realization.

mod executable;
mod ladders;
mod model;

pub(super) use model::{
    ExecutableEntranceDomain, RequiredCoordinationEntrance, SemanticLadder, SemanticLadderDomain,
};

pub(super) const EXECUTABLE_ENTRANCE_DOMAINS: &[ExecutableEntranceDomain] = &[
    ExecutableEntranceDomain {
        name: "build and compiler optimization hooks",
        entrances: executable::compiler::ENTRANCES,
    },
    ExecutableEntranceDomain {
        name: "Psi contracts and optimization",
        entrances: executable::psi::ENTRANCES,
    },
    ExecutableEntranceDomain {
        name: "representation translation",
        entrances: executable::translation::ENTRANCES,
    },
    ExecutableEntranceDomain {
        name: "selection and allocation",
        entrances: executable::selection_allocation::ENTRANCES,
    },
    ExecutableEntranceDomain {
        name: "machine optimization",
        entrances: executable::machine::ENTRANCES,
    },
    ExecutableEntranceDomain {
        name: "physical pipeline, artifacts, and native realization",
        entrances: executable::pipeline_native::ENTRANCES,
    },
    ExecutableEntranceDomain {
        name: "offline optimizer tooling",
        entrances: executable::tooling::ENTRANCES,
    },
];

pub(super) const SEMANTIC_LADDER_DOMAINS: &[SemanticLadderDomain] = &[
    SemanticLadderDomain {
        name: "build and compiler optimization hooks",
        ladders: ladders::compiler::LADDERS,
    },
    SemanticLadderDomain {
        name: "Psi contracts and optimization",
        ladders: ladders::psi::LADDERS,
    },
    SemanticLadderDomain {
        name: "representation translation",
        ladders: ladders::translation::LADDERS,
    },
    SemanticLadderDomain {
        name: "selection and allocation",
        ladders: ladders::selection_allocation::LADDERS,
    },
    SemanticLadderDomain {
        name: "machine optimization",
        ladders: ladders::machine::LADDERS,
    },
    SemanticLadderDomain {
        name: "physical pipeline, artifacts, and native realization",
        ladders: ladders::pipeline_native::LADDERS,
    },
    SemanticLadderDomain {
        name: "offline optimizer tooling",
        ladders: ladders::tooling::LADDERS,
    },
];

pub(crate) fn is_required_coordination_entrance(path: &str) -> bool {
    EXECUTABLE_ENTRANCE_DOMAINS
        .iter()
        .flat_map(|domain| domain.entrances)
        .any(|entrance| entrance.path == path)
}
