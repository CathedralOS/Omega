//! Contracts that compare values with `==` state the same comparison the body
//! evaluates (`wiki/spec/proofs/contracts.md`, "Data identity in facts"), and
//! only explicit evidence moves between that comparison and
//! `|proof|::Identity` ("Equality bridges and comparison facts").
//!
//! Each program is checked with the bundled `core/equality.omg` and
//! `core/foundation.omg` beside it, as the toolchain supplies them.

#[path = "support/source_handoff.rs"]
mod source;

use source::{Program, Verdict};

const EQUALITY: &str = include_str!("../../../../../../../source/library/core/equality.omg");
const FOUNDATION: &str = include_str!("../../../../../../../source/library/core/foundation.omg");

fn checked(main: &str) -> Result<Verdict, source::EarlierRefusal> {
    Program::try_of(&[
        ("core/equality.omg", EQUALITY),
        ("core/foundation.omg", FOUNDATION),
        ("product/main.omg", main),
    ])
    .map(|program| program.judged(|_| {}))
}

#[track_caller]
fn accepted(main: &str) {
    let verdict = checked(main);
    assert!(matches!(verdict, Ok(Verdict::Accepted)), "{verdict:?}");
}

#[track_caller]
fn call_requirement_unproved(main: &str) {
    let verdict = checked(main);
    assert!(
        format!("{verdict:?}").contains("unproved: [\"CallRequirement"),
        "{verdict:?}"
    );
}

#[track_caller]
fn refused_before_checking(main: &str, message: &str) {
    match checked(main) {
        Err(refusal) => assert!(refusal.message.contains(message), "{refusal:?}"),
        other => panic!("expected a refusal naming {message:?}, got {other:?}"),
    }
}

/// `Coin`'s selected comparison is a user row that compares one field, and
/// promises what a clause needs of an operation: no service, no crash, and
/// that it returns.
const COIN: &str = "data Coin { value: i32; year: i32; }
CoinByValue satisfies Equatable<Coin> {
    machine equals(left: @Coin, right: @Coin) -> bool reaches none crashes none terminates {
        left.value == right.value
    }
}
";

fn coin(program: &str) -> String {
    format!("{COIN}{program}")
}

#[test]
fn a_user_comparison_in_a_requirement_is_owed_by_the_same_comparison() {
    accepted(&coin(
        "machine needs(a: Coin, b: Coin) requires a == b {}
machine forward(a: Coin, b: Coin) requires a == b { needs(a, b); }
",
    ));
    accepted(&coin(
        "machine needs(a: Coin, b: Coin) requires a != b {}
machine forward(a: Coin, b: Coin) requires a != b { needs(a, b); }
",
    ));
}

#[test]
fn the_operator_and_the_selected_row_name_one_comparison() {
    accepted(&coin(
        "machine needs(a: Coin, b: Coin) requires CoinByValue::equals(@a, @b) {}
machine forward(a: Coin, b: Coin) requires a == b { needs(a, b); }
",
    ));
}

#[test]
fn a_comparison_of_shared_referents_is_owed_through_the_same_loans() {
    accepted(&coin(
        "machine needs(a: @Coin, b: @Coin) requires a == b {}
machine forward(a: @Coin, b: @Coin) requires a == b { needs(a, b); }
",
    ));
}

#[test]
fn a_comparison_of_other_values_does_not_prove_the_call() {
    call_requirement_unproved(&coin(
        "machine needs(a: Coin, b: Coin) requires a == b {}
machine forward(a: Coin, b: Coin, c: Coin) requires a == b { needs(a, c); }
",
    ));
    // A user comparison need not be symmetric.
    call_requirement_unproved(&coin(
        "machine needs(a: Coin, b: Coin) requires a == b {}
machine forward(a: Coin, b: Coin) requires a == b { needs(b, a); }
",
    ));
}

#[test]
fn equal_and_unequal_are_different_claims() {
    call_requirement_unproved(&coin(
        "machine needs(a: Coin, b: Coin) requires a != b {}
machine forward(a: Coin, b: Coin) requires a == b { needs(a, b); }
",
    ));
    call_requirement_unproved(&coin(
        "machine needs(a: Coin, b: Coin) requires a == b {}
machine forward(a: Coin, b: Coin) requires a != b { needs(a, b); }
",
    ));
}

