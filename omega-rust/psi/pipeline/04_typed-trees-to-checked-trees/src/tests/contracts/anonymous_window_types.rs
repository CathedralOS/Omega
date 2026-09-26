use super::super::lower_typed_trees;
use super::parse_typed_trees;
use crate::CheckingRequest;

const EXACT_SEVEN: [&str; 6] = [
    "7 / 2 * 2",
    "7 / 2.0 * 2",
    "0.1 * 70",
    "(18446744073709551615 + 7) - 18446744073709551615",
    "(18446744073709551615 * 18446744073709551615) / 18446744073709551615 - 18446744073709551608",
    "(18446744073709551615 / 2.0 * 2) - 18446744073709551608",
];

// These templates vary the collection and selectors while retaining an i32
// element destination. Unknown replacement counts still need runtime checks.
const MUTABLE_WINDOWS: [&str; 8] = [
    "machine fill(values: &mut [i32; 4]) { values[2..] = [ELEMENT, 8]; }",
    "machine fill(values: &mut [i32; 2]) { values[..] = [ELEMENT, 8]; }",
    "machine fill(values: &mut [i32]) { values[..] = [ELEMENT, 8]; }",
    "machine fill(values: &mut [i32], start: u64, end: u64)
     requires start <= end && end <= values.len;
     { values[start..end] = [ELEMENT, 8]; }",
    "machine fill(values: &mut [i32], start: u64)
     requires start <= values.len;
     { values[start..] = [ELEMENT, 8]; }",
    "machine fill<const N: u64>(values: &mut [i32; N])
     { values[..] = [ELEMENT, 8]; }",
    "machine fill(values: &mut [i32; 4], start: u64, end: u64)
     requires start <= end && end < values.len;
     { values[start..=end] = [ELEMENT, 8]; }",
    "machine fill(values: &mut [i32; 4]) {
         let mut start: u64 = 1;
         values[start..3] = [ELEMENT, 8];
     }",
];

fn accepts(source: &str) {
    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
}

fn rejects(source: &str, expected_fragments: &[&str]) {
    // Earlier syntax, resolution, or typing errors do not witness the checked
    // numeric, bounds, count, or permission contract under test.
    let typed = parse_typed_trees(source);
    let diagnostics = match lower_typed_trees(typed, &CheckingRequest::settled()) {
        Ok(_) => panic!("invalid window replacement accepted: {source}"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics.iter().any(|diagnostic| expected_fragments
            .iter()
            .all(|fragment| diagnostic.message.contains(fragment))),
        "expected diagnostic containing {expected_fragments:?}: {source}: {diagnostics:#?}"
    );
}

#[test]
fn mutable_windows_accept_exact_decimals_and_large_cancellation() {
    for template in MUTABLE_WINDOWS {
        // Establish the source form with an ordinary typed integer as well.
        accepts(&template.replace("ELEMENT", "7i32"));
        for expression in EXACT_SEVEN {
            accepts(&template.replace("ELEMENT", expression));
        }
    }
}

#[test]
fn exact_element_landing_does_not_open_unsupported_write_only_windows() {
    for (template, expected) in [
        (
            "machine fill(values: &write [i32; 4]) {
                 values[2..] = [ELEMENT, 8];
             }",
            "omitted end",
        ),
        (
            "machine fill(values: &write [i32; 4], start: u64 [0..=2]) {
                 values[start..3] = [ELEMENT, 8];
             }",
            "bounds are not statically known",
        ),
        (
            "machine fill(values: &write [i32; 4]) {
                 let mut start: u64 = 1;
                 values[start..3] = [ELEMENT, 8];
             }",
            "bounds are not statically known",
        ),
        (
            "machine fill(values: &write [i32; 4]) {
                 let start: u64 = 0 + 1;
                 values[start..3] = [ELEMENT, 8];
             }",
            "bounds are not statically known",
        ),
        (
            "machine fill(values: &write [i32]) { values[..] = [ELEMENT, 8]; }",
            "unsupported write-only projection",
        ),
        (
            "machine fill<const N: u64>(values: &write [i32; N]) {
                 values[..] = [ELEMENT, 8];
             }",
            "unsupported write-only projection",
        ),
    ] {
        for expression in ["7i32", "0.1 * 70", EXACT_SEVEN[3]] {
            rejects(&template.replace("ELEMENT", expression), &[expected]);
        }
    }
}
