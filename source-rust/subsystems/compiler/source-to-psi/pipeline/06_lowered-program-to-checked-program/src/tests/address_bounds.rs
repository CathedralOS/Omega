//! `addr::Bound` in a contract clause, read from source as a Build program.
//!
//! counts_and_addresses.md: establishing valid geometry proves
//! `embed(region.base) + embed(region.length) <= addr::Bound` "in unbounded
//! proof integers". The bound is one past the largest address: a
//! mathematical integer, which no carrier holds. The lowered form gives
//! `addr` one carrier, so a clause reads the bound as that carrier's, and a
//! literal that large is a literal like any other.

use super::build_purpose::judge_build;
use super::*;

/// `accepted`, `rejected`, or the refusal's text.
fn verdict(build: &str) -> String {
    match judge_build(build) {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
        Ok(RouteVerdict::Accepted(accepted)) => {
            accepted.publish_invocation().unwrap();
            "accepted".to_owned()
        }
        Ok(RouteVerdict::Rejected { .. }) => "rejected".to_owned(),
        other => panic!("{other:?}"),
    }
}

/// A machine may require the address width it was written for, and a caller
/// owes it like any clause: true at the width the route gives `addr`, false
/// at any other.
#[test]
fn a_clause_states_the_address_bound_as_a_mathematical_integer() {
    let requiring = |bound: &str| {
        format!(
            "machine sized() requires addr::Bound == {bound} {{ }}
machine build_entry() {{ sized(); }}"
        )
    };
    assert_eq!(verdict(&requiring("18446744073709551616")), "accepted");
    assert_eq!(verdict(&requiring("4294967296")), "rejected");
}

/// A name that is no bound is still a runtime value, which a mathematical
/// term takes only through `embed`.
#[test]
fn a_runtime_value_in_a_mathematical_term_still_needs_embed() {
    assert_eq!(
        verdict(
            "machine sized(limit: u64) requires embed(limit) + limit == 3 { }
machine build_entry() { sized(1); }"
        ),
        "contract runtime value in a mathematical term without embed while reading the signature of \
         `sized`"
    );
}

/// counts_and_addresses.md: an `addr` is "interpreted under the selected
/// target's address model" and "grants no storage or access authority". A
/// value of it is carried and compared like any scalar. Exact subtraction
/// has its separate count/address signature and fit proof. Address addition
/// to another address and arbitrary value/address casts remain refused.
#[test]
fn address_comparisons_keep_unlicensed_arithmetic_and_casts_refused() {
    let using = |body: &str| {
        format!(
            "machine probe(base: addr, limit: addr) -> bool {{ {body} }}
machine build_entry(base: addr, limit: addr) {{ let seen: bool = probe(base, limit); }}"
        )
    };
    assert_eq!(verdict(&using("base == limit")), "accepted");
    assert_eq!(
        verdict(&using("let held: addr = base; held < limit")),
        "accepted"
    );
    assert_eq!(verdict(&using("base == 64")), "accepted");
    assert_eq!(
        verdict(&using("(base + limit) == limit")),
        "expression arithmetic on an address while reading the body of `probe`"
    );
    assert_eq!(
        verdict(&using("(base as u64) == 64")),
        "expression cast to or from an address while reading the body of `probe`"
    );
}

/// The count operand and the address distance keep their separate carriers,
/// including when the offset is an anonymous literal in an address context.
#[test]
fn exact_address_subtraction_keeps_address_and_count_results() {
    assert_eq!(
        verdict(
            "machine build_entry() {
    let a: addr = 4096;
    let b: addr = 4000;
    let back: addr = a - 16;
    let typed_count: u64 = 16;
    let back_again: addr = a - typed_count;
    let typed_literal_back: addr = a - 16u64;
    let distance: u64 = a - b;
}"
        ),
        "accepted"
    );
}

#[test]
fn exact_address_addition_keeps_counts_in_both_operand_orders() {
    let observed = verdict(
        "machine build_entry() {
    let base: addr = 4096;
    let count: u64 = 17;
    let forward: addr = base + count;
    let reversed: addr = count + base;
    let anonymous_right: addr = base + 17;
    let anonymous_left: addr = 17 + base;
    let explicit_right: addr = base + 17u64;
    let explicit_left: addr = 17u64 + base;
    let distance: u64 = forward - base;
}",
    );
    assert_eq!(observed, "accepted");
}

