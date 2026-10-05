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
mod input;
mod package_policy;
mod providers;
mod representation;
mod semantics;
mod source;
mod terminal_authority_permissions;

pub use callables::project_checked_callable_policy;
pub use input::PackageReviewInput;
pub use package_policy::project_checked_package_policy;
pub use providers::project_checked_selected_provider_policy;
pub use representation::project_checked_representation_policy;
pub use semantics::conformances::project_checked_conformance_policy;
pub(crate) use semantics::declarations::nominal_identity;
pub use terminal_authority_permissions::project_checked_terminal_permission_policy;
