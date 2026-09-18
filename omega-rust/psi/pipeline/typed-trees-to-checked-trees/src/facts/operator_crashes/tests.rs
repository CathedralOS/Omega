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
fn named_call_discharges_a_route_through_a_shared_borrow_operand() {
    // `&rec` is a non-scalar carrier: the guard reads the referent's storage,
    // and `rec`'s entry-proven binding keeps the statement-entry fact live
    // through the invocation, so the route discharges.
    let source = "pub data Rec { count: i32 }
         boundary operator Ns::probe(cell: &Rec) -> bool
         crashes Trap !(cell.count >= 0);
         pub machine safe(rec: Rec) -> bool
         requires rec.count >= 0 {
             Ns::probe(&rec)
         }";
    check(source).expect("a shared borrow of an entry-proven referent discharges the route");
    let checked = inspect(source);
    let sites = named_sites(&checked);
    let [site] = sites.as_slice() else {
        panic!("one named operator crash site")
    };
    assert_eq!(site.published.len(), 1);
    assert!(site.surviving.is_empty());
}

#[test]
fn named_call_discharges_a_route_across_multiple_operands() {
    // A relation leaf touches both operands; each must carry entry
    // provenance for the same statement-entry fact to falsify it.
    let source = "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         crashes Trap !(left != right);
         pub machine neq(a: i32, b: i32) -> bool
         requires a != b {
             Comparison::equal(a, b)
         }";
    check(source).expect("a fact covering both entry-proven operands discharges the route");
    let checked = inspect(source);
    let sites = named_sites(&checked);
    let [site] = sites.as_slice() else {
        panic!("one named operator crash site")
    };
    assert_eq!(site.published.len(), 1);
    assert!(site.surviving.is_empty());
}

