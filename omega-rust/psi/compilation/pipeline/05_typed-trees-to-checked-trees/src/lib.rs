/*
    We check an immutable typed program (Psi 05). `lower_typed_trees` consumes
    `FrozenTypedTrees` and the retained preparation judgments, validates source
    semantics, and publishes `CheckFacts` beside the unchanged typed program:
    flow, value, borrow, ownership, contract, range, multiplicity and
    termination judgments, plus the executable plans a Terminal producer later
    consumes. A program either leaves here with every obligation settled or
    with diagnostics; we never publish an intermediate plan set.
*/

/*
    Checking only reads the typed tables. Source elaboration and specialization
    belong to the preceding preparation stage, and independent validation and
    proof packages own the shared semantic queries, so a judgment here cannot
    quietly rewrite what preparation decided. Invocation-owned declaration
    authority stays fresh even when an exact-input language judgment is reused.
    The source is flattened into one file per domain; deeper module paths
    (`checks::borrows::resources`, `execution::terminal_unit::calls`, ...) are
    kept as inline modules, or as `#[path]` modules in a sibling
    `<parent>_<child>.rs` file when a subtree is large, so every path, privacy
    boundary and `super::` reference is the same as before.
*/

/*
    We sit between `typed-trees-to-typed-trees` (Psi 04, which specializes
    bodies and settles call identities) and `checked-trees-to-lowered-psi`
    (Psi 06, which lowers the checked facts and plans). The psi compiler,
    build-time evaluation, and Omega's package review and product coordinator
    also call the `products` seams to rederive facts and prune a checked tree
    to a selected product. Start at `checking`; the steps then run as `published`
    facts are built, `flow` and `values` are judged, `borrow` and `checks` are
    settled, and `execution` assembles the plans, reading the `shared` helpers.
*/

pub(crate) use optimization::digest_encoder;

/* The entry, and the guard that pins preparation's tables as read-only here. */
mod checking;
#[cfg(test)]
mod immutable_checking_input;

/*
    The steps. `lower_typed_trees` runs them as: facts are built, flow and
    values are judged, borrows and checks are settled, execution plans are
    assembled.
*/
mod borrow;
mod checks;
mod execution;
mod flow;
mod values;

/*
    What the stage publishes, the helpers every step reads, and the seams the
    product coordinator reaches once a checked tree exists.
*/
mod products;
mod published;
mod shared;

pub use ::facts::declaration_page_fork_totals;
pub use checking::{
    CheckingOutcome, CheckingRequest, RetainedCheck, lower_typed_trees, lower_typed_trees_reusing,
    reuse_retained_check,
};
pub use products::package_review::{
    derive_authored_machine_crash_buckets, derive_authored_signature_crash_buckets,
    derive_checked_contract_expression_evidence_instantiation,
    derive_checked_contract_expression_evidence_parameters, derive_checked_nominal_call_target,
    derive_checked_semantic_dependencies, infer_checked_crash_causes,
    infer_checked_machine_crash_causes, infer_checked_recursive_machine_terminations,
    infer_machine_termination_summary, late_bound_member_declaration_from_exact_owner,
    typed_operator_authored_selection_candidates, typed_operator_has_no_authored_selection,
};

pub use products::product_pruning::{
    CheckedTreeProductPruning, CheckedTreeProductPruningOutcome, CheckedTreeProductRoots,
    CheckedTreeProductRootsError, CheckedTreeProductSelection, CheckedTreeProductSelectionIdentity,
    prune_checked_tree_product, select_checked_tree_product,
};

/*
    Build inputs to Psi's instruction-authority check. The checking entrance
    consumes these; the compiler does not run an additional validation pass.
*/
pub use ::validation::{AsmAuthorityAdmission, data_requires_establishment};

#[doc(hidden)]
pub fn __mem_probe(label: &str) {
    if std::env::var_os("OMEGA_MEM_PROBE").is_none() {
        return;
    }
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| {
        status
            .lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|kb| kb.parse::<u64>().ok())
            .unwrap_or(0)
            / 1024
    };
    eprintln!("MEMPROBE {label}: rss={}MB hwm={}MB", field("VmRSS:"), field("VmHWM:"));
}

#[doc(hidden)]
pub fn __mem_rss_mb() -> u64 {
    if std::env::var_os("OMEGA_MEM_PROBE").is_none() {
        return 0;
    }
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| s.split_whitespace().nth(1).and_then(|p| p.parse::<u64>().ok()))
        .map(|pages| pages * 4096 / (1024 * 1024))
        .unwrap_or(0)
}
