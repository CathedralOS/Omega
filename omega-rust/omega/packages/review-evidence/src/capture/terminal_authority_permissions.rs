// Explicit service terminal-authority permissions rejoined to resolved semantic
// bindings without inferring permissions from filenames, provider names, or
// semantic roles.

pub(crate) mod declarations;
mod policy;

pub(crate) use policy::project_source_terminal_permission_policy;
