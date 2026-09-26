use super::{HandleSpan, ProofFact};
use crate::monomorphization::identities::contract_fact_text;

use crate::monomorphization::refresh_closed_domain_instance_identities;
use crate::tests::front_end::{checked_program_result, typed_program};

#[test]
fn membership_contract_identity_retains_normalized_indices() {
    let identity = |index: &str| {
        let program = typed_program(&format!(
            "domain<T, const I: u64> T::Coordinate<I>; machine run(value: i64 in Coordinate<{index}>) -> i64 in Coordinate<{index}> {{ value }}"
        ));
        let fact = program
            .proof_facts
            .iter()
            .find_map(|(_, fact)| matches!(fact, ProofFact::Membership(_)).then_some(fact))
            .expect("membership");
        contract_fact_text(&program, fact)
    };
    assert_ne!(identity("7"), identity("9"));
    assert_eq!(identity("7"), identity("(7 + 0)"));
}

#[test]
fn refresh_rejects_missing_index_arguments_instead_of_using_family_identity() {
    let mut program = typed_program(
        "domain<T, const I: u64> T::Coordinate<I>; machine run(value: i64 in Coordinate<7>) -> i64 in Coordinate<7> { value }",
    );
    let handle = program
        .proof_facts
        .iter()
        .find_map(|(handle, fact)| matches!(fact, ProofFact::Membership(_)).then_some(handle))
        .expect("membership");
    let ProofFact::Membership(membership) = program.proof_facts.get_mut(handle) else {
        panic!("membership");
    };
    membership.domain_arguments = HandleSpan::empty();
    assert!(refresh_closed_domain_instance_identities(&mut program).is_err());
}

#[test]
fn bound_domain_index_forwards_through_generic_calls() {
    // A call's const-position index binder (`const` or runtime `Value`) is
    // instantiated by the index the call bound it to, so a caller's own
    // `Coordinate<N>` membership discharges a callee's declared
    // `Coordinate<I>` qualification once `I` binds to `N`. This holds for
    // every binding kind: literal args, forwarded `const` binders, and
    // forwarded runtime `Value` binders.
    for source in [
        "domain<T, const I: u32> T::Coordinate<I>;
         machine relay<const I: u32>(value: i64 in Coordinate<I>) -> i64 in Coordinate<I> { value }
         machine outer<T>(v: i64 in Coordinate<7>) -> i64 in Coordinate<7> { relay<7>(v) }",
        "domain<T, const I: u32> T::Coordinate<I>;
         machine relay<const I: u32>(value: i64 in Coordinate<I>) -> i64 in Coordinate<I> { value }
         machine outer<const N: u32>(v: i64 in Coordinate<N>) -> i64 in Coordinate<N> { relay<N>(v) }",
        "domain<T, const I: u32> T::Coordinate<I>;
         machine relay<const I: u32>(value: i64 in Coordinate<I>) -> i64 in Coordinate<I> { value }
         machine outer<N: u32>(v: i64 in Coordinate<N>) -> i64 in Coordinate<N> { relay<N>(v) }",
        "domain<T, const I: u32> T::Coordinate<I>;
         machine relay<J: u32>(value: i64 in Coordinate<J>) -> i64 in Coordinate<J> { value }
         machine outer<N: u32>(v: i64 in Coordinate<N>) -> i64 in Coordinate<N> { relay<N>(v) }",
        "domain<T, const I: u32> T::Coordinate<I>;
         machine relay<const I: u32>(value: i64 in Coordinate<I>) -> i64 in Coordinate<I> { value }
         machine outer<const N: u32>(v: i64 in Coordinate<N>) -> i64 in Coordinate<N> { relay<N>(v) }
         machine main() -> i64 { let x: i64 in Coordinate<7> = 9; outer<7>(x) as i64 }",
    ] {
        checked_program_result(source).unwrap_or_else(|diagnostics| {
            panic!(
                "call-bound index forwarding should lower: {}\n{source}",
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.as_str())
                    .collect::<Vec<_>>()
                    .join(" | ")
            )
        });
    }
}

#[test]
fn bound_domain_index_rejects_a_mismatched_forwarding() {
    // Binding `I` to a different runtime binder does not establish
    // `Coordinate<N>`: the substituted index stays open and the retained call
    // still names a foreign binder the caller's scope cannot supply.
    let diagnostics = checked_program_result(
        "domain<T, const I: u32> T::Coordinate<I>;
         machine relay<J: u32>(value: i64 in Coordinate<J>) -> i64 in Coordinate<J> { value }
         machine outer<N: u32, M: u32>(v: i64 in Coordinate<N>) -> i64 in Coordinate<N> { relay<M>(v) }",
    )
        .expect_err("forwarding a different index binder must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("index")),
        "mismatched domain index rejected without an index diagnostic: {diagnostics:#?}"
    );
}
