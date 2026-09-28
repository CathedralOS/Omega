//! Named `Namespace::requirement(...)` crash uses prove route falsity from
//! their own operand-time invocation captures — recorded against the
//! `named_uses` row, never a fabricated `uses` row or ordinary call fact —
//! with the same per-operand and per-context selection a spelled use gets.
//! A call whose evaluation emitted no capture falls back to the containing
//! statement's entry contexts, and only while every place a guard leaf reads
//! keeps its invocation-entry provenance below the operand it binds to.
//! Token-bearing `boundary machine` declarations instead use ordinary calls:
//! their published routes live on the machine contract, and invocation-specific
//! refinements join that contract to the exact flow call.

use crate::tests::front_end::{checked_program_result, typed_program};
use checked_trees::CheckedTrees;

fn inspect(source: &str) -> CheckedTrees {
    crate::lower_typed_trees(
        typed_program(source),
        &crate::CheckingRequest::crash_fact_inspection(),
    )
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

fn boundary_machine_site<'a>(
    checked: &'a CheckedTrees,
    target_name: &str,
) -> (
    &'a checked_trees::CheckedCrashCallSite,
    &'a checked_trees::FlowCallFact,
) {
    let target = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == target_name)
        .expect("the boundary machine is a declared call target");
    let plans = &checked.facts.contract_plans;
    let target_plan = plans
        .for_machine(target.symbol)
        .expect("the callee owns its contract");
    let sites = plans
        .machines
        .iter()
        .flat_map(|plan| plan.crash.checked_calls())
        .filter(|site| site.target_machine() == target.symbol)
        .collect::<Vec<_>>();
    let [site] = sites.as_slice() else {
        panic!("one ordinary call to {target_name}: {sites:#?}");
    };
    assert_eq!(site.target_contract_commitment(), target_plan.commitment);
    assert!(
        plans
            .machines
            .iter()
            .flat_map(|plan| plan.crash.checked_operators())
            .all(|site| site.selected_operator != target.symbol),
        "a boundary-machine call must not fabricate a legacy operator site"
    );
    let location = site.location();
    let flow = &checked.facts.flow;
    let calls = flow
        .control
        .states
        .iter()
        .filter(|(_, state)| state.state_symbol == location.state())
        .flat_map(|(_, state)| flow.control.calls.span_or_empty(state.calls))
        .filter(|call| {
            call.statement_index == location.statement_ordinal() as usize
                && call.call_ordinal == location.call_ordinal() as usize
                && call.target_symbol == site.target_state()
        })
        .collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        panic!("one exact flow invocation for the checked crash site: {calls:#?}");
    };
    assert!(call.authored_expression.is_valid());
    (site, call)
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
    let diagnostics = checked_program_result(source)
        .expect_err("a written referent cannot borrow its entry facts");
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
    checked_program_result(source)
        .expect("the re-entry requirement re-establishes the premise at every arrival");
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
    checked_program_result(source)
        .expect("each operand's read projection is provenance-checked independently");
    let checked = inspect(source);
    let sites = named_sites(&checked);
    let [site] = sites.as_slice() else {
        panic!("one named operator crash site")
    };
    assert!(site.surviving.is_empty());
}

/// The rewrite can only carry a selection with a declared result type — a
/// `boundary operator` without one stays a `StatementNode::Call`, mints no
/// `named_uses` row, and has no operand capture. Its crash obligations must
/// fail closed rather than pass unexamined.
#[test]
fn unrewritten_statement_call_to_a_crash_contracted_operator_fails_closed() {
    for source in [
        "boundary operator Ns::store(v: i32)
         crashes Trap !(v >= 0);
         pub machine m(value: i32) requires value >= 0 { Ns::store(value); }",
        "operator Ns::store(v: i32) -> ()
         crashes Trap !(v >= 0);
         pub machine m(value: i32) requires value >= 0 { Ns::store(value); }",
    ] {
        let diagnostics = checked_program_result(source)
            .expect_err("a statement call with no retained site must not pass");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("carries crash routes no checked site can cover")
            }),
            "{diagnostics:#?}"
        );
    }
}

