use crate::lower_typed_trees;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

fn check(source: &str, accepted: bool) {
    let tokens = Lexer::new(source)
        .tokenize()
        .unwrap_or_else(|diagnostics| panic!("tokenize: {diagnostics:#?}\n{source}"));
    let syntax = parse_syntax_trees(&tokens)
        .unwrap_or_else(|diagnostics| panic!("parse: {diagnostics:#?}\n{source}"));
    let resolved = resolve(ResolutionRequest::new(&syntax))
        .unwrap_or_else(|diagnostics| panic!("resolve: {diagnostics:#?}\n{source}"));
    let typed = lower_symbol_resolved_trees(&resolved)
        .unwrap_or_else(|diagnostics| panic!("type: {diagnostics:#?}\n{source}"));
    match lower_typed_trees(typed) {
        Ok(_) => assert!(accepted, "stale atomic bounds accepted: {source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic.is_error()),
                "expected an error: {diagnostics:#?}\n{source}"
            );
            assert!(
                diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.is_error())
                    .all(|diagnostic| diagnostic.message.contains("cannot prove subslice range")),
                "expected only subslice range errors, not earlier proof or authority failures: \
                 {diagnostics:#?}\n{source}"
            );
        }
    }
}

// A `place.load(ordering)` observation reads exactly its resident place, so a
// premise on the load survives writes that cannot reach that place and is
// retired by any write that can.
#[test]
fn atomic_store_retires_only_the_resident_place_premise() {
    fn source(mutation: &str) -> String {
        format!(
            "data Host {{ counter: AtomicU32; other: AtomicU32; unrelated: i64; }}
             machine Host::window(&mut self, items: &[i32; 4]) -> u64
             requires 0 <= self.counter.load(NoOrdering)
                 && self.counter.load(NoOrdering) <= 4; {{
                 {mutation}
                 let view: &[i32] = items[0..self.counter.load(NoOrdering)];
                 view.len
             }}"
        )
    }
    for (mutation, accepted) in [
        ("", true),
        ("self.unrelated = 1;", true),
        ("self.other.store(9, NoOrdering);", true),
        ("self.counter.store(9, NoOrdering);", false),
    ] {
        check(&source(mutation), accepted);
    }
}
