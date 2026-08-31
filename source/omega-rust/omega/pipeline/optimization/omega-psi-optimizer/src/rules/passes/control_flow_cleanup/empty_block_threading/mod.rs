//! Optimizer module role: stage group. Linear and path-qualified empty-block threading.

mod binding_composition;
mod linear;
mod linear_accounting;
mod ownership_identity;
mod path_qualified;
mod path_qualified_accounting;

pub use linear::LinearEmptyBlockThreadRule;
pub use path_qualified::PathQualifiedEmptyBlockThreadRule;

use binding_composition::compose_linear_thread_bindings;
use linear_accounting::linear_thread_accounting;
use ownership_identity::linear_thread_ownership_is_identity;
use path_qualified_accounting::path_thread_accounting;