#[test]
fn a_user_comparison_is_not_read_as_its_fields() {
    // The body compares `value`, but a clause sees only the selected
    // operation: the field fact supplies no comparison result.
    call_requirement_unproved(&coin(
        "machine needs(a: Coin, b: Coin) requires a == b {}
machine forward(a: Coin, b: Coin) requires a.value == b.value { needs(a, b); }
",
    ));
}

#[test]
fn another_implementation_for_the_same_type_is_another_operation() {
    let two = "data Coin { value: i32; year: i32; }
CoinByValue satisfies Equatable<Coin> {
    machine equals(left: @Coin, right: @Coin) -> bool reaches none crashes none terminates {
        left.value == right.value
    }
}
CoinByYear satisfies Equatable<Coin> {
    machine equals(left: @Coin, right: @Coin) -> bool reaches none crashes none terminates {
        left.year == right.year
    }
}
";
    accepted(&format!(
        "{two}machine needs(a: Coin, b: Coin) requires CoinByYear::equals(@a, @b) {{}}
machine forward(a: Coin, b: Coin) requires CoinByYear::equals(@a, @b) {{ needs(a, b); }}
"
    ));
    call_requirement_unproved(&format!(
        "{two}machine needs(a: Coin, b: Coin) requires CoinByYear::equals(@a, @b) {{}}
machine forward(a: Coin, b: Coin) requires CoinByValue::equals(@a, @b) {{ needs(a, b); }}
"
    ));
}

#[test]
fn a_comparison_that_may_not_return_is_not_a_term() {
    // `Equatable::equals` promises nothing, so the row must: proof use
    // keeps the selected invocation's termination and effect rules.
    refused_before_checking(
        "data Coin { value: i32; year: i32; }
CoinByValue satisfies Equatable<Coin> {
    machine equals(left: @Coin, right: @Coin) -> bool { left.value == right.value }
}
machine needs(a: Coin, b: Coin) requires a == b {}
",
        "clause application needs an unconditional pure and total operation",
    );
}

#[test]
fn a_mutable_loan_names_no_referent_a_clause_can_keep() {
    // The machine may write through `a` before the call; a fact about the
    // referent at entry would then describe a value the call never sees.
    refused_before_checking(
        &coin(
            "machine needs(a: @Coin, b: @Coin) requires a == b {}
machine stale(a: @mutable Coin, b: @Coin) requires $a == $b { a.value = 3i32; needs(a, b); }
",
        ),
        "Identity operand has no exact input observation",
    );
    refused_before_checking(
        "use {omega}/language/core/foundation.omg as proof;
data P { x: i32; y: i32; }
machine needs(a: @P, b: @P) requires |proof|::Identity::<P>($a, $b) {}
machine stale(a: @mutable P, b: @P) requires |proof|::Identity::<P>($a, $b) { a.x = 3i32; needs(a, b); }
",
        "Identity operand has no exact input observation",
    );
}

#[test]
fn a_precondition_proves_the_same_comparison_the_body_returns() {
    accepted(&coin(
        "machine same(a: Coin, b: Coin) -> bool requires a == b ensures result { a == b }\n",
    ));
    accepted(&coin(
        "machine same(a: @Coin, b: @Coin) -> bool requires a == b ensures result { a == b }\n",
    ));
    accepted(&coin(
        "machine differ(a: Coin, b: Coin) -> bool requires a != b ensures !result { a == b }\n",
    ));
}

#[test]
fn the_body_returns_the_operation_without_a_precondition() {
    accepted(&coin(
        "machine same(a: Coin, b: Coin) -> bool ensures result == (a == b) { a == b }\n",
    ));
}

#[test]
fn congruence_needs_the_same_values() {
    call_requirement_or_ensures_unproved(&coin(
        "machine same(a: Coin, b: Coin, c: Coin) -> bool requires a == b ensures result { a == c }\n",
    ));
    call_requirement_or_ensures_unproved(&coin(
        "machine same(a: Coin, b: Coin) -> bool requires a == b ensures result { b == a }\n",
    ));
}

