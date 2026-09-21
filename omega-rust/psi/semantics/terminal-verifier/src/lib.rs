#![forbid(unsafe_code)]

//! Structural validation and proof checking for terminal-Psi modules.
//!
//! The verifier reconstructs semantic axioms from executable operations and
//! edges, then requires evidence for every bodyful contract clause. Proof
//! bundles cannot choose which obligations exist.
//!
//! Start at `validation.rs`: `validate_module` checks a module's structure
//! in a fixed pass order and yields a `ValidatedTerminalModule`. Then
//! `verification.rs`: `verify_module` reconstructs every obligation of a
//! validated module and discharges each against the proof bundle, yielding
//! a `VerifiedTerminalModule` (or the interpretable, optimizable and
//! fixed-fuel variants). `trusted_surface` inventories what verification
//! trusts. The remaining root modules answer questions over an already
//! validated module: `control_graph` and `control_cycles` (control
//! topology and natural ranks), `proof_recursion` (recursive-component
//! questions), `optimization` (checks of target-neutral rewrites),
//! `quotient_correspondence` (the quotient bridge replay) and
//! `terminal_trace_v1` (the bounded observation profile).

mod control_cycles;
mod control_graph;
pub mod trusted_surface;
pub use control_cycles::{
    AcceptedControlCycle, ReconstructedControlCycleObligation, control_cycle_components,
    control_cycle_identity, control_cycle_members, cyclic_component_identity,
    dominating_control_cycle_entries, reconstruct_control_cycle_obligations,
};
mod optimization;
mod proof_recursion;
mod quotient_correspondence;
mod terminal_trace_v1;
mod validation;
mod verification;

pub use optimization::*;
pub use proof_recursion::*;
pub use quotient_correspondence::*;
pub use terminal_trace_v1::*;
pub use validation::*;
pub use verification::*;
