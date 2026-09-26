use super::check;

#[test]
fn routed_scalar_match_results_still_require_a_custody_join() {
    let source = "domain i64::Issued established by Issuer::issue;
        trait Issuer { machine issue(value: i64) -> i64 in Issued; }
        machine choose(flag: bool, left: i64 in Issued, right: i64 in Issued) -> i64 in Issued {
            match flag { true -> left, false -> right }
        }";
    let errors = check(source).expect_err("routed provenance is not a plain scalar join");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("branch custody join")),
        "{errors:?}"
    );
}

#[test]
fn scalar_match_result_casts_still_prove_domain_predicates() {
    let source = "domain i64::Positive requires self > 0;
        machine choose(flag: bool) -> i64 in Positive {
            match flag { true -> 0 as i64 in Positive, false -> 1 as i64 in Positive }
        }";
    let errors =
        check(source).expect_err("the false membership claim cannot become result evidence");
    assert!(
        errors.iter().any(|error| error.message.contains("Positive")
            && (error.message.contains("prove")
                || error.message.contains("predicate")
                || error.message.contains("requires"))),
        "{errors:?}"
    );
}

#[test]
fn bound_generic_operator_arms_join_scalar_peers() {
    for source in [
        "operator + Math::sum<T>(left: T, right: T) -> T;
         machine choose(flag: bool, left: u8, right: u8) -> u8 {
             match flag { true -> left + right, false -> 1 }
         }",
        "operator + Math::sum<T>(left: T, right: T) -> T;
         domain u8::Counted;
         machine choose(flag: bool, left: u8, right: u8, counted: u8 in Counted) -> u8 {
             match flag { true -> left + right, false -> counted as u8 }
         }",
    ] {
        check(source).unwrap_or_else(|errors| panic!("{source}: {errors:?}"));
    }
}

// A `-> T` result bound by the operands carries the same custody contract as
// a concrete `-> Payload`: both arms face the branch custody join. An unbound
// result parameter supplies no identity, so only the declared arm is judged.
#[test]
fn bound_generic_operator_arms_enter_the_owned_custody_join() {
    let errors = check(
        "data Payload { value: u64; }
         operator + Math::combine<T>(left: T, right: T) -> T;
         machine choose(flag: bool, left: Payload, right: Payload, spare: Payload) -> Payload {
             match flag { true -> left + right, false -> spare }
         }",
    )
    .expect_err("an owned `-> T` arm must face the same custody join");
    assert_eq!(
        errors
            .iter()
            .filter(|error| error.message.contains("branch custody join"))
            .count(),
        2,
        "{errors:?}"
    );
    let errors = check(
        "data Payload { value: u64; }
         operator + Math::pick<T>(left: Payload, right: Payload) -> T;
         machine choose(flag: bool, left: Payload, right: Payload, spare: Payload) -> Payload {
             match flag { true -> left + right, false -> spare }
         }",
    )
    .expect_err("the declared owned arm still needs the join");
    assert_eq!(
        errors
            .iter()
            .filter(|error| error.message.contains("branch custody join"))
            .count(),
        1,
        "{errors:?}"
    );
}
