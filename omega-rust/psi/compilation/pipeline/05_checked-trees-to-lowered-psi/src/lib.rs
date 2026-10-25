#![forbid(unsafe_code)]

//! Checked trees to unsealed, target-neutral Psi.
//!
//! Output retains source custody, proof and debug companions for later stages.
//! Unsupported constructs fail closed; this stage does not optimize or publish.
//! Expression place/result types and storage carriers come from the frozen
//! checked facts; lowering does not resolve them again from source context.
//! The identity table is shared across bodies and retained declarations, then
//! sealed at the production handoff. It does not merge the machine,
//! state, value, block, edge or place namespaces, or runtime and erased formals.
//!
//! One entrance: [`lower_machine`], in `machine_lowering.rs`. Read
//! `lower_terminal_selection` there to read this stage — the module order
//! below is the order it runs them in. A callback body is not a second
//! entrance: it lowers through the same route rooted at the callback entry,
//! and retains its checked declaration directly in the produced machine.
//!
//! 1. `machine_lowering` validates the checked permission ledger before any
//!    consumer uses its evidence, then selects the checked machine named by a
//!    [`TerminalMachineSelection`], gates its guarded exits, then dispatches
//!    through `machine_lowering::machine_dispatch` to the plan family that
//!    owns the machine's shape.
//! 2. The plan families are layered, and each one reaches only downward:
//!    [`unit`] lowers attached, structural-control and cleanup Unit
//!    machines, including every structural or scalar result a body returns;
//!    [`scalar_lowering`] lowers the scalar computations, contracts, callees
//!    and branch states those bodies embed;
//!    [`expression_preparation`] turns source-bound expressions and bindings
//!    into what a body needs; [`emission`] writes the operations and stores
//!    every body shares.
//! 3. [`retention`] installs the checked custody the assembled module must
//!    keep: closed reaches, placed view inputs, restored reborrow call uses,
//!    suspension call plans, foreign borrow custody and operation crash
//!    contracts.
//! 4. [`proofs`] lowers authored propositions, contracts, evidence terms,
//!    crash routes, content conservation and float-meaning projections.
//!
//! The entrance closes by attaching the Terminal debug companion that
//! `machine_lowering::debug_map` builds when the checked plan is eligible.
//!
//! Beside the route: `producer_result` names what a plan family hands back —
//! its source mapping and its debug and conformance completion modes
//! — `terminal_identities` and `lowering_error` carry the identity and
//! failure vocabulary every producer shares, and `declarations` maps a checked
//! state symbol to its declared machine and state. `direct_calls` is the one place
//! a direct in-module call's facts choose its Terminal call operation, and
//! the one reader of those operations' callee and operand lanes.
//! `structural_types` turns checked structural type plans into the Terminal
//! declarations every family that declares structural storage shares.

// The entrance.
pub(crate) use optimization::digest_encoder;
mod machine_lowering;
mod program_roots;
pub use program_roots::TerminalProgramRoots;

// The plan families `machine_dispatch` selects between, in their dependency
// order: each reaches down this list and never back up it.
mod emission;
mod expression_preparation;
mod scalar_lowering;
mod unit;

// What the entrance runs over the assembled module, in that order.
mod proofs;
mod retention;

// Beside the route: shared producer vocabulary and diagnostic source attribution.
mod activation_storage;
mod call_operands;
mod call_sites;
mod content_classes;
mod declarations;
mod direct_calls;
mod lowered_machine_provenance;
mod lowering_error;
mod producer_result;
mod reference_custody;
mod structural_types;
mod terminal_identities;

#[cfg(test)]
mod source_guards;

// The entrance, and the failure vocabulary its callers match on.
pub use lowering_error::LoweringError;
pub use machine_lowering::lower_machine;
pub use machine_lowering::machine_dispatch::{TerminalMachineSelection, select_terminal_machine};

// Diagnostic-only source attribution for the unoptimized lowering.
pub use lowered_machine_provenance::lowered_machine_provenance;

// Proof artifacts this stage produces for owners outside it.
pub use proofs::content_conservation::{
    LoweredContentConservation, LoweredContentIdentityReshuffles,
};
pub use proofs::float_meaning_projection::FloatMeaningProjectionLoweringError;

// The front-end pipeline the tests run, shared with the `suite` integration
// target that includes the same file; see its module documentation.

// The integer prover runs scoped worker threads that allocate heavily; the
// macOS system allocator serializes them on its zone locks, and mimalloc's
// per-thread heaps do not. Test-only: the compiler binaries keep the system
// allocator.
