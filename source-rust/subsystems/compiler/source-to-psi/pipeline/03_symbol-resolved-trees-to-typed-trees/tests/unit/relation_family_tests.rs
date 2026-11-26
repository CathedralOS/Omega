//! A trait's relation family `R: C -> C -> |proof|::Type` types only when it is a
//! family of types, which is how the kernel declares every relation.

#[test]
fn a_relation_family_ends_in_core_type() {
    let family = |result: &str| {
        super::qualified_operator_home_tests::packages(
            &[
                (
                    0,
                    "foundation.omg",
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../../../../../source/library/core/foundation.omg"
                    )),
                ),
                (
                    31,
                    "relation.omg",
                    &format!(
                        "use {{omega}}/language/core/foundation.omg as proof;
                 trait Reflexive<C, R: C -> C -> {result}> {{
                     machine reflexive(value: C) ensures R(value, value);
                 }}"
                    ),
                ),
            ],
            &[],
        )
    };
    family("|proof|::Type").expect("a family of types is a relation");
    for (result, refusal) in [
        (
            "C",
            "must end in the foundation type `Type`, a family of types",
        ),
        ("|proof|::Type<1>", "with its universe level elided"),
    ] {
        let error = family(result).expect_err("only a family of types is a relation");
        assert!(error.contains(refusal), "{result}: {error}");
    }
}
