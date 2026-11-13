/*
Compiler-issued package review evidence, policy projection, and runtime consent
verification.

We separate the package review surface into three distinct owners so compiler
representation never leaks into persisted lock or admission state:
1. `record` defines the inert, target-neutral review vocabulary (`PackagePolicyBaseline`,
   `PackageReviewEvidence`, and canonical `PackagePolicyRow` items).
2. `capture` translates checked compiler outputs (`CheckedCompilation`,
   `CheckedTrees`, and `CheckedProductSource`) into inert review records and
   semantic-binding nominations (`project_package_review`,
   `project_checked_package_policy`, and `PreparedRuntimePolicy`), comparing any
   supplied candidate program against immutable compiler custody.
3. `encoding` serializes and recovers canonical policy bytes and policy rows
   with bounded accounting.
*/

/*
Evidence projected here is an auditable description of what a compiled package
declares and reaches, never an admission grant. `permission_acceptance` compares
the root product's projected `TerminalPermission` and `AssumedBindingContract`
rows against retained lock consent or the consuming root's own public `Build`
evaluation before execution.
*/

#![forbid(unsafe_code)]

mod capture;
pub mod encoding;
pub mod permission_acceptance;
pub mod record;

pub use capture::PublishedPackageReviewInput as PackageReviewInput;
pub use capture::{PackageReviewProjectionError, project_package_review};
pub use capture::{
    SemanticBindingProjectionError, candidate_hosted_exit_bindings, candidate_service_bindings,
    candidate_target_entry_binding,
};
pub use capture::{project_checked_package_policy, project_checked_terminal_permission_policy};
