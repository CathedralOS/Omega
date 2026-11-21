//! Lower a prepared package into owned, target-neutral Psi declarations and machine bodies.
//! [`lower_package`] takes the frozen trees and returns a [`LoweredPackage`]. The array extent
//! programs in `tests/fixtures/array_extent/` run through the CLI. For the actual Psi parameter
//! types and return operands, read `tests/handoff.rs`.
//!
//! Start with lowering/package.rs for package assembly. The declarations modules lower
//! schemas and contracts, and the machines modules lower executable bodies. Downstream consumers read the owned records from the omega-psi crate.
#![forbid(unsafe_code)]

mod declarations;
mod lowering;
mod machines;

mod identity;
pub use lowering::error::LoweringError;
use lowering::error::site_at;
pub use lowering::package::lower_package;
pub use declarations::domains::lower_selected_policy_definition;
use lowering::types::LoweringTypes;
pub use lowering::types::TypeError;
pub use omega_psi::*;
