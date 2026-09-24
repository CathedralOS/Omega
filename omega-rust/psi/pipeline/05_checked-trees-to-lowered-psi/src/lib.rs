#![forbid(unsafe_code)]

//! Checked trees to unsealed, target-neutral Psi.
//!
//! Output retains source custody, proof and debug companions for later stages.
//! Unsupported constructs fail closed; this stage does not optimize or publish.
//!
//! One entrance: [`lower_machine`], in `machine_lowering.rs`. Read
//! `lower_terminal_selection` there to read this stage — the module order
//! below is the order it runs them in. A callback body is not a second
//! entrance: it lowers through the same route rooted at the callback entry,
//! and [`callback_lowering_receipt`] joins the checked callback coordinate
//! onto the module's entry machine afterward.
//!
//! 1. `machine_lowering` selects the checked machine named by a
//!    [`TerminalMachineSelection`], gates its guarded exits, then dispatches
//!    through `machine_lowering::machine_dispatch` to the plan family that
//!    owns the machine's shape.
//! 2. The plan families are layered, and each one reaches only downward:
//!    [`unit`] lowers attached, dynamic composed, structural-control and
//!    cleanup Unit machines; [`returns`] lowers affine, boundary-scalar,
//!    payloadless and structural return machines; [`scalar_graph`] prepares
//!    scalar graphs, closes their calls and assembles the module;
//!    [`expression_preparation`] turns source-bound expressions and bindings
//!    into what a body needs; [`emission`] writes the operations and stores
//!    every body shares.
//! 3. [`retention`] installs the checked custody the assembled module must
//!    keep: closed reaches, placed view inputs, restored reborrow call uses,
//!    suspension call plans, foreign borrow custody and operation crash
//!    contracts.
//! 4. [`proofs`] publishes conformances' evidence, proof recursion and
//!    quotient correspondence, then finalizes or validates operand proofs and
//!    produces the crash-obligation roster keyed to the finished module.
//!
//! The entrance closes by validating the module through `terminal_verifier`
//! and, when the checked plan is eligible, attaching the Terminal debug
//! companion that `machine_lowering::debug_map` builds.
//!
//! Beside the route: `producer_result` names what a plan family hands back —
//! its source mapping and its operand, debug and conformance completion modes
//! — and `terminal_identities` and `lowering_error` carry the identity and
//! failure vocabulary every producer shares.

// The entrance.
mod machine_lowering;

// The plan families `machine_dispatch` selects between, in their dependency
// order: each reaches down this list and never back up it.
mod emission;
mod expression_preparation;
mod returns;
mod scalar_graph;
mod unit;

// What the entrance runs over the assembled module, in that order.
mod proofs;
mod retention;

// Beside the route: the vocabulary every producer shares.
mod lowering_error;
mod producer_result;
mod terminal_identities;

// The entrance, and the failure vocabulary its callers match on.
pub use lowering_error::LoweringError;
pub use machine_lowering::machine_dispatch::{TerminalMachineSelection, select_terminal_machine};
pub use machine_lowering::{callback_lowering_receipt, lower_machine};

// Proof artifacts this stage produces for owners outside it.
pub use proofs::content_conservation::{
    LoweredContentConservation, LoweredContentIdentityReshuffles,
    LoweredContentPartitionComposition, LoweredContentPartitionCompositions,
};
pub use proofs::float_meaning_projection::FloatMeaningProjectionLoweringError;
// Crash entry-requirement certificates are produced here but attached to the
// artifact by the Terminal assembly stage, which runs in a downstream crate.
pub use proofs::entry_requirement_certificates::{
    CrashRosterError, EntryRequirementCertificate, check_entry_requirement_certificate,
    produce_crash_obligation_evidence, produce_entry_requirement_certificates,
};
// The native-differential optimizer corpus proves trap obligations through
// the checked canonical certificate producer from outside this crate.
pub use proofs::nonzero_divisor_certificate::produce_checked_canonical_integer_proof;

// The front-end pipeline the tests run, shared with the `suite` integration
// target that includes the same file; see its module documentation.
#[cfg(test)]
#[path = "../tests/support/front_end.rs"]
mod front_end;
#[cfg(test)]
mod tests;
