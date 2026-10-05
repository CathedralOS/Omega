//! Explicit D45 service permissions rejoined to resolved semantic bindings.
//!
//! Supplied permissions are not inferred from filenames, provider names,
//! semantic roles, or broad risk labels.

mod declarations;
mod policy;
mod requirements;

pub use policy::project_checked_terminal_permission_policy;
