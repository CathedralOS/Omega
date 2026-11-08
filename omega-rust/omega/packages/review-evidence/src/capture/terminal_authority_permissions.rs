// Explicit service terminal-authority permissions rejoined to resolved semantic
// bindings without inferring permissions from filenames, provider names, or
// semantic roles.

pub(crate) mod declarations;
mod policy;

pub use policy::project_checked_terminal_permission_policy;
