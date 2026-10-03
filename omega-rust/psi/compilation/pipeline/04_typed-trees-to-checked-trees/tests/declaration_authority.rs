//! Exercise the public checking entrypoint, including its production memo.
//! Package permission must be checked anew, not cached with language judgments.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceMap, SourceOrigin};
use source_files_to_tokens::Lexer;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::{parse, parse_syntax_trees_with_id};
use typed_trees::TypedTrees;
use typed_trees_to_checked_trees::{CheckingRequest, lower_typed_trees, retain_repeated_checks};
use validation::DeclarationSelectionAuthority;

struct Authority {
    permits_dependency: bool,
    dependency_queries: AtomicUsize,
}

impl Authority {
    fn new(permits_dependency: bool) -> Self {
        Self {
            permits_dependency,
            dependency_queries: AtomicUsize::new(0),
        }
    }
}

impl DeclarationSelectionAuthority for Authority {
    fn allows_declaration_selection(
        &self,
        requester: PackageKeyIdentity,
        owner: PackageKeyIdentity,
    ) -> bool {
        if requester == owner {
            return true;
        }
        self.dependency_queries.fetch_add(1, Ordering::Relaxed);
        self.permits_dependency
    }

    fn package_label(&self, package: PackageKeyIdentity) -> String {
        format!("package-{}", package.digest()[0])
    }
}

const CONSUMER: &str = "machine consumer(value: Visible) -> u64 { 0 }";

fn fixture(requester_has_identity: bool, requester_text: &str) -> TypedTrees {
    let mut sources = SourceMap::default();
    let owner_text = "pub data Visible {}";
    let owner = sources
        .add_with_metadata(
            PathBuf::from("owner/main.omg"),
            owner_text.to_owned(),
            PathBuf::from("owner"),
            Some(PackageKeyIdentity::from_digest([2; 32]).expect("owner identity")),
            SourceOrigin::User,
        )
        .source_id;
    let requester = sources
        .add_with_metadata(
            PathBuf::from("requester/main.omg"),
            requester_text.to_owned(),
            PathBuf::from("requester"),
            requester_has_identity
                .then(|| PackageKeyIdentity::from_digest([1; 32]).expect("requester identity")),
            SourceOrigin::User,
        )
        .source_id;
    let mut syntax = parse_syntax_trees_with_id(
        owner,
        &Lexer::new(owner_text).tokenize().expect("owner tokens"),
    )
    .expect("owner syntax");
    parse(
        &mut syntax,
        requester,
        &Lexer::new(requester_text)
            .tokenize()
            .expect("requester tokens"),
    )
    .expect("requester syntax");
    let resolved = resolve(ResolutionRequest {
        syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolved declarations");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed declarations")
}

#[test]
fn settled_check_rechecks_authority_on_a_memo_hit() {
    let _retention = retain_repeated_checks();
    let typed = fixture(true, CONSUMER);
    let permitted = Authority::new(true);
    let forbidden = Authority::new(false);
    let request = CheckingRequest::settled().with_declaration_authority(Some(&permitted));
    lower_typed_trees(typed.clone(), &request).expect("direct dependency admits");
    let queries = permitted.dependency_queries.load(Ordering::Relaxed);
    assert!(
        queries > 0,
        "the actual parameter type selects the foreign package"
    );
    let diagnostics = lower_typed_trees(
        typed.clone(),
        &CheckingRequest::settled().with_declaration_authority(Some(&forbidden)),
    )
    .expect_err("a cached language judgment cannot confer package authority");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("without declaring that package as a direct dependency")
    }));
    assert_eq!(
        forbidden.dependency_queries.load(Ordering::Relaxed),
        queries
    );
    lower_typed_trees(typed, &request).expect("another caller's refusal does not poison the memo");
    assert_eq!(
        permitted.dependency_queries.load(Ordering::Relaxed),
        queries * 2
    );
}

#[test]
fn preliminary_permission_does_not_authorize_the_settled_caller() {
    let _retention = retain_repeated_checks();
    let typed = fixture(true, CONSUMER);
    lower_typed_trees(
        typed.clone(),
        &CheckingRequest::preliminary().with_declaration_authority(Some(&Authority::new(true))),
    )
    .expect("pre-build source admitted under its own authority");
    let diagnostics = lower_typed_trees(
        typed,
        &CheckingRequest::settled().with_declaration_authority(Some(&Authority::new(false))),
    )
    .expect_err("settled authority is independently enforced");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("without declaring that package as a direct dependency")
    }));
}

#[test]
fn package_check_rejects_missing_requester_custody() {
    let diagnostics = lower_typed_trees(
        fixture(false, CONSUMER),
        &CheckingRequest::settled().with_declaration_authority(Some(&Authority::new(true))),
    )
    .expect_err("a permissive dependency policy does not invent source ownership");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("no reconciled requesting package identity")
    }));
}

#[test]
fn sibling_scope_survives_the_preliminary_pruning_checkpoint() {
    let typed = fixture(
        true,
        "demo_target machine consumer(value: Visible) -> u64 { 0 }",
    );
    let denied = Authority::new(false);
    lower_typed_trees(
        typed.clone(),
        &CheckingRequest::settled().with_declaration_authority(Some(&denied)),
    )
    .expect("sibling selections are not admitted by this realization");
    let mut provisional = typed;
    typed_trees_to_checked_trees::prune_target_siblings_typed(&mut provisional);
    lower_typed_trees(
        provisional,
        &CheckingRequest::preliminary().with_declaration_authority(Some(&denied)),
    )
    .expect("already-pruned sibling occurrences retain their exclusion");
    assert_eq!(denied.dependency_queries.load(Ordering::Relaxed), 0);
}

#[test]
fn sibling_exclusion_does_not_hide_a_portable_selection_in_the_same_file() {
    let typed = fixture(
        true,
        "demo_target machine sibling(value: Visible) -> u64 { 0 }
         machine consumer(value: Visible) -> u64 { 1 }",
    );
    let denied = Authority::new(false);
    let diagnostics = lower_typed_trees(
        typed,
        &CheckingRequest::settled().with_declaration_authority(Some(&denied)),
    )
    .expect_err("the portable caller still requires the direct dependency");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("without declaring that package as a direct dependency")
    }));
    assert!(denied.dependency_queries.load(Ordering::Relaxed) > 0);
}
