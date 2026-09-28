use crate::tests::front_end::checked_program_result;

fn check(source: &str, accepted: bool) {
    match checked_program_result(source) {
        Ok(_) => assert!(accepted, "stale indexed bounds accepted: {source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic.is_error()),
                "expected an error: {diagnostics:#?}\n{source}"
            );
            assert!(
                diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.is_error())
                    .all(|diagnostic| diagnostic.message.contains("cannot prove subslice range")),
                "expected only subslice range errors, not earlier proof or authority failures: \
                 {diagnostics:#?}\n{source}"
            );
        }
    }
}

/// A builtin `[]` over a literal-constructed collection field owns no element
/// place: the premise's reads are exactly the evaluated initializer and
/// selector places, so writes reaching any of them retire it while disjoint
/// writes cannot.
#[test]
fn temporary_collection_indexes_keep_only_their_producing_reads() {
    for (mutation, accepted) in [
        ("", true),
        ("unrelated = 1;", true),
        ("left = replacement;", false),
        ("right = replacement;", false),
        ("index = 1;", false),
    ] {
        check(
            &format!(
                "data Pair {{ a: [i64; 4]; b: i64; }}
                 machine window(items: &[i32; 4], mut left: i64, mut right: i64,
                     mut index: u64 [0..=3], mut unrelated: i64, replacement: i64) -> u64
                 requires 0 <= (Pair {{ a: [left, 0, 0, 0], b: right }}).a[index]
                     && (Pair {{ a: [left, 0, 0, 0], b: right }}).a[index] <= 4; {{
                     {mutation}
                     let view: &[i32] = items[0..(Pair {{ a: [left, 0, 0, 0], b: right }}).a[index]];
                     view.len
                 }}"
            ),
            accepted,
        );
    }
}

#[test]
fn captured_selectors_keep_old_bounds_without_lending_them_to_later_captures() {
    for (body, selector, accepted) in [
        (
            "let selected: u64 [0..=1] = selectors[index]; selectors[index] = 1;",
            "selected",
            true,
        ),
        (
            "selectors[index] = 1; let selected: u64 [0..=1] = selectors[index];",
            "selected",
            false,
        ),
        (
            "let selected: u64 [0..=1] = selectors[index]; table[0] = replacement;",
            "selected",
            false,
        ),
        (
            "let selected: u64 [0..=1] = selectors[index]; selectors[index] = 1; let later: u64 [0..=1] = selectors[index];",
            "later",
            false,
        ),
        (
            "let selected: u64 [0..=1] = selectors[index]; let copied: u64 [0..=1] = selected; selectors[index] = 1;",
            "copied",
            true,
        ),
        (
            "let mut selected: u64 [0..=1] = selectors[index]; selected = 1;",
            "selected",
            false,
        ),
        (
            "let selected: u64 [0..=1] = selectors[index]; selectors[index] = 1; let copied: u64 [0..=1] = selected;",
            "copied",
            true,
        ),
        (
            "let selected: u64 [0..=1] = selectors[index]; table[0] = replacement; let copied: u64 [0..=1] = selected;",
            "copied",
            false,
        ),
        (
            "let mut selected: u64 [0..=1] = selectors[index]; selected = 1; let copied: u64 [0..=1] = selected;",
            "copied",
            false,
        ),
    ] {
        check(
            &format!(
                "machine window(items: &[i32; 4], table: &mut [i64; 2],
            selectors: &mut [u64 [0..=1]; 2], index: u64 [0..=1], replacement: i64) -> u64
            requires 0 <= table[selectors[index]] && table[selectors[index]] <= 4; {{
                {body}
                let view: &[i32] = items[0..table[{selector}]]; view.len
            }}"
            ),
            accepted,
        );
    }
}
