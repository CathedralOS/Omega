use super::{lower_typed_trees, parse_typed_trees};

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
    lower_typed_trees(parse_typed_trees(source))
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
}

fn rejects(source: &str, expected_fragments: &[&str]) {
    // Earlier syntax, resolution, or typing errors do not witness the checked
    // numeric, bounds, count, or permission contract under test.
    let typed = parse_typed_trees(source);
    let diagnostics = match lower_typed_trees(typed) {
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
fn mutable_windows_reject_final_fractions_at_the_integer_element_type() {
    for template in MUTABLE_WINDOWS {
        for (expression, exact_value) in
            [("7 / 2", "7/2"), ("7 / 2.0", "7/2"), ("0.1 * 71", "71/10")]
        {
            rejects(
                &template.replace("ELEMENT", expression),
                &["not an integer", exact_value],
            );
        }
    }
}

#[test]
fn mutable_windows_reject_final_integer_overflow() {
    for template in MUTABLE_WINDOWS {
        for expression in ["2147483647 + 1", "0 - 2147483649"] {
            rejects(
                &template.replace("ELEMENT", expression),
                &["narrowing store", "i32", "not provably in range"],
            );
        }
    }
}

#[test]
fn mutable_windows_require_explicit_conversion_for_typed_float_elements() {
    for template in MUTABLE_WINDOWS {
        for expression in ["7.0f32", "7.0f64", "7.0f64 / 2 * 2", "0.1f64 * 70"] {
            rejects(&template.replace("ELEMENT", expression), &["float", "i32"]);
        }
    }
}

#[test]
fn exact_element_landing_does_not_prove_invalid_static_bounds() {
    // Impossible footprints retain their literal-width obligations, so large
    // leaves can reject before the range checker. Keep this witness in range.
    for selection in ["3..1", "3..5", "3..=4"] {
        for expression in ["7i32", "0.1 * 70"] {
            rejects(
                &format!(
                    "machine fill(values: &mut [i32; 4]) {{
                         values[{selection}] = [{expression}, 8];
                     }}"
                ),
                &["cannot prove subslice range", "slice length 4"],
            );
        }
    }
}

#[test]
fn exact_element_landing_does_not_change_static_replacement_counts() {
    for (selection, count) in [("1..2", "1"), ("1..", "3"), ("..", "4")] {
        for expression in ["0.1 * 70", EXACT_SEVEN[3]] {
            rejects(
                &format!(
                    "machine fill(values: &mut [i32; 4]) {{
                         values[{selection}] = [{expression}, 8];
                     }}"
                ),
                &[&format!("must supply exactly {count} element(s)")],
            );
        }
    }
}

#[test]
fn runtime_slice_bounds_still_require_independent_proof() {
    for expression in ["0.1 * 70", EXACT_SEVEN[3]] {
        rejects(
            &format!(
                "machine fill(values: &mut [i32], start: u64, end: u64) {{
                     values[start..end] = [{expression}, 8];
                 }}"
            ),
            &["cannot prove subslice range", "unknown slice length"],
        );
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
