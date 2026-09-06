use super::*;

fn check(source: &str, accepted: bool) {
    let tokens = Lexer::new(source)
        .tokenize()
        .unwrap_or_else(|diagnostics| panic!("tokenize: {diagnostics:#?}\n{source}"));
    let syntax = parse_syntax_trees(&tokens)
        .unwrap_or_else(|diagnostics| panic!("parse: {diagnostics:#?}\n{source}"));
    let resolved = lower_syntax_trees(&syntax)
        .unwrap_or_else(|diagnostics| panic!("resolve: {diagnostics:#?}\n{source}"));
    let typed = lower_symbol_resolved_trees(&resolved)
        .unwrap_or_else(|diagnostics| panic!("type: {diagnostics:#?}\n{source}"));
    match lower_typed_trees(typed) {
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

fn indexed_source(expression: &str, body: &str, access: &str) -> String {
    format!(
        "machine set(output: &mut i64, replacement: i64) {{ output = replacement; }}
         machine inspect(value: &i64) {{}}
         machine select(output: &mut u64 [0..=1]) {{ output = 1; }}
         machine window(items: &[i32; 4], original: &mut [i64; 2],
             mut index: u64 [0..=1], replacement: i64) -> u64
         requires 0 <= {expression} && {expression} <= 4; {{
             let mut unrelated: i64 = 0;
             {body}
             let view: &[i32] = items[{access}];
             view.len
         }}"
    )
}

#[test]
fn literal_element_writes_preserve_only_disjoint_indexed_bounds() {
    for (mutation, accepted) in [
        ("", true),
        ("original[1] = replacement;", true),
        ("original[0] = replacement;", false),
        (
            "let alias: &mut i64 = &mut original[1]; alias = replacement;",
            false,
        ),
        (
            "let alias: &mut i64 = &mut original[0]; alias = replacement;",
            false,
        ),
    ] {
        check(
            &indexed_source("original[0]", mutation, "0..original[0]"),
            accepted,
        );
    }
}

#[test]
fn call_frames_preserve_only_proven_disjoint_indexed_bounds() {
    // The shared frame/origin representation currently widens projected
    // element writes to the collection root. That complete over-approximation
    // cannot prove disjointness; retaining coordinates is remaining work.
    for (body, accepted) in [
        ("set(&mut unrelated, replacement);", true),
        ("inspect(&original[0]);", true),
        ("set(&mut original[1], replacement);", false),
        ("set(&mut original[0], replacement);", false),
        (
            "let alias: &mut i64 = &mut original[1]; set(alias, replacement);",
            false,
        ),
        (
            "let alias: &mut i64 = &mut original[0]; set(alias, replacement);",
            false,
        ),
    ] {
        check(
            &indexed_source("original[0]", body, "0..original[0]"),
            accepted,
        );
    }
}

#[test]
fn dynamic_selectors_and_potentially_selected_elements_are_dependencies() {
    // The selector's declared range keeps indexing legal after every write;
    // only the bound on the resulting element may expire.
    for (mutation, accepted) in [
        ("", true),
        ("unrelated = replacement;", true),
        ("index = 1;", false),
        ("select(&mut index);", false),
        (
            "let alias: &mut u64 [0..=1] = &mut index; select(alias);",
            false,
        ),
        ("original[index] = replacement;", false),
        ("original[0] = replacement;", false),
        ("original[1] = replacement;", false),
    ] {
        check(
            &indexed_source("original[index]", mutation, "0..original[index]"),
            accepted,
        );
    }
}

#[test]
fn captures_before_element_or_selector_writes_keep_independent_bounds() {
    for (expression, mutation) in [
        ("original[0]", "original[0] = replacement;"),
        (
            "original[0]",
            "let alias: &mut i64 = &mut original[0]; set(alias, replacement);",
        ),
        ("original[index]", "index = 1;"),
    ] {
        for (body, access, accepted) in [
            (format!("let cut: i64 = {expression};"), "0..cut", true),
            (
                format!("let cut: i64 = {expression}; {mutation} let last: i64 = cut;"),
                "0..last",
                true,
            ),
            (
                format!("{mutation} let cut: i64 = {expression};"),
                "0..cut",
                false,
            ),
        ] {
            check(&indexed_source(expression, &body, access), accepted);
        }
    }
}

#[test]
fn replacing_an_array_root_or_reference_descriptor_retires_indexed_bounds() {
    // Both descriptors retain length two, so replacement cannot turn the
    // negative fixture into an unrelated out-of-bounds element access.
    for carrier in ["[i64; 2]", "&[i64; 2]"] {
        for (body, boundary, accepted) in [
            ("", "original[0]", true),
            ("original = replacement;", "original[0]", false),
            (
                "original = replacement; let cut: i64 = original[0];",
                "cut",
                false,
            ),
            (
                "let cut: i64 = original[0]; original = replacement;",
                "cut",
                true,
            ),
        ] {
            check(
                &format!(
                    "machine window(items: &[i32; 4], mut original: {carrier},
                         replacement: {carrier}) -> u64
                     requires 0 <= original[0] && original[0] <= 4; {{
                         {body}
                         let view: &[i32] = items[0..{boundary}]; view.len
                     }}"
                ),
                accepted,
            );
        }
    }
}

#[test]
fn nested_index_reads_track_selector_storage_and_selector_coordinates() {
    for (selector, mutation, accepted) in [
        ("index", "", true),
        ("index", "unrelated = 1;", true),
        ("index", "index = 1;", false),
        ("index", "selectors[index] = 1;", false),
        (
            "index",
            "let alias: &mut u64 [0..=1] = &mut selectors[index]; select(alias);",
            false,
        ),
        ("index", "table[0] = replacement;", false),
        ("0", "selectors[1] = 1;", true),
        ("0", "selectors[0] = 1;", false),
    ] {
        // Both index levels have declared bounds independent of the premise
        // about table's result. No selector write can cause an earlier error.
        let expression = format!("table[selectors[{selector}]]");
        check(
            &format!(
                "machine select(output: &mut u64 [0..=1]) {{ output = 1; }}
                 machine window(items: &[i32; 4], table: &mut [i64; 2],
                     selectors: &mut [u64 [0..=1]; 2], mut index: u64 [0..=1],
                     replacement: i64) -> u64
                 requires 0 <= {expression} && {expression} <= 4; {{
                     let mut unrelated: i64 = 0;
                     {mutation}
                     let view: &[i32] = items[0..{expression}]; view.len
                 }}"
            ),
            accepted,
        );
    }
}

#[test]
fn members_below_indexes_preserve_field_and_element_disjointness() {
    for (mutation, accepted) in [
        ("", true),
        ("cells[0].other = replacement;", true),
        ("cells[1].end = replacement;", true),
        ("cells[0].end = replacement;", false),
        (
            "let alias: &mut i64 = &mut cells[0].other; set(alias, replacement);",
            false,
        ),
        (
            "let alias: &mut i64 = &mut cells[0].end; set(alias, replacement);",
            false,
        ),
        ("cells[0] = Endpoint { end: replacement, other: 0 };", false),
    ] {
        check(
            &format!(
                "data Endpoint {{ end: i64; other: i64; }}
                 machine set(output: &mut i64, replacement: i64) {{ output = replacement; }}
                 machine window(items: &[i32; 4], cells: &mut [Endpoint; 2],
                     replacement: i64) -> u64
                 requires 0 <= cells[0].end && cells[0].end <= 4; {{
                     {mutation}
                     let view: &[i32] = items[0..cells[0].end]; view.len
                 }}"
            ),
            accepted,
        );
    }
}

#[test]
fn indexed_start_and_tail_bounds_use_the_same_dependencies() {
    for access in ["original[0]..4", "original[0].."] {
        for (mutation, accepted) in [
            ("original[1] = replacement;", true),
            ("original[0] = replacement;", false),
        ] {
            check(&indexed_source("original[0]", mutation, access), accepted);
        }
    }
}

#[test]
fn unrelated_assignment_preserves_an_indexed_endpoint_bound() {
    for mutation in ["", "unrelated = 1;"] {
        check(
            &format!(
                "machine window(items: &[i32; 4], original: &[i64; 2]) -> u64
            requires 0 <= original[0] && original[0] <= 4;
        {{
            let mut unrelated: i64 = 0;
            {mutation}
            let view: &[i32] = items[0..original[0]];
            view.len
        }}"
            ),
            true,
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

#[test]
fn parameter_selector_copy_chains_keep_the_original_indexed_bound() {
    for (mutability, mutation, accepted) in [
        ("", "", true),
        ("mut ", "index = 1;", true),
        ("", "table[0] = replacement;", false),
    ] {
        check(
            &format!(
                "machine window(items: &[i32; 4], table: &mut [i64; 2],
                {mutability}index: u64 [0..=1], replacement: i64) -> u64
                requires 0 <= table[index] && table[index] <= 4; {{
                    let selected: u64 [0..=1] = index;
                    {mutation}
                    let copied: u64 [0..=1] = selected;
                    let view: &[i32] = items[0..table[copied]]; view.len
                }}"
            ),
            accepted,
        );
    }
}