#[test]
fn exact_address_addition_uses_the_parameters_fit_contract() {
    let source = "machine advance(base: addr, count: u64) -> addr
requires embed(base) + embed(count) < addr::Bound
{ base + count }
machine reversed(count: u64, base: addr) -> addr
requires embed(base) + embed(count) < addr::Bound
{ count + base }
machine build_entry() {
    let base: addr = 4096;
    let count: u64 = 17;
    let forward: addr = advance(base, count);
    let backward_order: addr = reversed(count, base);
}";
    assert_eq!(verdict(source), "accepted");
    let insufficient = judge_build(
        "machine advance(base: addr, count: u64) -> addr
requires embed(base) + embed(count) <= addr::Bound
{ base + count }
machine build_entry() {
    let base: addr = 18446744073709551615;
    let beyond: addr = advance(base, 1u64);
}",
    );
    let Ok(RouteVerdict::Rejected { unproved, .. }) = insufficient else {
        panic!("an inclusive bound cannot prove the offset fits: {insufficient:?}");
    };
    assert_eq!(unproved.len(), 1, "{unproved:?}");
    assert!(
        matches!(unproved[0].kind, ObligationKind::OperationSafety { .. }),
        "{unproved:?}"
    );
}

/// The endpoint identity describes a completed add. It cannot establish that
/// the operands fit before that operation executes.
#[test]
fn a_correlated_endpoint_identity_keeps_its_independent_address_fit() {
    let source = "machine advance(base: addr, capacity: u64, requested: u64) -> addr
requires embed(base) + embed(capacity) <= addr::Bound, requested < capacity
ensures embed(result) == embed(base) + embed(requested)
{
    let endpoint: addr = base + requested;
    endpoint
}
machine build_entry() {
    let advanced: addr = advance(4096, 32u64, 17u64);
}";
    let missing_fit = source.replace("embed(base) + embed(capacity) <= addr::Bound, ", "");
    assert_ne!(source, missing_fit);
    let observed = [judge_build(source), judge_build(&missing_fit)];
    assert!(
        matches!(&observed[0], Ok(RouteVerdict::Accepted(_))),
        "{:?}",
        observed[0]
    );
    let Ok(RouteVerdict::Rejected {
        formations,
        violations,
        unproved,
        ..
    }) = &observed[1]
    else {
        panic!(
            "the endpoint identity cannot form its own address addition: {:?}",
            observed[1]
        );
    };
    assert!(
        formations.is_empty() && violations.is_empty(),
        "{:?}",
        observed[1]
    );
    assert!(
        unproved.iter().any(|failure| matches!(
            failure.kind,
            ObligationKind::OperationSafety { .. }
        ) && failure.statement.contains("local `endpoint`")),
        "{unproved:?}"
    );
}

#[test]
fn exact_address_addition_keeps_fit_and_operand_restrictions() {
    let invalid = [
        (
            "wrong count",
            "machine probe(base: addr, count: u32) -> addr { base + count }
machine build_entry() {}",
        ),
        (
            "two addresses",
            "machine probe(base: addr, other: addr) -> addr { base + other }
machine build_entry() {}",
        ),
        (
            "wrapping",
            "machine build_entry() { let base: addr = 4096; let next: addr = base +% 17u64; }",
        ),
        (
            "qualified count",
            "machine probe(base: addr, count: u64 in Trapping) -> addr { base + count }
machine build_entry() {}",
        ),
    ]
    .map(|(name, source)| (name, verdict(source)));
    let overflow = judge_build(
        "machine build_entry() {
    let base: addr = 18446744073709551615;
    let beyond: addr = base + 1u64;
}",
    );
    for (name, observed) in invalid {
        assert!(
            observed.contains("arithmetic on an address"),
            "{name}: {observed}"
        );
    }
    let Ok(RouteVerdict::Rejected { unproved, .. }) = overflow else {
        panic!("the exact overflow must fail its fit proof: {overflow:?}");
    };
    assert_eq!(unproved.len(), 1, "{unproved:?}");
    assert!(
        matches!(unproved[0].kind, ObligationKind::OperationSafety { .. }),
        "{unproved:?}"
    );
}

#[test]
fn exact_address_subtraction_refuses_wrong_count_carriers_and_underflow() {
    let wrong_count = verdict(
        "machine probe(base: addr, count: u32) -> addr { base - count }
machine build_entry() {}",
    );
    assert!(
        wrong_count.contains("operand types") || wrong_count.contains("arithmetic on an address"),
        "{wrong_count}"
    );
    assert_eq!(
        verdict(
            "machine build_entry() {
    let base: addr = 0;
    let back: addr = base - 1;
}"
        ),
        "rejected"
    );
    let wrapping = verdict(
        "machine build_entry() {
    let base: addr = 4096;
    let back: addr = base -% 16;
}",
    );
    assert!(wrapping.contains("arithmetic on an address"), "{wrapping}");
    let qualified_count = verdict(
        "machine probe(base: addr, count: u64 in Trapping) -> addr { base - count }
machine build_entry() {}",
    );
    assert!(
        qualified_count.contains("arithmetic on an address"),
        "{qualified_count}"
    );
}

