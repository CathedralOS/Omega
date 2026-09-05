//! Compiler-owned translation into inert package-review evidence.
//!
//! The entrance coordinates checked semantic interpretation, public API and
//! behavior projection, provider selection, representation disclosure, and
//! source custody. Evidence types and canonical encoding remain separate owners.

mod api;
mod authority;
mod behavior;
mod callables;
mod calling;
mod contracts;
mod package;
mod providers;
mod quotients;
mod representation;
mod semantics;
mod source;
mod terminal_authority_permissions;

pub use calling::project_checked_calling_policy;
pub use package::project_checked_package_review;
pub use providers::project_checked_selected_provider_policy;
pub use quotients::project_non_executable_quotient_package_review;
pub use representation::project_checked_representation_policy;
pub use semantics::conformances::project_checked_conformance_policy;
pub(crate) use semantics::declarations::nominal_identity;
pub use terminal_authority_permissions::project_checked_terminal_permission_policy;
