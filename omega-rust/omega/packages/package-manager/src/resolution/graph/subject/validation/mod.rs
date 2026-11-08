/*
    We organize canonical source-closure validation by the invariant being
    checked: `closure` validates the complete graph, `dependency` checks
    dependency requests and selections, `root` validates root requests, `source`
    validates package keys, lineages, and navigations, and `walk` checks
    reachability and acyclicity.
*/

/*
    Canonical subject validation belongs to the source-closure subject boundary
    (`resolution/graph/subject/validation/mod.rs`).
*/

mod closure;
mod dependency;
mod root;
mod source;
mod walk;

pub(super) use closure::validate_subject;
pub(super) use dependency::{validate_dependency_request, validate_dependency_selection_kind};
pub(super) use root::canonical_root_request;
pub(super) use source::{validate_package_key, validate_source_lineage};
pub(super) use source::{validate_package_navigation, validate_source_identity};