#[test]
fn congruence_needs_the_same_operation() {
    let two = "data Coin { value: i32; year: i32; }
CoinByValue satisfies Equatable<Coin> {
    machine equals(left: @Coin, right: @Coin) -> bool reaches none crashes none terminates {
        left.value == right.value
    }
}
CoinByYear satisfies Equatable<Coin> {
    machine equals(left: @Coin, right: @Coin) -> bool reaches none crashes none terminates {
        left.year == right.year
    }
}
";
    call_requirement_or_ensures_unproved(&format!(
        "{two}machine same(a: Coin, b: Coin) -> bool requires CoinByValue::equals(@a, @b) ensures result {{ CoinByYear::equals(@a, @b) }}\n"
    ));
    accepted(&format!(
        "{two}machine same(a: Coin, b: Coin) -> bool requires CoinByYear::equals(@a, @b) ensures result {{ CoinByYear::equals(@a, @b) }}\n"
    ));
}

#[track_caller]
fn call_requirement_or_ensures_unproved(main: &str) {
    let verdict = checked(main);
    let text = format!("{verdict:?}");
    assert!(
        text.contains("unproved: [\"CallRequirement")
            || text.contains("unproved: [\"NormalGuarantee")
            || text.contains("cannot prove ensures"),
        "{verdict:?}"
    );
}

#[test]
fn a_written_input_is_not_the_value_the_precondition_compared() {
    // `a` is written before the comparison, so its result is about another
    // value than the one `requires` compared.
    call_requirement_or_ensures_unproved(&coin(
        "machine same(mutable a: Coin, b: Coin) -> bool requires a == b ensures result { a.value = 3i32; a == b }\n",
    ));
}

#[test]
fn the_returned_comparison_is_the_operation_on_the_same_values() {
    call_requirement_or_ensures_unproved(&coin(
        "machine same(a: Coin, b: Coin) -> bool ensures result == (a == b) { b == a }\n",
    ));
}

/// Raw `Identity<f32>` and `Identity<f64>` compare exact same-format bits
/// (`wiki/spec/terminal-psi/mathematical_values.md`, "Float equality and
/// source identity"): not IEEE `==`, and not the payload-erased meaning.
const PROOF: &str = "use {omega}/language/core/foundation.omg as proof;\n";

fn proof(program: &str) -> String {
    format!("{PROOF}{program}")
}

#[test]
fn raw_float_identity_is_reflexive_and_follows_exact_copies() {
    for program in [
        "machine same(x: f64) ensures |proof|::Identity::<f64>(x, x) {}",
        "machine flip(x: f64, y: f64) requires |proof|::Identity::<f64>(x, y) ensures |proof|::Identity::<f64>(y, x) {}",
        "machine flip(x: f32, y: f32) requires |proof|::Identity::<f32>(x, y) ensures |proof|::Identity::<f32>(y, x) {}",
        "machine copy(x: f64) -> f64 ensures |proof|::Identity::<f64>(result, x) { x }",
        "machine pass(x: f64, y: f64) -> f64 requires |proof|::Identity::<f64>(x, y) ensures |proof|::Identity::<f64>(result, y) { x }",
        "machine zero() -> f64 ensures |proof|::Identity::<f64>(result, 0.0f64) { 0.0f64 }",
        "machine zero() ensures |proof|::Identity::<f64>(0.0f64, 0.0f64) {}",
    ] {
        accepted(&proof(program));
    }
}

#[test]
fn signed_zeros_are_not_identical() {
    for program in [
        "machine zeros() ensures |proof|::Identity::<f64>(0.0f64, -0.0f64) {}",
        "machine zero() -> f64 ensures |proof|::Identity::<f64>(result, -0.0f64) { 0.0f64 }",
    ] {
        call_requirement_or_ensures_unproved(&proof(program));
    }
}

#[test]
fn a_record_is_identical_when_its_float_field_is() {
    accepted(&proof(
        "data P { x: f64; n: i32; }
machine same(a: P, b: P)
requires |proof|::Identity::<f64>(a.x, b.x)
requires |proof|::Identity::<i32>(a.n, b.n)
ensures |proof|::Identity::<P>(a, b) {}
",
    ));
    call_requirement_or_ensures_unproved(&proof(
        "data P { x: f64; n: i32; }
machine same(a: P, b: P)
requires |proof|::Identity::<i32>(a.n, b.n)
ensures |proof|::Identity::<P>(a, b) {}
",
    ));
}

#[test]
fn ieee_equality_and_raw_identity_do_not_stand_for_each_other() {
    // IEEE `==` relates signed zeros and never a NaN to itself, so neither
    // direction holds; a clause cannot state one to prove the other.
    refused_before_checking(
        &proof(
            "machine from_ieee(x: f64, y: f64) requires x == y ensures |proof|::Identity::<f64>(x, y) {}",
        ),
        "contract IEEE comparison without its selected declaration",
    );
    refused_before_checking(
        &proof(
            "machine to_ieee(x: f64, y: f64) requires |proof|::Identity::<f64>(x, y) ensures x == y {}",
        ),
        "contract IEEE comparison without its selected declaration",
    );
}

