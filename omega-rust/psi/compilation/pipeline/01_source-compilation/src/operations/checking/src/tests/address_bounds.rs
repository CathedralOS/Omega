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
        Err(crate::checking::CompilationRefusal::Elaboration(refusal))
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
            "public let no_wrap(base: addr, length: u64): core::Strict<0> =
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
