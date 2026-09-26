use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;

fn accepts(source: &str) {
    lower_typed_trees(typed_program(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn rejects(source: &str, expected: &str) {
    let diagnostics = match lower_typed_trees(typed_program(source), &CheckingRequest::settled()) {
        Ok(_) => panic!("unsafe integer bitwise bounds were accepted: {source}"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected)),
        "expected {expected:?}: {source}\n{diagnostics:#?}"
    );
}

#[test]
fn complement_does_not_hide_following_exact_overflow() {
    for carrier in ["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64"] {
        rejects(
            &format!("machine run(value: {carrier}) -> {carrier} {{ (~value) + 1{carrier} }}"),
            "may overflow",
        );
    }
}

#[test]
fn literal_suffixes_keep_nested_complement_arithmetic_in_their_carrier() {
    for destination in ["u8", "u16"] {
        rejects(
            &format!("machine run() -> {destination} {{ ~(255u8 + 1u8) }}"),
            "may overflow",
        );
    }
    accepts("machine run() -> u8 [0..=0] { ~(254u8 + 1u8) }");
    accepts("machine run() -> u16 [0..=0] { (~(254u8 + 1u8)) as u16 }");
    accepts("machine run() -> u16 [255..=255] { (~0u8) as u16 }");
}

#[test]
fn nonstrict_u64_guard_does_not_prove_an_increment() {
    for (condition, selected, fallback) in [
        ("index <= limit", "index + 1u64", "index"),
        ("index > limit", "index", "index + 1u64"),
    ] {
        rejects(
            &format!(
                "machine run(index: u64, limit: u64) -> u64 {{
                    transition {condition} {{ true -> ({selected}) false -> ({fallback}) }}
                }}"
            ),
            "may overflow",
        );
    }
}
