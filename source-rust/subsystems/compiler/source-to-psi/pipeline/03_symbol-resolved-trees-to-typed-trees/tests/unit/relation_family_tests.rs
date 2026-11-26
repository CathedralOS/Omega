//! A trait's relation family `R: C -> C -> core\Type` types only when it is a
//! family of types, which is how the kernel declares every relation.

#[test]
fn a_relation_family_ends_in_core_type() {
    let family = |result: &str| {
        super::qualified_operator_home_tests::checked(&[(
            "relation.omg",
            &format!(
                "trait Reflexive<C, R: C -> C -> {result}> {{
                     machine reflexive(value: C) ensures R(value, value);
                 }}"
            ),
        )])
    };
    family("core\\Type").expect("a family of types is a relation");
    for (result, refusal) in [
        ("C", "must end in `core\\Type`, a family of types"),
        ("core\\Type<1>", "with its universe level elided"),
    ] {
        let error = family(result).expect_err("only a family of types is a relation");
        assert!(error.contains(refusal), "{result}: {error}");
    }
}
