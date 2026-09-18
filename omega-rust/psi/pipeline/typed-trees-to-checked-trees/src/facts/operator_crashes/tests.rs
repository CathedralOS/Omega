//! Named `Namespace::requirement(...)` crash uses prove route falsity from
//! their own operand-time invocation captures — recorded against the
//! `named_uses` row, never a fabricated `uses` row or ordinary call fact —
//! with the same per-operand and per-context selection a spelled use gets.
//! A call whose evaluation emitted no capture falls back to the containing
//! statement's entry contexts, and only while every place a guard leaf reads
//! keeps its invocation-entry provenance below the operand it binds to.

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
fn named_call_discharges_a_route_whose_exclusive_loan_never_writes() {
    // `peek` borrows `value` exclusively but provably never writes it, so the
    // operand-time capture of `value` still carries `value >= 0` at the
    // named call. The statement-entry provenance gate could not see the
    // callee's write summary; the captured constraints can.
    let source = "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         crashes Trap !(right >= 0);
         machine peek(x: &mut i32) { }
         pub machine shadowed(mut value: i32) -> bool
         requires value >= 0 {
             peek(&mut value);
             Comparison::equal(1, value)
         }";
    check(source).expect("an exclusive loan whose callee never writes keeps the premise live");
    let checked = inspect(source);
    let sites = named_sites(&checked);
    let [site] = sites.as_slice() else {
        panic!("one named operator crash site")
    };
    assert!(site.surviving.is_empty());
}

