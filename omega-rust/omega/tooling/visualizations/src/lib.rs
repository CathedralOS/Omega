//! JSON manifests of what the compiler concluded, written beside a build - and
//! the one design choice worth knowing before you read a line of it: these
//! writers have no error type. They panic.
//!
//! Every public entry point returns `String`, not `Result<String, _>`. There is
//! no error enum in the crate. `checked_trees.rs` alone carries 77 `.expect(..)`
//! calls plus bare `panic!`s in its production path, and 154 `#[should_panic]`
//! fixtures across nine test files pin their exact messages. The tests are
//! 6,359 of the crate's 12,904 lines - the fixtures are not incidental
//! coverage, they are the specification.
//!
//! Read the messages together and they all have one shape:
//!
//! ```text
//!   "content partition composition must name one exact checked flow state"
//!   "must resolve to exactly one retained typed semantic declaration"
//!   "must match one distinct exact parent row"
//!   "semantic-domain commitments must be strictly increasing"
//!   "content identity reshuffle paths must not retain a runtime index"
//! ```
//!
//! Every one is a cardinality or exactness claim about a coordinate resolving
//! into the checked trees. Eight of these writers run from
//! `compiler/src/pipeline/artifacts.rs` on every package-aware build.

//! Refusing to emit is the decision, and a renderer is exactly the kind of tool
//! that normally does the opposite. The obvious design is a `Result`, or an
//! `"unknown"` field, or an omitted row - degrade gracefully, because a
//! visualization failing is not a compilation failing.
//!
//! That is rejected because these manifests are read back as evidence about the
//! compiler's own state. A row reading `"state": "unknown"` is indistinguishable
//! from a row that resolved correctly to nothing, and both get quoted later as
//! fact. A manifest that cannot be produced honestly is worth less than no
//! manifest, so an unresolvable coordinate stops the build at the moment of
//! generation instead of shipping something plausible.
//!
//! The cost is real and worth stating plainly: a defect anywhere upstream in the
//! checked trees surfaces as a panic inside a TOOLING crate during artifact
//! writing, which is a confusing place to land. The 154 fixtures are what makes
//! that trade payable - each one names the invariant whose violation produced
//! the panic, so the message identifies the upstream defect rather than merely
//! reporting that rendering failed.

//! `compiler/src/pipeline/artifacts.rs` calls into this crate eight times
//! per build. `effects` owns every carrier rendered here.
//!
//! @Cleanup: `src/typed_trees/behavior.rs` is 313 lines that rustc never
//! compiles. `lib.rs` declares `mod checked_trees` and
//! `mod executable_tcb_manifest` and nothing else - there is no `mod
//! typed_trees` anywhere in the crate. The file is not dead code, which at
//! least type-checks; it is absent code that looks present, so grep finds it,
//! review finds it, and the build has never had an opinion about whether it
//! compiles. Delete it or declare it, but do not leave it findable.
//!
//! @Note: `executable_tcb_manifest_set_json` and the
//! `ExecutableTcbManifestSet` it takes are dead on both sides of the crate
//! boundary. The writer's only callers are its own test and the re-export in
//! `lib.rs`; the type is constructed only below the `#[cfg(test)]` line of
//! `effects/src/isolated_executable_scopes.rs`. This is the mirror of the
//! seam recorded in `effects`, where a LIVE writer in this crate consumes
//! evidence nothing in production produces. One boundary, two opposite defects,
//! and neither crate's own tests can see either.

mod checked_trees;
mod executable_tcb_manifest;

pub use checked_trees::{
    capability_manifest_json, capability_manifest_json_with_composition,
    capability_manifest_json_with_selection, carry_manifest_json, claim_outcome_manifest_json,
    index_compatibility_manifest_json, machine_contract_manifest_json,
    qualification_evidence_manifest_json, task_activation_manifest_json,
};
pub use executable_tcb_manifest::{
    executable_tcb_manifest_json, executable_tcb_manifest_set_json,
    executable_tcb_manifest_value_json,
};