/// The unproved obligations of a program, in the checker's words.
fn unproved(main: &str) -> Vec<String> {
    match checked(main) {
        Ok(Verdict::Rejected(rejection)) => rejection.unproved,
        Ok(Verdict::Accepted) => Vec::new(),
        other => panic!("expected a judged program, got {other:?}"),
    }
}

/// `Coin`'s comparison of both fields, with its comparison-implies-identity
/// law. The law's body proves nothing yet, so its own obligation stays owed;
/// what a caller may do with it is checked apart from that.
const BOTH: &str = "use {omega}/language/core/foundation.omg as proof;
data Coin { value: i32; year: i32; }
CoinByBoth satisfies EqualImpliesIdentity<Coin> {
    machine equals(left: @Coin, right: @Coin) -> bool reaches none crashes none terminates {
        left.value == right.value && left.year == right.year
    }
    machine identity_of_equal(left: Coin, right: Coin)
    requires equals(@left, @right)
    ensures |proof|::Identity::<Coin>(left, right) {}
}
";

#[test]
fn a_law_over_the_inherited_comparison_is_owed_by_its_body() {
    let owed = unproved(BOTH);
    assert!(
        matches!(owed.as_slice(), [only] if only.starts_with("NormalGuarantee")),
        "{owed:?}"
    );
}

#[test]
fn a_cited_law_proves_identity_from_the_same_comparison() {
    let caller = "machine same(a: Coin, b: Coin)
requires CoinByBoth::equals(@a, @b)
ensures |proof|::Identity::<Coin>(a, b) {
    CoinByBoth::identity_of_equal(a, b);
}
";
    assert_eq!(unproved(&format!("{BOTH}{caller}")), unproved(BOTH));
}

/// A caller of `BOTH`'s law whose citation is refused: it owes more than the
/// law's own body does, and what it owes is the law's premise.
#[track_caller]
fn premise_owed(caller: &str) {
    let alone = unproved(BOTH);
    let owed = unproved(&format!("{BOTH}{caller}"));
    assert!(
        owed.len() > alone.len() && owed.iter().any(|row| row.starts_with("CallRequirement")),
        "{owed:?}"
    );
}

#[test]
fn a_cited_law_owes_its_comparison_at_the_use() {
    premise_owed(&format!(
        "machine same(a: Coin, b: Coin)
ensures |proof|::Identity::<Coin>(a, b) {{
    CoinByBoth::identity_of_equal(a, b);
}}
"
    ));
}

#[test]
fn a_law_about_both_fields_says_nothing_of_a_comparison_of_one() {
    premise_owed(&format!(
        "CoinByValue satisfies Equatable<Coin> {{
    machine equals(left: @Coin, right: @Coin) -> bool reaches none crashes none terminates {{
        left.value == right.value
    }}
}}
machine same(a: Coin, b: Coin)
requires CoinByValue::equals(@a, @b)
ensures |proof|::Identity::<Coin>(a, b) {{
    CoinByBoth::identity_of_equal(a, b);
}}
"
    ));
}

#[test]
fn evidence_needs_a_comparison_a_clause_can_name() {
    refused_before_checking(
        "use {omega}/language/core/foundation.omg as proof;
data Coin { value: i32; }
CoinByValue satisfies IdentityImpliesEqual<Coin> {
    machine equals(left: @Coin, right: @Coin) -> bool { left.value == right.value }
    machine equal_of_identity(left: Coin, right: Coin)
    requires |proof|::Identity::<Coin>(left, right)
    ensures equals(@left, @right) {}
}
",
        "clause application needs an unconditional pure and total operation",
    );
}

#[test]
fn only_equality_evidence_names_an_abstract_comparison() {
    refused_before_checking(
        "trait Law<T> {
    machine same(left: @T, right: @T) -> bool;
    machine reflexive(value: T) ensures same(@value, @value);
}
",
        "clause application needs an unconditional pure and total operation",
    );
}