#[test]
fn named_call_keeps_a_route_whose_operand_a_borrow_rewrites() {
    // `poke` writes through its exclusive loan, so the operand-time capture
    // of `value` no longer holds `value >= 0` and the route survives.
    let source = "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         crashes Trap !(right >= 0);
         machine poke(x: &mut i32) { x = 0; }
         pub machine shadowed(mut value: i32) -> bool
         requires value >= 0 {
             poke(&mut value);
             Comparison::equal(1, value)
         }";
    let diagnostics =
        check(source).expect_err("an operand rewritten through a borrow keeps its route");
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
fn named_call_discharges_a_route_through_a_mutable_receiver_field() {
    // `&mut self` is bound once at the invocation and never rebound; with no
    // statement anywhere in the machine able to write `self.count`, the read
    // still names the entry field, so the statement-entry fact covers it.
    let source = "pub data Main { count: i32; }
         boundary operator Ns::probe(value: i32) -> bool
         crashes Trap !(value >= 0);
         pub machine Main::check(&mut self) -> bool
         requires self.count >= 0 {
             Ns::probe(self.count)
         }";
    check(source).expect("the entry fact covers an unwritten mutable-receiver field");
    let checked = inspect(source);
    let sites = named_sites(&checked);
    let [site] = sites.as_slice() else {
        panic!("one named operator crash site")
    };
    assert!(site.surviving.is_empty());
}

#[test]
fn named_call_mutable_receiver_field_discharge_is_per_field() {
    // Writes to `self.other` and a `&mut self` call on `self.inner`'s own
    // machine never reach `self.count`; a later state still reads the entry
    // field even when `run` itself never writes.
    let source = "pub data Inner { n: i32; }
         pub machine Inner::bump(&mut self) { self.n = 1; }
         pub data Main { count: i32; other: i32; inner: Inner; }
         boundary operator Ns::probe(value: i32) -> bool
         crashes Trap !(value >= 0);
         pub machine Main::check(&mut self) -> bool
         requires self.count >= 0 {
             self.other = 1;
             self.inner.bump();
             transition true { true -> work() false -> true }
             state work(&mut self) -> bool
             requires self.count >= 0 {
                 Ns::probe(self.count)
             }
         }";
    check(source).expect("sibling writes do not escape the read field's entry identity");
    let checked = inspect(source);
    let sites = named_sites(&checked);
    let [site] = sites.as_slice() else {
        panic!("one named operator crash site")
    };
    assert!(site.surviving.is_empty());
}

#[test]
fn named_call_keeps_a_route_whose_mutable_receiver_field_is_written() {
    // `self.count = -1` reaches the exact projection the guard reads, so the
    // operand carries no entry identity and the route survives widened to
    // `Truth` rather than borrowing the statement-entry fact.
    let source = "pub data Main { count: i32; }
         boundary operator Ns::probe(value: i32) -> bool
         crashes Trap !(value >= 0);
         pub machine Main::check(&mut self) -> bool
         requires self.count >= 0 {
             self.count = -1;
             Ns::probe(self.count)
         }";
    let diagnostics = check(source).expect_err("a written receiver field keeps its route");
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
    let [bucket] = site.surviving.as_slice() else {
        panic!("one surviving bucket: {:?}", site.surviving)
    };
    assert_eq!(
        bucket.alternative_guards(),
        &[checked_trees::CrashRouteGuard::Truth],
    );
}

#[test]
fn named_call_mutable_receiver_field_window_excludes_states_that_cannot_precede() {
    // `done` writes `self.count` but runs only after `check`'s read and can
    // never return, so it sits outside the escape window: the entry fact
    // still covers the read and the route discharges.
    let source = "pub data Main { count: i32; }
         boundary operator Ns::probe(value: i32) -> bool
         crashes Trap !(value >= 0);
         pub machine Main::check(&mut self) -> bool
         requires self.count >= 0 {
             let seen: bool = Ns::probe(self.count);
             transition true { true -> done() false -> seen }
             state done(&mut self) -> bool { self.count = -1; true }
         }";
    check(source).expect("a downstream-only write cannot precede the read");
    let checked = inspect(source);
    let sites = named_sites(&checked);
    let [site] = sites.as_slice() else {
        panic!("one named operator crash site")
    };
    assert!(site.surviving.is_empty());
}

#[test]
fn named_call_mutable_receiver_field_cycle_discharges_when_reentry_reestablishes() {
    // `done` writes `self.count` and can return to `check`, but every `check`
    // arrival must satisfy its `self.count >= 0` entry requirement — the
    // guarded re-entry cannot be taken after the write — so the fact captured
    // at `Ns::probe` is genuinely live at the invocation and the route
    // discharges. Operand-time custody replaces the escape window's
    // conservative "a cycling predecessor may precede the read" retention.
    let source = "pub data Main { count: i32; }
         boundary operator Ns::probe(value: i32) -> bool
         crashes Trap !(value >= 0);
         pub machine Main::check(&mut self) -> bool
         requires self.count >= 0 {
             let seen: bool = Ns::probe(self.count);
             transition true { true -> done() false -> seen }
             state done(&mut self) -> bool {
                 self.count = -1;
                 transition self.count >= 0 { true -> check() false -> true }
             }
         }";
    check(source).expect("the re-entry requirement re-establishes the premise at every arrival");
    let checked = inspect(source);
    let sites = named_sites(&checked);
    let [site] = sites.as_slice() else {
        panic!("one named operator crash site")
    };
    assert!(site.surviving.is_empty());
}

#[test]
fn named_call_keeps_a_route_whose_mutable_receiver_escapes() {
    // A `&mut self` receiver call that writes the read field and a write
    // through an exclusive `&mut self` loan each retire `self.count` from the
    // captured constraints — no matter where in the machine the escape sits.
    for body in [
        "self.recompute(); Ns::probe(self.count)",
        "let loan: &mut Main = &mut self; loan.count = -1; Ns::probe(self.count)",
    ] {
        let source = &format!(
            "pub data Main {{ count: i32; }}
             boundary operator Ns::probe(value: i32) -> bool
             crashes Trap !(value >= 0);
             pub machine Main::recompute(&mut self) {{ self.count = -1; }}
             pub machine Main::check(&mut self) -> bool
             requires self.count >= 0 {{
                 {body}
             }}",
        );
        let diagnostics = check(source).expect_err("a receiver escape that writes keeps the route");
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
        assert_eq!(site.surviving.len(), 1, "{body}");
    }
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
fn named_call_discharge_reads_the_guard_field_through_a_sibling_write() {
    // `rec` is mutable: writing `rec.other` ends whole-storage provenance but
    // not the `count` projection the guard actually reads through `&rec`, so
    // the statement-entry fact still falsifies the route.
    let source = "pub data Rec { count: i32; other: i32; }
         boundary operator Ns::probe(cell: &Rec) -> bool
         crashes Trap !(cell.count >= 0);
         pub machine safe(mut rec: Rec) -> bool
         requires rec.count >= 0 {
             rec.other = 1;
             Ns::probe(&rec)
         }";
    check(source).expect("a sibling-field write keeps the read projection's provenance");
    let checked = inspect(source);
    let sites = named_sites(&checked);
    let [site] = sites.as_slice() else {
        panic!("one named operator crash site")
    };
    assert_eq!(site.published.len(), 1);
    assert!(site.surviving.is_empty());
}

#[test]
fn named_call_discharge_reads_the_guard_field_through_a_sibling_loan() {
    // An exclusive borrow of `rec.other` likewise never reaches `rec.count`.
    let source = "pub data Rec { count: i32; other: i32; }
         boundary operator Ns::probe(cell: &Rec) -> bool
         crashes Trap !(cell.count >= 0);
         machine peek(x: &mut i32) { }
         pub machine safe(mut rec: Rec) -> bool
         requires rec.count >= 0 {
             peek(&mut rec.other);
             Ns::probe(&rec)
         }";
    check(source).expect("an exclusive sibling-field loan keeps the read projection's provenance");
    let checked = inspect(source);
    let sites = named_sites(&checked);
    let [site] = sites.as_slice() else {
        panic!("one named operator crash site")
    };
    assert!(site.surviving.is_empty());
}

#[test]
fn named_call_keeps_a_route_whose_read_field_is_rewritten() {
    // The write reaches the exact projection the guard reads, so the
    // statement-entry fact no longer describes `rec.count` at the call.
    let source = "pub data Rec { count: i32; other: i32; }
         boundary operator Ns::probe(cell: &Rec) -> bool
         crashes Trap !(cell.count >= 0);
         pub machine drifted(mut rec: Rec) -> bool
         requires rec.count >= 0 {
             rec.count = -1;
             Ns::probe(&rec)
         }";
    let diagnostics = check(source).expect_err("a rewritten read projection keeps its route");
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
fn named_call_keeps_a_route_whose_read_field_is_rewritten_through_a_loan() {
    // `&mut rec.count` lends the read projection itself and the callee writes
    // through it, so the captured constraints for `&rec` no longer carry
    // `rec.count >= 0` at the named call.
    let source = "pub data Rec { count: i32; other: i32; }
         boundary operator Ns::probe(cell: &Rec) -> bool
         crashes Trap !(cell.count >= 0);
         machine poke(x: &mut i32) { x = 0; }
         pub machine drifted(mut rec: Rec) -> bool
         requires rec.count >= 0 {
             poke(&mut rec.count);
             Ns::probe(&rec)
         }";
    let diagnostics =
        check(source).expect_err("a loan of the read projection that writes keeps its route");
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
fn named_call_discharges_a_route_through_a_mutated_by_value_operand() {
    // The operand is the record itself, not a borrow: the leaf's `count`
    // projection is still all the guard reads.
    let source = "pub data Rec { count: i32; other: i32; }
         boundary operator Ns::probe(cell: Rec) -> bool
         crashes Trap !(cell.count >= 0);
         pub machine safe(mut rec: Rec) -> bool
         requires rec.count >= 0 {
             rec.other = 1;
             Ns::probe(rec)
         }";
    check(source).expect("a by-value operand's read projection survives a sibling write");
    let checked = inspect(source);
    let sites = named_sites(&checked);
    let [site] = sites.as_slice() else {
        panic!("one named operator crash site")
    };
    assert!(site.surviving.is_empty());
}

#[test]
fn named_call_discharge_versions_each_operand_projection_independently() {
    // A relation leaf reads one field per operand; `left.other` interferes
    // with neither `left.count` nor `right.count`, so both projections keep
    // entry provenance and the fact discharges the route.
    let source = "pub data Rec { count: i32; other: i32; }
         boundary operator Ns::paired(left: &Rec, right: &Rec) -> bool
         crashes Trap !(left.count == right.count);
         pub machine paired(mut left: Rec, right: Rec) -> bool
         requires left.count == right.count {
             left.other = 1;
             Ns::paired(&left, &right)
         }";
    check(source).expect("each operand's read projection is provenance-checked independently");
    let checked = inspect(source);
    let sites = named_sites(&checked);
    let [site] = sites.as_slice() else {
        panic!("one named operator crash site")
    };
    assert!(site.surviving.is_empty());
}

#[test]
fn named_call_discharge_follows_the_leafs_nested_projection() {
    // `cell.outer.count` reads `rec.outer.count`: a sibling write outside
    // that path preserves it, a write inside it ends it.
    let discharge = "pub data Inner { count: i32 }
         pub data Rec { outer: Inner; other: i32; }
         boundary operator Ns::probe(cell: &Rec) -> bool
         crashes Trap !(cell.outer.count >= 0);
         pub machine safe(mut rec: Rec) -> bool
         requires rec.outer.count >= 0 {
             rec.other = 1;
             Ns::probe(&rec)
         }";
    check(discharge).expect("a sibling write keeps the nested read projection's provenance");
    let retain = "pub data Inner { count: i32 }
         pub data Rec { outer: Inner; other: i32; }
         boundary operator Ns::probe(cell: &Rec) -> bool
         crashes Trap !(cell.outer.count >= 0);
         pub machine drifted(mut rec: Rec) -> bool
         requires rec.outer.count >= 0 {
             rec.outer.count = -1;
             Ns::probe(&rec)
         }";
    let diagnostics =
        check(retain).expect_err("a write inside the read projection keeps its route");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("uncovered")),
        "{diagnostics:#?}"
    );
}

/// The surviving guard for `crashes Trap !(cell.count >= 0)` over a caller
/// whose parameter `rec` is operand 0: the entry operand is the caller's
/// `rec` snapshot, and the leaf's `count` projection rides its own `Member`
/// node — never doubled by the resolver.
fn negated_count_at_least_zero() -> checked_trees::CrashPredicateExpression {
    use checked_trees::CrashPredicateExpression;
    use typed_trees::expression::{BinaryOperator, UnaryOperator};
    CrashPredicateExpression::Unary {
        operator: UnaryOperator::LogicalNot as u8,
        operand: Box::new(CrashPredicateExpression::Binary {
            operator: BinaryOperator::GreaterOrEqual as u8,
            left: Box::new(CrashPredicateExpression::Member {
                receiver: Box::new(CrashPredicateExpression::Parameter(0)),
                member: "count".to_owned(),
            }),
            right: Box::new(CrashPredicateExpression::Integer("0".to_owned())),
        }),
    }
}

#[test]
fn named_call_surviving_route_keeps_the_projected_caller_field() {
    // `rec.other` was written, so `rec` has no whole-operand entry identity —
    // but the guard reads only `cell.count`, whose projection stayed pristine.
    // The surviving route names the caller's `rec.count` rather than widening
    // to `Truth`.
    let source = "pub data Rec { count: i32; other: i32; }
         boundary operator Ns::probe(cell: &Rec) -> bool
         crashes Trap !(cell.count >= 0);
         pub machine uncovered(mut rec: Rec) -> bool {
             rec.other = 1;
             Ns::probe(&rec)
         }";
    let diagnostics =
        check(source).expect_err("the unproven route survives with an exact per-field caller name");
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
    let [bucket] = site.surviving.as_slice() else {
        panic!("one surviving bucket: {:?}", site.surviving)
    };
    assert_eq!(
        bucket.alternative_guards(),
        &[checked_trees::CrashRouteGuard::Predicate(
            checked_trees::CrashPredicateIdentity::from_expression(negated_count_at_least_zero()),
        )],
    );
}

#[test]
fn named_call_surviving_route_widens_when_the_read_field_is_rewritten() {
    // `rec.count = -1` reaches the exact projection the guard reads, so no
    // per-field provenance survives and the route keeps its unconditional
    // widening.
    let source = "pub data Rec { count: i32; other: i32; }
         boundary operator Ns::probe(cell: &Rec) -> bool
         crashes Trap !(cell.count >= 0);
         pub machine drifted(mut rec: Rec) -> bool {
             rec.count = -1;
             Ns::probe(&rec)
         }";
    let diagnostics = check(source).expect_err("a rewritten read projection keeps its route");
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
    let [bucket] = site.surviving.as_slice() else {
        panic!("one surviving bucket: {:?}", site.surviving)
    };
    assert_eq!(
        bucket.alternative_guards(),
        &[checked_trees::CrashRouteGuard::Truth],
    );
}

#[test]
fn spelled_use_surviving_route_keeps_the_projected_caller_field() {
    // The same per-field substitution serves a spelled use: the invoked
    // operand capture is not what supplies the leaf projection.
    let source = "pub data Rec { count: i32; other: i32; }
         boundary operator + Same::add(left: Rec, right: Rec) -> bool
         crashes Trap !(left.count >= 0);
         pub machine uncovered(mut rec: Rec, other: Rec) -> bool {
             rec.other = 1;
             rec + other
         }";
    let diagnostics = check(source)
        .expect_err("the unproven spelled route survives with an exact per-field caller name");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("uncovered")),
        "{diagnostics:#?}"
    );
    let checked = inspect(source);
    let sites = checked
        .facts
        .contract_plans
        .machines
        .iter()
        .flat_map(|machine| machine.crash.checked_operators())
        .collect::<Vec<_>>();
    let [site] = sites.as_slice() else {
        panic!("one operator crash site")
    };
    let [bucket] = site.surviving.as_slice() else {
        panic!("one surviving bucket: {:?}", site.surviving)
    };
    assert_eq!(
        bucket.alternative_guards(),
        &[checked_trees::CrashRouteGuard::Predicate(
            checked_trees::CrashPredicateIdentity::from_expression(negated_count_at_least_zero()),
        )],
    );
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

#[test]
fn named_call_discharges_a_route_its_short_circuit_premise_establishes() {
    // The `&&` left operand's evaluated predicate exists only between the
    // operands and the invocation: statement-entry contexts predate it, so
    // discharge must come from the named call's own operand-time capture —
    // the same evidence the spelled `1 == value` use consumes.
    let source = "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         crashes Trap !(right >= 0);
         pub machine compare(value: i32) -> bool {
             value >= 0 && Comparison::equal(1, value)
         }";
    check(source).expect("the operand-time capture carries the short-circuit premise");
    let checked = inspect(source);
    let sites = named_sites(&checked);
    let [site] = sites.as_slice() else {
        panic!("one named operator crash site")
    };
    assert_eq!(site.published.len(), 1);
    assert!(site.surviving.is_empty());
    // The capture row is keyed by the named use itself; no `uses` row or
    // ordinary call fact is fabricated for it.
    assert!(
        checked
            .facts
            .flow
            .control
            .operator_invocations
            .iter()
            .any(|(_, invocation)| invocation.named_use == site.named_use
                && !invocation.operator_use.is_valid()),
        "the named use owns an operand-time capture row"
    );
}

#[test]
fn named_call_keeps_a_route_whose_premise_an_earlier_operand_overwrote() {
    // `bump(&mut value)` runs before `value` is read as the right operand, so
    // the operand-time capture of `value` no longer holds `value >= 0` — the
    // stale statement-entry premise cannot discharge the route.
    let source = "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         crashes Trap !(right >= 0);
         machine bump(value: &mut i32) -> i32 { value = 0; 0 }
         pub machine drifted(mut value: i32) -> bool
         requires value >= 0 {
             Comparison::equal(bump(&mut value), value)
         }";
    let diagnostics = check(source)
        .expect_err("a write between operand evaluation and invocation voids the premise");
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
fn named_call_discharge_intersects_shared_borrow_operands_at_invocation() {
    // A relation leaf reads both reference operands: the proving fact must be
    // live in each operand's captured constraints at invocation — the same
    // per-context intersection a spelled use performs.
    let source = "pub data Rec { count: i32; }
         boundary operator Ns::paired(left: &Rec, right: &Rec) -> bool
         crashes Trap !(left.count >= right.count);
         pub machine paired(a: Rec, b: Rec) -> bool
         requires a.count >= b.count {
             Ns::paired(&a, &b)
         }";
    check(source).expect("the relation leaf proves through both captured borrow operands");
    let checked = inspect(source);
    let sites = named_sites(&checked);
    let [site] = sites.as_slice() else {
        panic!("one named operator crash site")
    };
    assert!(site.surviving.is_empty());
}

#[test]
fn named_call_keeps_a_route_whose_referent_an_earlier_operand_overwrote() {
    // `bump(&mut rec)` inside the `&&` left operand writes the referent before
    // `&rec` is captured; the non-scalar carrier intersects capture with
    // invocation-live facts, so the stale entry premise cannot discharge.
    let source = "pub data Rec { count: i32; }
         boundary operator Ns::probe(cell: &Rec) -> bool
         crashes Trap !(cell.count >= 0);
         machine bump(rec: &mut Rec) -> bool { rec.count = -1; true }
         pub machine drifted(mut rec: Rec) -> bool
         requires rec.count >= 0 {
             bump(&mut rec) && Ns::probe(&rec)
         }";
    let diagnostics =
        check(source).expect_err("a referent write before the named call keeps its route");
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
fn named_call_capture_rows_fail_closed_when_their_operands_are_substituted() {
    // A capture row exists, so the entry-context fallback does not apply: a
    // row whose recorded operands no longer match the call selects no
    // contexts and the route survives rather than borrowing stale evidence.
    let source = "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         crashes Trap !(right >= 0);
         pub machine compare(value: i32) -> bool
         requires value >= 0 {
             Comparison::equal(1, value)
         }";
    let checked = check(source).expect("the capture discharges the route");
    let mut facts = checked.facts.clone();
    let captures: Vec<_> = facts
        .flow
        .control
        .operator_invocations
        .iter()
        .filter_map(|(handle, invocation)| invocation.named_use.is_valid().then_some(handle))
        .collect();
    assert_eq!(captures.len(), 1, "one named capture row exists to corrupt");
    for handle in captures {
        facts
            .flow
            .control
            .operator_invocations
            .get_mut(handle)
            .operands = arena::HandleSpan::empty();
    }
    let diagnostics = crate::checks::check_checked_facts(&checked.typed, &facts)
        .expect_err("substituted operand custody cannot discharge the named route");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("captured source occurrence")),
        "{diagnostics:#?}"
    );
}

#[test]
fn named_call_capture_rows_fail_closed_when_duplicated() {
    // Two rows claiming the same named use are ambiguous custody: neither may
    // supply the operand-time evidence, so the route survives.
    let source = "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         crashes Trap !(right >= 0);
         pub machine compare(value: i32) -> bool
         requires value >= 0 {
             Comparison::equal(1, value)
         }";
    let checked = check(source).expect("the capture discharges the route");
    let mut facts = checked.facts.clone();
    let duplicate = facts
        .flow
        .control
        .operator_invocations
        .iter()
        .map(|(_, invocation)| invocation.clone())
        .find(|invocation| invocation.named_use.is_valid())
        .expect("one named capture row exists to duplicate");
    facts.flow.control.operator_invocations.append(duplicate);
    let diagnostics = crate::checks::check_checked_facts(&checked.typed, &facts)
        .expect_err("duplicated operand custody cannot discharge the named route");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("captured source occurrence")),
        "{diagnostics:#?}"
    );
}
