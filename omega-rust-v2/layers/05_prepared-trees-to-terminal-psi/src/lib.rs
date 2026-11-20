//! Lower a prepared package into owned, target-neutral Psi declarations and machine bodies.
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
use lowering::types::LoweringTypes;
pub use lowering::types::TypeError;
pub use omega_psi::*;
