//! Cast predicates may weaken at a result join; arithmetic policy may not.

fn diagnostics(source: &str) -> Vec<String> {
    let typed = crate::front_end::typed_program(source);
    validation::validate_program(&typed)
        .err()
        .unwrap_or_default()
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

#[test]
fn anonymous_match_peer_does_not_inherit_another_arms_range_predicate() {
    for arms in [
        "true -> 1 as u64 [0..=10] in Wrapping, false -> 11",
        "true -> 11, false -> 1 as u64 [0..=10] in Wrapping",
        "true -> 1 as u64 [0..=10] in Wrapping, false -> 11 as u64 [0..=20] in Wrapping",
        "true -> 11 as u64 [0..=20] in Wrapping, false -> 1 as u64 [0..=10] in Wrapping",
    ] {
        let source =
            format!("machine run(flag: bool) -> u64 {{ (match flag {{ {arms} }}) as u64 }}");
        let errors = diagnostics(&source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
    }
}