/// The named spelling uses the same machine contract and ordinary-call
/// discharge as the token spelling, not the legacy named-operator table.
#[test]
fn named_index_call_matches_the_spelled_index_discharge() {
    let source = "data Index {}
         boundary machine [] Index::at(items: &[i32], index: u64) -> i32
         requires index < items.len
         crashes Trap !(index >= 1);
         data Main {}
         machine Main::main(&mut self, items: &[i32], index: u64) -> i32
         requires index < items.len && index >= 1 {
             Index::at(items, index)
         }";
    checked_program_result(source).expect("the named call keeps the discharged route");
    let checked = inspect(source);
    let (site, _) = boundary_machine_site(&checked, "Index::at");
    assert!(site.surviving_buckets().is_empty());
    let spelled =
        checked_program_result(&source.replace("Index::at(items, index)", "items[index]"))
            .expect("the token spelling uses the same contract");
    let (spelled_site, _) = boundary_machine_site(&spelled, "Index::at");
    assert_eq!(site.surviving_buckets(), spelled_site.surviving_buckets());
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
    let checked = checked_program_result(source).expect("the capture discharges the route");
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

/// The named spelling of the same operator reads the same authored `crashes`
/// guard: `items.len` resolves to the collection-length intrinsic once, on
/// the declaration, and the named use carries the same surviving predicate
/// route the spelled use produced.
#[test]
fn named_index_call_crash_guard_resolves_collection_length_on_a_slice_formal() {
    let source = "data Index {}
         boundary machine [] Index::at(items: &[i32], index: u64) -> i32
         requires index < items.len
         crashes Trap items.len == 0;
         data Main {}
         machine Main::main(&mut self, items: &[i32], index: u64) -> i32
         requires index < items.len {
             Index::at(items, index)
         }";
    checked_program_result(source).expect("the named call retains the collection-length route");
    let checked = inspect(source);
    let (site, _) = boundary_machine_site(&checked, "Index::at");
    assert_eq!(
        checked
            .facts
            .contract_plans
            .for_machine(site.target_machine())
            .unwrap()
            .crash
            .published()
            .len(),
        1
    );
    assert_eq!(site.surviving_buckets().len(), 1);
}

fn spelled_site(checked: &CheckedTrees) -> &checked_trees::CheckedCrashOperatorSite {
    let sites: Vec<_> = checked
        .facts
        .contract_plans
        .machines
        .iter()
        .flat_map(|machine| machine.crash.checked_operators())
        .filter(|site| site.operator_use.is_valid())
        .collect();
    let [site] = sites.as_slice() else {
        panic!("one spelled operator crash site: {sites:?}")
    };
    site
}

fn surviving_guard_expressions(
    site: &checked_trees::CheckedCrashOperatorSite,
) -> Vec<&checked_trees::CrashPredicateExpression> {
    site.surviving
        .iter()
        .flat_map(|bucket| bucket.alternative_guards())
        .map(|guard| match guard {
            checked_trees::CrashRouteGuard::Predicate(identity) => identity
                .expression()
                .expect("a checked route keeps its predicate expression"),
            checked_trees::CrashRouteGuard::Truth => {
                panic!("the route must keep a structured predicate, not Truth")
            }
        })
        .collect()
}

/// A call leaf is structural: formals appearing inside its arguments still
/// substitute from the saved actuals, so `offset(left)` becomes `offset(1)`
/// at the use `1 == value`.
#[test]
fn spelled_use_substitutes_formals_inside_a_guard_call_argument() {
    use checked_trees::CrashPredicateExpression;
    use typed_trees::expression::{BinaryOperator, UnaryOperator};
    let source = "machine offset(amount: i32) -> i32 { amount }
         boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         crashes Trap !(right >= offset(left));
         machine keep(value: i32) -> bool {
             1 == value
         }";
    checked_program_result(source).expect("a formal inside a call argument still substitutes");
    let checked = inspect(source);
    let site = spelled_site(&checked);
    let expected = CrashPredicateExpression::Unary {
        operator: UnaryOperator::LogicalNot as u8,
        operand: Box::new(CrashPredicateExpression::Binary {
            operator: BinaryOperator::GreaterOrEqual as u8,
            left: Box::new(CrashPredicateExpression::Parameter(0)),
            right: Box::new(CrashPredicateExpression::Call {
                target: "offset".to_owned(),
                receiver: Box::new(CrashPredicateExpression::Invalid),
                arguments: vec![CrashPredicateExpression::Integer("1".to_owned())],
            }),
        }),
    };
    assert_eq!(surviving_guard_expressions(site), [&expected]);
}

/// A namespaced call keeps its receiver path: `Ns::floor()` encodes the
/// receiver as a `Name` leaf, which transports verbatim since a non-formal
/// name means the same path in caller coordinates.
#[test]
fn spelled_use_keeps_a_route_whose_guard_call_has_a_namespace_receiver() {
    use checked_trees::CrashPredicateExpression;
    use typed_trees::expression::{BinaryOperator, UnaryOperator};
    let source = "machine Ns::floor() -> i32 { 0 }
         boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         crashes Trap !(right >= Ns::floor());
         machine keep(value: i32) -> bool {
             1 == value
         }";
    checked_program_result(source).expect("a namespaced call leaf keeps its route structured");
    let checked = inspect(source);
    let site = spelled_site(&checked);
    let expected = CrashPredicateExpression::Unary {
        operator: UnaryOperator::LogicalNot as u8,
        operand: Box::new(CrashPredicateExpression::Binary {
            operator: BinaryOperator::GreaterOrEqual as u8,
            left: Box::new(CrashPredicateExpression::Parameter(0)),
            right: Box::new(CrashPredicateExpression::Call {
                target: "floor".to_owned(),
                receiver: Box::new(CrashPredicateExpression::Name(vec!["Ns".to_owned()])),
                arguments: Vec::new(),
            }),
        }),
    };
    assert_eq!(surviving_guard_expressions(site), [&expected]);
}

/// The transport boundary stays conservative outside `Indexed`: a cast leaf
/// still flattens to `Opaque`, whose display can hide a formal —
/// `left[0u64] as i32 in Wrapping` mentions `left` — so the route still
/// widens to `Truth` rather than leaking a callee spelling into caller
/// coordinates.
#[test]
fn named_call_still_widens_a_route_whose_opaque_leaf_hides_a_formal() {
    let source = "boundary operator Ns::probe(left: [i64; 4], right: i32) -> bool
         crashes Trap !(right >= (left[0u64] as i32 in Wrapping));
         machine keep(items: [i64; 4], value: i32) -> bool {
             Ns::probe(items, value)
         }";
    checked_program_result(source).expect("an internal caller retains the widened route");
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