/// Transparent declarations use the same address carrier and exclusive bound
/// as an ordinary clause. Removing the caller's geometry does not establish it.
#[test]
fn a_transparent_predicate_keeps_ordinary_address_geometry() {
    let source = |input_contract: &str| {
        format!(
            "public let no_wrap(base: addr, length: u64): core\\Strict<0> =
    embed(base) + embed(length) <= addr::Bound;
machine consume(base: addr, length: u64) requires no_wrap(base, length) {{ }}
machine build_entry(base: addr, length: u64) {input_contract} {{ consume(base, length); }}"
        )
    };
    assert_eq!(
        verdict(&source(
            "requires embed(base) + embed(length) <= addr::Bound"
        )),
        "accepted"
    );
    // Without the caller's bound, this call has an unproved requirement.
    // The compiler need not prove the proposition false to refuse the call.
    let unproved = match judge_build(&source("")) {
        Ok(RouteVerdict::Incomplete(unproved)) | Ok(RouteVerdict::Rejected { unproved, .. }) => {
            unproved
        }
        other => panic!("missing address geometry must refuse the call: {other:?}"),
    };
    assert_eq!(unproved.len(), 1, "{unproved:?}");
    assert!(
        matches!(unproved[0].kind, ObligationKind::CallRequirement { .. })
            && unproved[0].statement.contains("no_wrap(base, length)"),
        "{unproved:?}",
    );
}

/// A transparent definition's body is a clause like any other, so its
/// mathematical terms are read by the same reader as a machine's. Anonymous
/// arithmetic there is the exact integer it computes: `4 * 1024` is 4096.
/// The twin's caller only knows `length <= 4097`, which leaves the premise
/// unproved.
#[test]
fn a_transparent_definition_reads_anonymous_arithmetic_as_an_exact_integer() {
    const FITS: &str =
        include_str!("../../tests/fixtures/transparent_definition/pass/anonymous_bound/build.omg");
    const EXCEEDS: &str = include_str!(
        "../../tests/fixtures/transparent_definition/fail/anonymous_bound_exceeded/build.omg"
    );
    assert_eq!(verdict(FITS), "accepted");
    let unproved = match judge_build(EXCEEDS) {
        Ok(RouteVerdict::Incomplete(unproved)) | Ok(RouteVerdict::Rejected { unproved, .. }) => {
            unproved
        }
        other => panic!("a bound past 4096 must leave the premise unproved: {other:?}"),
    };
    assert_eq!(unproved.len(), 1, "{unproved:?}");
    assert!(
        matches!(unproved[0].kind, ObligationKind::CallRequirement { .. })
            && unproved[0].statement.contains("fits(length)"),
        "{unproved:?}",
    );
}

