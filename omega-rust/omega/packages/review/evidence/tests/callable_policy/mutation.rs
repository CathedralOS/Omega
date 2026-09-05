use super::*;

const SOURCE: &str = r#"
pub data Pair { left: u64; right: u64; }
machine write_one(value: &mut u64) { value = 1; }
pub machine update(pair: &mut Pair) {
    transition { _ -> apply(pair) }
    state apply(current: &mut Pair) { write_one(&mut current.left); }
}
"#;

#[test]
fn private_helper_and_state_renames_preserve_entry_root_write_meaning() {
    let original = project(&Fixture::local(SOURCE));
    let renamed_source = SOURCE
        .replace("write_one", "assign_value")
        .replace("apply", "perform")
        .replace("current", "destination");
    let renamed = project(&Fixture::local(&renamed_source));
    assert_eq!(original, renamed);
    assert_eq!(
        original.canonical_bytes().unwrap(),
        renamed.canonical_bytes().unwrap()
    );
    assert!(!callable(&original, "update").mutation().paths().is_empty());

    let redirected = project(&Fixture::local(
        &SOURCE.replace("current.left", "current.right"),
    ));
    assert_eq!(
        callable(&original, "update").parameters(),
        callable(&redirected, "update").parameters()
    );
    assert_ne!(
        callable(&original, "update").mutation(),
        callable(&redirected, "update").mutation()
    );
    assert_ne!(
        original.canonical_bytes().unwrap(),
        redirected.canonical_bytes().unwrap()
    );
}