/// A comparison that holds of every two `Flag`s, written so that its own
/// contract says so, and its identity-implies-comparison law.
fn always(contract: &str, body: &str, law: &str) -> String {
    format!(
        "use {{omega}}/language/core/foundation.omg as proof;
data Flag {{ set: bool; }}
FlagAlways satisfies {law}<Flag> {{
    machine equals(left: @Flag, right: @Flag) -> bool reaches none crashes none terminates
    {contract} {{ {body} }}
{}
}}
",
        if law == "IdentityImpliesEqual" {
            "    machine equal_of_identity(left: Flag, right: Flag)
    requires |proof|::Identity::<Flag>(left, right)
    ensures equals(@left, @right) {
        let same: bool = equals(@left, @right);
    }"
        } else {
            "    machine identity_of_equal(left: Flag, right: Flag)
    requires equals(@left, @right)
    ensures |proof|::Identity::<Flag>(left, right) {
        let same: bool = equals(@left, @right);
    }"
        }
    )
}

#[test]
fn a_law_uses_what_its_comparison_ensures_of_itself() {
    accepted(&always("ensures result", "true", "IdentityImpliesEqual"));
}

#[test]
fn a_law_cannot_read_its_comparison_body() {
    let owed = unproved(&always("", "true", "IdentityImpliesEqual"));
    assert!(!owed.is_empty(), "{owed:?}");
}

#[test]
fn the_comparison_still_owes_what_it_ensures_of_itself() {
    let owed = unproved(&always(
        "ensures result",
        "left.set == right.set",
        "IdentityImpliesEqual",
    ));
    assert!(!owed.is_empty(), "{owed:?}");
}

#[test]
fn a_comparison_that_always_holds_justifies_no_substitution() {
    let owed = unproved(&always("ensures result", "true", "EqualImpliesIdentity"));
    assert!(!owed.is_empty(), "{owed:?}");
}

#[test]
fn only_its_implementation_sees_what_a_member_ensures_of_itself() {
    let law = always("ensures result", "true", "IdentityImpliesEqual");
    let owed = unproved(&format!(
        "{law}machine both(a: Flag, b: Flag) -> bool ensures result {{
    FlagAlways::equals(@a, @b)
}}
"
    ));
    assert!(!owed.is_empty(), "{owed:?}");
}

#[test]
fn the_operator_selects_the_comparison_a_law_is_about() {
    let caller = "machine same(a: Coin, b: Coin)
requires a == b
ensures |proof|::Identity::<Coin>(a, b) {
    CoinByBoth::identity_of_equal(a, b);
}
";
    assert_eq!(unproved(&format!("{BOTH}{caller}")), unproved(BOTH));
}

#[test]
fn an_evidence_implementation_is_the_type_s_one_comparison() {
    match checked(&format!(
        "{BOTH}CoinByValue satisfies Equatable<Coin> {{
    machine equals(left: @Coin, right: @Coin) -> bool reaches none crashes none terminates {{
        left.value == right.value
    }}
}}
machine same(a: Coin, b: Coin) -> bool {{ a == b }}
"
    )) {
        Err(refusal) => assert!(
            refusal
                .message
                .contains("multiple Equatable implementations"),
            "{refusal:?}"
        ),
        other => panic!("expected the operator to find two comparisons, got {other:?}"),
    }
}

/// `same`, generic over its comparison evidence, cites the law through its
/// implementation parameter. Its copy for `CoinByBoth` reads `E::equals` and
/// `E::identity_of_equal` as that implementation's rows.
const THROUGH_E: &str = "machine same<T, E satisfies EqualImpliesIdentity<T>>(a: T, b: T)
requires E::equals(@a, @b)
ensures |proof|::Identity::<T>(a, b) {
    E::identity_of_equal(a, b);
}
";

#[test]
fn a_law_cited_through_an_implementation_parameter_proves_identity() {
    let caller = "machine use_it(a: Coin, b: Coin)
requires CoinByBoth::equals(@a, @b)
ensures |proof|::Identity::<Coin>(a, b) {
    same::<Coin, CoinByBoth>(a, b);
}
";
    assert_eq!(
        unproved(&format!("{BOTH}{THROUGH_E}{caller}")),
        unproved(BOTH)
    );
}

#[test]
fn a_law_cited_through_an_implementation_parameter_owes_its_comparison() {
    premise_owed(&format!(
        "{THROUGH_E}machine use_it(a: Coin, b: Coin)
ensures |proof|::Identity::<Coin>(a, b) {{
    same::<Coin, CoinByBoth>(a, b);
}}
"
    ));
}