#[test]
fn named_call_retains_a_route_through_a_shared_borrow_without_a_covering_fact() {
    // The referent is entry-proven, but nothing proves `rec.count >= 0` at
    // statement entry, so the route stays.
    let source = "pub data Rec { count: i32 }
         boundary operator Ns::probe(cell: &Rec) -> bool
         crashes Trap !(cell.count >= 0);
         pub machine uncovered(rec: Rec) -> bool {
             Ns::probe(&rec)
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
fn named_call_keeps_a_route_whose_borrowed_referent_is_overwritten() {
    // `rec` is mutable: the earlier write ends its bound-snapshot
    // provenance, so `&rec` cannot carry the entry `rec.count >= 0` fact
    // into the invocation and the route stays.
    let source = "pub data Rec { count: i32 }
         boundary operator Ns::probe(cell: &Rec) -> bool
         crashes Trap !(cell.count >= 0);
         pub machine drifted(mut rec: Rec) -> bool
         requires rec.count >= 0 {
             rec = Rec { count: 1 };
             Ns::probe(&rec)
         }";
    let diagnostics = check(source).expect_err("a written referent cannot borrow its entry facts");
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
fn named_call_keeps_a_route_through_a_borrow_of_an_unproven_aggregate() {
    // `p`'s immutable storage is stable, but its aggregate initializer has
    // no entry operand and no statement-entry fact names `p.x`, so the route
    // stays.
    let source = "pub data Point { x: i32 }
         boundary operator Ns::probe(cell: &Point) -> bool
         crashes Trap !(cell.x >= 0);
         pub machine uncovered(seed: i32) -> bool
         requires seed >= 0 {
             let p: Point = Point { x: seed };
             Ns::probe(&p)
         }";
    let diagnostics = check(source).expect_err("an unproven aggregate keeps its route");
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

#[test]
fn named_call_discharges_a_route_through_a_receiver_field_operand() {
    // `self.count` is an entry-proven operand: an immutable receiver is bound
    // once at the invocation and never rebound or written, so the
    // statement-entry fact `self.count >= 0` still describes the read.
    let source = "pub data Main { count: i32; }
         boundary operator Ns::probe(value: i32) -> bool
         crashes Trap !(value >= 0);
         pub machine Main::check(&self) -> bool
         requires self.count >= 0 {
             Ns::probe(self.count)
         }";
    check(source).expect("the entry fact covers the receiver-field operand");
    let checked = inspect(source);
    let sites = named_sites(&checked);
    let [site] = sites.as_slice() else {
        panic!("one named operator crash site")
    };
    assert_eq!(site.published.len(), 1);
    assert!(site.surviving.is_empty());
}

#[test]
fn named_call_discharges_a_receiver_field_route_in_a_non_entry_state() {
    // `self` is never rebound by a transition, so `work`'s own entry
    // requirement still describes the entry receiver's `count` field.
    let source = "pub data Main { count: i32; }
         boundary operator Ns::probe(value: i32) -> bool
         crashes Trap !(value >= 0);
         pub machine Main::check(&self) -> bool
         requires self.count >= 0 {
             transition true { true -> work() false -> true }
             state work(&self) -> bool requires self.count >= 0 { Ns::probe(self.count) }
         }";
    check(source).expect("the immutable receiver carries entry identity across states");
    let checked = inspect(source);
    let sites = named_sites(&checked);
    let [site] = sites.as_slice() else {
        panic!("one named operator crash site")
    };
    assert!(site.surviving.is_empty());
}

#[test]
fn named_call_keeps_a_route_whose_receiver_is_mutable() {
    // `&mut self` can write `self.count` between entry and the read, so the
    // operand carries no entry identity and the route cannot borrow the
    // statement-entry fact.
    let source = "pub data Main { count: i32; }
         boundary operator Ns::probe(value: i32) -> bool
         crashes Trap !(value >= 0);
         pub machine Main::check(&mut self) -> bool
         requires self.count >= 0 {
             Ns::probe(self.count)
         }";
    let diagnostics = check(source).expect_err("a mutable receiver operand keeps its route");
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
fn named_call_discharges_a_route_through_a_shared_borrow_of_a_receiver_field() {
    // `&self.cell` borrows the receiver's field: the referent's entry
    // identity is the immutable receiver's entry field.
    let source = "pub data Cell { count: i32; }
         pub data Main { cell: Cell; }
         boundary operator Ns::probe(cell: &Cell) -> bool
         crashes Trap !(cell.count >= 0);
         pub machine Main::check(&self) -> bool
         requires self.cell.count >= 0 {
             Ns::probe(&self.cell)
         }";
    check(source).expect("a shared borrow of an immutable receiver field discharges the route");
    let checked = inspect(source);
    let sites = named_sites(&checked);
    let [site] = sites.as_slice() else {
        panic!("one named operator crash site")
    };
    assert!(site.surviving.is_empty());
}

fn published_guard_forms(
    checked: &CheckedTrees,
) -> Vec<Option<checked_trees::CheckedBooleanExpression>> {
    checked
        .facts
        .contract_plans
        .machines
        .iter()
        .flat_map(|machine| machine.crash.checked_operators())
        .flat_map(|site| site.published.iter())
        .flat_map(|bucket| bucket.alternative_guards())
        .map(|guard| match guard {
            checked_trees::CrashRouteGuard::Predicate(predicate) => {
                predicate.scalar_expression().cloned()
            }
            checked_trees::CrashRouteGuard::Truth => panic!("a guarded route, not Truth"),
        })
        .collect()
}

/// `!(right >= 0)` over the operator's own formals: `right` is dense scalar
/// position 1 (the Terminal operation-contract formal 2) and the literal
/// lands in the operand's `i32`.
fn assert_negated_right_at_least_zero(form: &checked_trees::CheckedBooleanExpression) {
    use checked_trees::{CheckedBooleanExpression, CheckedScalarExpression};
    use typed_trees::types::PrimitiveType;
    let CheckedBooleanExpression::Not(comparison) = form else {
        panic!("the negation survives: {form:?}");
    };
    let CheckedBooleanExpression::IntegerComparison { kind, left, right } = comparison.as_ref()
    else {
        panic!("an integer comparison over the formals: {form:?}");
    };
    assert_eq!(
        *kind,
        checked_trees::CheckedIntegerComparisonKind::LessOrEqual
    );
    assert!(
        matches!(
            left.as_ref(),
            CheckedScalarExpression::IntegerLiteral { .. }
        ),
        "{form:?}"
    );
    assert_eq!(
        right.as_ref(),
        &CheckedScalarExpression::Parameter {
            position: 1,
            primitive_type: PrimitiveType::I32,
        },
        "{form:?}"
    );
}

#[test]
fn authored_operator_crash_buckets_carry_their_structured_guard_forms() {
    // Both declaration forms publish the same guard over their own formal
    // parameters. Neither owns a machine contract plan in the checked
    // output, so the site's published bucket is the only carrier the
    // Terminal producer can read the form from. A spelled use and a named
    // call read the same declaration and therefore the same form.
    for (declaration, use_site) in [
        (
            "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
             crashes Trap !(right >= 0);",
            "1 == value",
        ),
        (
            "boundary machine == Comparison::equal(left: i32, right: i32) -> bool
             crashes Trap !(right >= 0);",
            "1 == value",
        ),
        (
            "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
             crashes Trap !(right >= 0);",
            "Comparison::equal(1, value)",
        ),
    ] {
        let source = format!(
            "{declaration}
             pub machine safe(value: i32) -> bool
             requires value >= 0 {{ {use_site} }}"
        );
        let checked = check(&source).expect("the entry fact discharges the route");
        let forms = published_guard_forms(&checked);
        let [Some(form)] = forms.as_slice() else {
            panic!("one guarded published route with a structured form: {forms:?}");
        };
        assert_negated_right_at_least_zero(form);
        let sites = checked
            .facts
            .contract_plans
            .machines
            .iter()
            .flat_map(|machine| machine.crash.checked_operators())
            .collect::<Vec<_>>();
        let [site] = sites.as_slice() else {
            panic!("one operator crash site");
        };
        assert!(
            checked
                .facts
                .contract_plans
                .for_machine(site.selected_operator)
                .is_none(),
            "{declaration}"
        );
        assert_eq!(published_guard_forms(&inspect(&source)), forms);
    }
}

#[test]
fn a_structural_operator_formal_keeps_its_route_identity_only() {
    // The operator reader binds scalar formals; a guard through a structural
    // formal has no scalar telescope position, so the route keeps its
    // canonical identity and no structured form. Nothing downstream may read
    // that absence as a crash-free route.
    let checked = inspect(
        "pub data Rec { count: i32 }
         boundary operator Ns::probe(cell: &Rec) -> bool
         crashes Trap !(cell.count >= 0);
         pub machine safe(rec: Rec) -> bool
         requires rec.count >= 0 {
             Ns::probe(&rec)
         }",
    );
    assert_eq!(published_guard_forms(&checked), vec![None]);
}
