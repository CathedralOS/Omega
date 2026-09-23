//! A construction gate reads the constructing machine's own `requires`.
//!
//! `data ... where` facts are proved AT CONSTRUCTION. A field value that is a
//! literal is a point and a place with a declared range carries that range,
//! but the ordinary shape -- a plain `i32` parameter the machine's signature
//! already constrains -- carried no interval at all, so
//! `dependent/data_where_ranged_param_constructs` and
//! `dependent/data_where_callee_establishes` could not construct from runtime
//! data. A `requires` clause holds at every call site, so the interval it
//! pins holds throughout the body, construction gate included.
//!
//! What matters as much is what this must NOT do: a `requires` clause that
//! leaves the fact undecided has to keep refusing, or the gate would be
//! widened into an assumption rather than a proof.

use validation::{OpaquePropertyValidation, validate_specialized_program};

fn gate_diagnostics(requires: &str) -> Vec<String> {
    let source = format!(
        "data Player
         where
             health >= 1,
         {{
             health: i32;
         }}
         data Main {{ champion: Player; }}
         machine Main::recruit(&mut self, strength: i32) -> i32
         {requires}
         {{
             self.champion = Player {{ health: strength }};
             self.champion.health
         }}"
    );
    let program = crate::front_end::typed_program(&source);
    match validate_specialized_program(&program, OpaquePropertyValidation::Required(&[])) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect(),
    }
}

fn refuses(requires: &str) -> bool {
    gate_diagnostics(requires)
        .iter()
        .any(|message| message.contains("cannot prove"))
}

#[test]
fn a_requires_interval_that_decides_the_fact_opens_the_gate() {
    assert!(
        !refuses("requires 1 <= strength, strength <= 100"),
        "`1 <= strength` decides `health >= 1`: {:?}",
        gate_diagnostics("requires 1 <= strength, strength <= 100")
    );
    // The same interval spelled from the other side, and with strict
    // comparisons, is the same interval.
    assert!(!refuses("requires strength >= 1, strength <= 100"));
    assert!(!refuses("requires strength > 0"));
    assert!(!refuses("requires 0 < strength"));
}

#[test]
fn a_requires_clause_that_leaves_the_fact_undecided_still_refuses() {
    // An interval exists and does not decide `health >= 1`: 0 is admitted by
    // the signature and violates the gate. Reading the clause must not be
    // mistaken for the clause deciding anything.
    assert!(refuses("requires 0 <= strength, strength <= 100"));
    // Only a ceiling: the floor is still unknown.
    assert!(refuses("requires strength <= 100"));
    // A clause about a different place says nothing about this one.
    assert!(refuses("requires 1 <= 1"));
}

#[test]
fn no_contract_at_all_still_refuses() {
    assert!(refuses(""));
}
