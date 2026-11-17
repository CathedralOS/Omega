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
/// value of it is carried and compared like any scalar. Arithmetic on it
/// relates counts to addresses under a fit proof this route does not form,
/// and "neither arbitrary value-to-address conversions nor the inverse"
/// follow from an address, so both are refused by name.
#[test]
fn an_address_is_carried_and_compared_and_not_computed_with() {
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
