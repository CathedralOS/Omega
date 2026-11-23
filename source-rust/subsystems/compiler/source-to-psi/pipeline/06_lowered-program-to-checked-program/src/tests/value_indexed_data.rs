//! Data indexed by a value or a type, read from source as a Build program.
//!
//! generics.md, "Value binders and const requirements": in
//! `data Index<Limit: u32> requires value < Limit`, `Limit` is an index of
//! the type, and "Constructing an instance owes the coupling" at the
//! argument. data_and_literals.md, "Case constraints": `data Value<T>`
//! relates its case to `T`, so a `Value<i32>` is `Value::Integer`. Each
//! twin differs from its positive in the one constructed value.

use super::build_purpose::judge_build;
use super::*;

const INDEX: &str = include_str!("../../tests/fixtures/value_indexed_data/run/index/main.omg");
const INDEX_AT_LIMIT: &str =
    include_str!("../../tests/fixtures/value_indexed_data/fail/index_at_limit/main.omg");
const CASE_TYPE: &str =
    include_str!("../../tests/fixtures/value_indexed_data/run/case_type/main.omg");
const CASE_TYPE_MISMATCH: &str =
    include_str!("../../tests/fixtures/value_indexed_data/fail/case_type_mismatch/main.omg");

fn build(source: &str) -> String {
    assert!(
        source.contains("machine main()"),
        "the fixture names its entry `main`"
    );
    source.replace("machine main()", "machine build_entry()")
}

/// `accepted`, or each unproved obligation's kind, or the refusal's text.
fn verdict(source: &str) -> String {
    match judge_build(&build(source)) {
        Ok(RouteVerdict::Accepted(accepted)) => {
            accepted.publish_invocation().unwrap();
            "accepted".to_owned()
        }
        Ok(RouteVerdict::Rejected { unproved, .. })
            if !unproved.is_empty()
                && unproved
                    .iter()
                    .all(|row| matches!(row.kind, ObligationKind::DataDomain { .. })) =>
        {
            "data domain unproved".to_owned()
        }
        Err(crate::checking::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_value_binder_is_the_index_its_construction_owes() {
    assert_eq!(verdict(INDEX), "accepted");
    assert_eq!(verdict(INDEX_AT_LIMIT), "data domain unproved");
}

/// `const Limit` asks for a static argument; the coupling is the same.
#[test]
fn a_const_binder_is_owed_the_same_way() {
    let constant = |source: &str| source.replace("Index<Limit: u32>", "Index<const Limit: u32>");
    assert_ne!(constant(INDEX), INDEX);
    assert_eq!(verdict(&constant(INDEX)), "accepted");
    assert_eq!(verdict(&constant(INDEX_AT_LIMIT)), "data domain unproved");
}

#[test]
fn a_case_constraint_selects_the_case_its_type_argument_allows() {
    assert_eq!(verdict(CASE_TYPE), "accepted");
    assert_eq!(verdict(CASE_TYPE_MISMATCH), "data domain unproved");
}
