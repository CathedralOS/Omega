//! Named `Namespace::requirement(...)` crash uses prove route falsity from
//! the containing statement's entry contexts — never from a fabricated flow
//! invocation capture, and only while every place a guard leaf reads is an
//! entry-proven operand.

use checked_trees::CheckedTrees;
use typed_trees::TypedTrees;

fn typed_program(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

fn check(source: &str) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    crate::lower_typed_trees(typed_program(source))
}

fn inspect(source: &str) -> CheckedTrees {
    crate::checking::lower_typed_trees_for_crash_fact_inspection(typed_program(source))
        .expect("crash-fact inspection lowers without crash admission")
}

fn named_sites(checked: &CheckedTrees) -> Vec<&checked_trees::CheckedCrashOperatorSite> {
    checked
        .facts
        .contract_plans
        .machines
        .iter()
        .flat_map(|machine| machine.crash.checked_operators())
        .filter(|site| site.named_use.is_valid())
        .collect()
}

#[test]
fn named_call_discharges_a_route_its_statement_entry_context_proves_false() {
    let source = "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         crashes Trap !(right >= 0);
         pub machine safe(value: i32) -> bool
         requires value >= 0 {
             Comparison::equal(1, value)
         }";
    check(source).expect("a statement-entry fact covering the named invocation discharges it");
    let checked = inspect(source);
    let sites = named_sites(&checked);
    let [site] = sites.as_slice() else {
        panic!("one named operator crash site")
    };
    // The authored obligation stays published in formal coordinates; the
    // discharged route leaves an empty surviving set — a proved discharge,
    // not a missing analysis.
    assert_eq!(site.published.len(), 1);
    assert!(site.surviving.is_empty());
}

#[test]
fn named_call_retains_a_route_no_statement_entry_fact_covers() {
    let source = "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         crashes Trap !(right >= 0);
         pub machine uncovered(value: i32) -> bool {
             Comparison::equal(1, value)
         }";
    let diagnostics = check(source).expect_err("an unproven named route cannot vanish");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("uncovered")),
        "{diagnostics:#?}"
    );
    let checked = inspect(source);
    let sites = named_sites(&checked);
    let [site] = sites.as_slice() else {
        panic!("one named operator crash site")
    };
    assert_eq!(site.surviving.len(), 1);
}

#[test]
fn named_call_discharge_survives_writes_to_disjoint_sibling_storage() {
    // The statement-entry fact about `value` is untouched by the writes to
    // `x`, and `value`'s immutable binding is entry-proven, so the route
    // still discharges at the later statement.
    check(
        "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         crashes Trap !(right >= 0);
         pub machine safe(value: i32) -> bool
         requires value >= 0 {
             let mut x: i32 = value;
             x = x - 100;
             Comparison::equal(1, value)
         }",
    )
    .expect("sibling writes do not disturb an entry-proven operand");
}

#[test]
fn named_call_keeps_a_route_whose_operand_lacks_entry_provenance() {
    // `peek` never writes, but an exclusive loan of `value` ends its
    // bound-snapshot provenance regardless: `entry_operand` cannot promise
    // the post-loan read still holds the bound value, so no statement-entry
    // fact about `value` may discharge the guard.
    let source = "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         crashes Trap !(right >= 0);
         machine peek(x: &mut i32) { }
         pub machine shadowed(mut value: i32) -> bool
         requires value >= 0 {
             peek(&mut value);
             Comparison::equal(1, value)
         }";
    let diagnostics =
        check(source).expect_err("an operand read behind an exclusive borrow keeps its route");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("uncovered")),
        "{diagnostics:#?}"
    );
    let checked = inspect(source);
    let sites = named_sites(&checked);
    let [site] = sites.as_slice() else {
        panic!("one named operator crash site")
    };
    assert_eq!(site.surviving.len(), 1);
}

#[test]
fn named_call_keeps_a_route_after_the_operand_storage_is_overwritten() {
    // `value` is mutable: the earlier write both retires the statement-entry
    // fact about it and ends its bound-snapshot provenance, so nothing may
    // discharge the route at the call.
    let source = "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         crashes Trap !(right >= 0);
         pub machine drifted(mut value: i32) -> bool
         requires value >= 0 {
             value = value - 1;
             Comparison::equal(1, value)
         }";
    let diagnostics = check(source).expect_err("a written operand cannot borrow its entry facts");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("uncovered")),
        "{diagnostics:#?}"
    );
}
