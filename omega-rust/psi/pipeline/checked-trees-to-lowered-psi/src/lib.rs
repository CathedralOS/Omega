#![forbid(unsafe_code)]

//! Checked trees to unsealed, target-neutral Psi.
//!
//! Output retains source custody, proof and debug companions for later stages.
//! Unsupported constructs fail closed; this stage does not optimize or publish.
//!
//! Start at `machine_lowering.rs`: it selects the checked machine named by a
//! [`TerminalMachineSelection`], dispatches it through
//! `machine_lowering::machine_dispatch` to the plan family that owns its shape,
//! and sequences the custody, evidence, validation and debug work every
//! selected module needs. The plan families beneath it are:
//!
//! - [`unit`]: attached, dynamic composed, structural-control and cleanup Unit machines.
//! - [`returns`]: affine, boundary-scalar, payloadless and structural return machines.
//! - [`scalar_graph`]: scalar-graph preparation, call closure and module assembly.
//! - [`expression_preparation`]: source-bound expressions, bindings and independent replay.
//! - [`emission`]: operation and store emission shared by Unit and scalar bodies.
//! - [`retention`]: checked custody installed on the assembled module.
//! - [`proofs`]: propositions, contracts, certificates and evidence artifacts.
//!
//! Beside the route, `producer_result` defines source ownership and completion modes.
//! `lowering_error` and `terminal_identities` carry the
//! failure and identity vocabulary every producer shares and `debug_map`
//! presents the Terminal debug companion.

mod emission;
mod expression_preparation;
mod lowering_error;
mod machine_lowering;
mod producer_result;
mod proofs;
mod retention;
mod returns;
mod scalar_graph;
mod terminal_identities;
mod unit;

pub use lowering_error::LoweringError;
pub use machine_lowering::machine_dispatch::{TerminalMachineSelection, select_terminal_machine};
pub use machine_lowering::{callback_lowering_receipt, lower_machine};
pub use proofs::content_conservation::{
    LoweredContentConservation, LoweredContentIdentityReshuffles,
    LoweredContentPartitionComposition, LoweredContentPartitionCompositions,
};
pub use proofs::float_meaning_projection::FloatMeaningProjectionLoweringError;
// The native-differential optimizer corpus proves trap obligations through
// the checked canonical certificate producer from outside this crate.
pub use proofs::nonzero_divisor_certificate::produce_checked_canonical_integer_proof;
// Crash entry-requirement certificates are produced here but attached to the
// artifact by the Terminal assembly stage, which runs in a downstream crate.
pub use proofs::entry_requirement_certificates::{
    CrashRosterError, EntryRequirementCertificate, check_entry_requirement_certificate,
    produce_crash_obligation_evidence, produce_entry_requirement_certificates,
};

// The front-end pipeline the tests run, shared with the `suite` integration
// target that includes the same file; see its module documentation.
#[cfg(test)]
#[path = "../tests/support/front_end.rs"]
mod front_end;
#[cfg(test)]
mod tests;
