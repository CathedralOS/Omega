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
        "contract runtime value in a mathematical term without embed"
    );
}
