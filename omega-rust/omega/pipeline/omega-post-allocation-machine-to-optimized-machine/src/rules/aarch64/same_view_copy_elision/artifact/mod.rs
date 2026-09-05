//! Optimizer module role: stage group. Common disposition persistence.

mod codec;
mod identity;
mod model;

pub use identity::aarch64_same_view_copy_elision_identity;
pub(crate) use identity::revision_identity;
pub use model::*;
