//! Execute a closed, checked Terminal Psi entry for the compiler.
//!
//! [`execute_policy`] uses the checker's proof to decide whether a closed
//! entry and everything it calls can run here, and refuses the whole entry
//! otherwise. It then runs the admitted bodies on the sponsor's actual
//! arguments, charging schedule-1 work and counting live cells under the
//! root's [`Ceilings`]. The result is a value of the unit's own types. It is
//! not validated against any consumer's demand and carries no authority.
//! [Contract](../CONTRACT.md)
#![forbid(unsafe_code)]

mod admission;
#[cfg(test)]
mod captured_policies_tests;
#[cfg(test)]
mod closed_constructor_tests;
mod execution;
mod meter;
mod places;
mod policy;
mod predicates;
mod values;

#[cfg(test)]
use admission::Reason;
use admission::{CheckedBody, admit};
pub use execution::{Completed, Location, Stop};
pub use meter::{CELL_SCHEDULE, Ceilings, ChargeSite, MeterStop, SiteUsage, Usage, WORK_SCHEDULE};
pub use policy::{PolicyReceipt, execute_policy};
pub use values::{Argument, Value};
