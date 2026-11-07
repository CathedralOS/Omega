// Compiler-owned projection of inert calling-contract components (application
// calling plans, callback layouts/binders, opaque uses, and physical contracts).

pub(crate) mod application;
pub(crate) mod callbacks;
mod opaque;
mod physical;

pub(crate) use application::project_checked_calling_policy;

use crate::capture::refusal;

refusal!(reason, "calling policy rejects {reason}");