/// The premise is the selected implementation's own comparison: another
/// implementation's comparison of the same values does not discharge it.
#[test]
fn a_law_cited_through_an_implementation_parameter_owes_that_implementation_s_comparison() {
    premise_owed(&format!(
        "{THROUGH_E}CoinByValue satisfies Equatable<Coin> {{
    machine equals(left: @Coin, right: @Coin) -> bool reaches none crashes none terminates {{
        left.value == right.value
    }}
}}
machine use_it(a: Coin, b: Coin)
requires CoinByValue::equals(@a, @b)
ensures |proof|::Identity::<Coin>(a, b) {{
    same::<Coin, CoinByBoth>(a, b);
}}
"
    ));
}

/// An implementation parameter bounded at `T` is given an implementation of
/// that trait at the type supplied for `T`, and no other.
#[test]
fn an_implementation_parameter_takes_its_trait_at_the_supplied_type() {
    let caller = "data Other { value: i32; }
machine use_it(a: Other, b: Other) {
    same::<Other, CoinByBoth>(a, b);
}
";
    let verdict = checked(&format!("{BOTH}{THROUGH_E}{caller}"));
    match verdict {
        Err(refusal) => assert!(
            refusal.message.contains("match its binder's exact trait"),
            "{refusal:?}"
        ),
        other => panic!("expected the call to be refused, got {other:?}"),
    }
}

/// `data` and a comparison over its fields that ensures `result ==
/// (compared)`, with the law of `kind`. The law's body calls the comparison,
/// so the comparison's own guarantee is known of that call.
fn field_law(data: &str, compared: &str, kind: &str) -> String {
    let law = if kind == "IdentityImpliesEqual" {
        "machine equal_of_identity(left: Coin, right: Coin)
    requires |proof|::Identity::<Coin>(left, right)
    ensures equals(@left, @right) {
        let same: bool = equals(@left, @right);
    }"
    } else {
        "machine identity_of_equal(left: Coin, right: Coin)
    requires equals(@left, @right)
    ensures |proof|::Identity::<Coin>(left, right) {
        let same: bool = equals(@left, @right);
    }"
    };
    format!(
        "use {{omega}}/language/core/foundation.omg as proof;
data Coin {{ {data} }}
CoinByValue satisfies {kind}<Coin> {{
    machine equals(left: @Coin, right: @Coin) -> bool reaches none crashes none terminates
    ensures result == ({compared}) {{
        {compared}
    }}
    {law}
}}
"
    )
}

/// contracts.md, "Equality bridges and comparison facts": "a proof that
/// builtin `x == y` is true establishes `|proof|::Identity::<T>(x, y)`."
#[test]
fn a_true_field_comparison_proves_the_record_s_identity() {
    accepted(&field_law(
        "value: i32;",
        "left.value == right.value",
        "EqualImpliesIdentity",
    ));
    accepted(&field_law(
        "value: u8;",
        "left.value == right.value",
        "EqualImpliesIdentity",
    ));
}

/// "Conversely, that identity establishes that the same comparison is true."
#[test]
fn the_record_s_identity_proves_its_field_comparison_true() {
    accepted(&field_law(
        "value: i32;",
        "left.value == right.value",
        "IdentityImpliesEqual",
    ));
}

#[test]
fn a_comparison_of_some_fields_proves_no_identity() {
    let owed = unproved(&field_law(
        "value: i32; year: i32;",
        "left.value == right.value",
        "EqualImpliesIdentity",
    ));
    assert!(!owed.is_empty(), "{owed:?}");
}

#[test]
fn another_integer_relation_proves_no_identity() {
    for compared in [
        "left.value != right.value",
        "left.value <= right.value",
        "left.value == left.value",
    ] {
        let owed = unproved(&field_law("value: i32;", compared, "EqualImpliesIdentity"));
        assert!(!owed.is_empty(), "{compared}: {owed:?}");
    }
}

/// IEEE `==` is not raw identity: `0.0 == -0.0` and `NaN != NaN`.
#[test]
fn an_ieee_field_comparison_proves_no_identity() {
    let verdict = checked(&field_law(
        "value: f64;",
        "left.value == right.value",
        "EqualImpliesIdentity",
    ));
    assert!(!matches!(verdict, Ok(Verdict::Accepted)), "{verdict:?}");
}