/// A transparent definition may take value binders, which a use writes as
/// static arguments: mathematical_bindings.md, "Generic arguments are
/// resolved first under their own binder rules". `below::<10>(value)`
/// unfolds to `value < 10`, which proves `result < 10`. The twin promises
/// `result < 9`, one input apart, and its guarantee is unproved.
#[test]
fn a_transparent_definition_takes_a_value_binder_from_its_static_argument() {
    const BELOW: &str =
        include_str!("../../tests/fixtures/transparent_definition/pass/value_binder/build.omg");
    const TOO_TIGHT: &str = include_str!(
        "../../tests/fixtures/transparent_definition/fail/value_binder_too_tight/build.omg"
    );
    let body = |source: &str| {
        source
            .lines()
            .filter(|line| !line.starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(
        body(TOO_TIGHT),
        body(BELOW).replace("ensures result < 10", "ensures result < 9")
    );
    assert_eq!(verdict(BELOW), "accepted");
    let unproved = match judge_build(TOO_TIGHT) {
        Ok(RouteVerdict::Incomplete(unproved)) | Ok(RouteVerdict::Rejected { unproved, .. }) => {
            unproved
        }
        other => panic!("a guarantee past the binder's bound must be unproved: {other:?}"),
    };
    assert!(
        unproved.iter().any(|failure| failure
            .statement
            .starts_with("cannot prove ensures contract for exit from check")),
        "{unproved:?}",
    );
    // A machine's own binder stands for its input in the definition's.
    assert_eq!(
        verdict(
            "let below<Limit: u64>(value: u64): core\\Strict<0> = value < Limit;
machine check<Bound: u64>(value: u64) -> u64 requires below::<Bound>(value) && Bound <= 10
ensures result < 10 { value }
machine build_entry() { let checked: u64 = check::<10>(3); }"
        ),
        "accepted"
    );
    // A type binder (`<T: core\Type<0>>` or `<T>`) is bound as a type
    // parameter and unfolds at its static type argument.
    assert_eq!(
        verdict(
            "let holds<T: core\\Type<0>>(value: u64): core\\Strict<0> = value < 10;
machine check(value: u64) -> u64 requires holds::<u64>(value) ensures result < 10 { value }
machine build_entry() { let checked: u64 = check(3); }"
        ),
        "accepted"
    );
    assert_eq!(
        verdict(
            "let holds<T>(value: u64): core\\Strict<0> = value < 10;
machine check(value: u64) -> u64 requires holds::<u64>(value) ensures result < 10 { value }
machine build_entry() { let checked: u64 = check(3); }"
        ),
        "accepted"
    );
    let tight = judge_build(
        "let holds<T>(value: u64): core\\Strict<0> = value < 10;
machine check(value: u64) -> u64 requires holds::<u64>(value) ensures result < 9 { value }
machine build_entry() { let checked: u64 = check(3); }",
    );
    assert!(
        matches!(
            tight,
            Ok(RouteVerdict::Incomplete(_)) | Ok(RouteVerdict::Rejected { .. })
        ),
        "too-tight guarantee over type-bound definition must not accept: {tight:?}"
    );
}

/// Adjoining address ranges share a mathematical endpoint. Its checked
/// equality remains available beneath a sum, including at a one-past bound.
#[test]
fn a_translated_endpoint_equality_preserves_the_original_bound() {
    let check = |shared_endpoint: &str| {
        judge_build(&format!(
            "machine build_entry(a: addr, x: u64, b: addr, y: u64)
requires embed(b) + embed(y) <= addr::Bound{shared_endpoint}
ensures embed(a) + (embed(x) + embed(y)) <= addr::Bound
{{}}"
        ))
        .unwrap()
    };
    for equality in ["embed(a) + embed(x) == embed(b)", "embed(b) == embed(a) + embed(x)"] {
        let RouteVerdict::Accepted(accepted) = check(&format!(", {equality}")) else {
            panic!("the original bound should follow through {equality}");
        };
        accepted.publish_invocation().unwrap();
    }
    for unrelated in ["", ", embed(a) == embed(b) + embed(x)"] {
        let verdict = check(unrelated);
        assert!(
            matches!(verdict, RouteVerdict::Incomplete(ref unproved) | RouteVerdict::Rejected { ref unproved, .. }
                if unproved.iter().any(|row| matches!(row.kind, ObligationKind::NormalGuarantee { .. }))),
            "{verdict:?}"
        );
    }
}

/// Concrete field observations establish both orders of the mathematical
/// endpoint. An endpoint one byte beyond it satisfies only one order.
#[test]
fn mathematical_equality_joins_the_two_proved_endpoint_orders() {
    let check = |base| {
        judge_build(&format!(
            "data Region {{ base: addr; length: u64; }}
machine build_entry(a: @Region, b: @Region)
requires a.base == 10, a.length == 16, b.base == {base}
ensures embed(b.base) == embed(a.base) + embed(a.length)
{{}}"
        ))
        .unwrap()
    };
    let RouteVerdict::Accepted(accepted) = check(26) else {
        panic!("the two endpoint bounds should establish equality");
    };
    accepted.publish_invocation().unwrap();
    for base in [25, 27] {
        let verdict = check(base);
        assert!(
            matches!(verdict, RouteVerdict::Incomplete(ref unproved) | RouteVerdict::Rejected { ref unproved, .. }
                if unproved.iter().any(|row| matches!(row.kind, ObligationKind::NormalGuarantee { .. }))),
            "{verdict:?}"
        );
    }
}

#[test]
fn a_nonpositive_displacement_preserves_a_shared_upper_endpoint() {
    let check = |guard| {
        judge_build(&format!(
            "machine build_entry(a: addr, x: u64) -> u64
ensures embed(a) == embed(a) + embed(result)
{{ match x == {guard} {{ true -> x, false -> 0, }} }}"
        ))
        .unwrap()
    };
    let RouteVerdict::Accepted(accepted) = check(0) else {
        panic!("each return displaces the address by zero");
    };
    accepted.publish_invocation().unwrap();
    let rejected = check(1);
    assert!(
        matches!(rejected, RouteVerdict::Incomplete(ref unproved) | RouteVerdict::Rejected { ref unproved, .. }
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::NormalGuarantee { .. }))),
        "{rejected:?}"
    );
}
