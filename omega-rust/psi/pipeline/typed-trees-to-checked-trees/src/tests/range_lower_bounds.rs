use super::*;

fn check(source: &str, accepted: bool, rejection: &str) {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    match lower_typed_trees(typed) {
        Ok(_) => assert!(accepted, "unproved lower bound accepted: {source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(rejection)),
                "expected {rejection}: {diagnostics:#?}\n{source}"
            );
        }
    }
}

#[test]
fn slice_scalar_upper_bounds_do_not_imply_nonnegativity() {
    for (scalar, requirement, accepted) in [
        ("i64", "index < items.len", false),
        ("i64", "0 <= index && index < items.len", true),
        ("i64 [0..=4]", "index < items.len", true),
        ("u64", "index < items.len", true),
    ] {
        check(
            &format!(
                "machine read(items: &[u8], index: {scalar}) -> u8
             requires {requirement}; {{ items[index] }}"
            ),
            accepted,
            "non-negative",
        );
    }
}

#[test]
fn guarded_scalar_access_requires_a_live_lower_bound() {
    for (guard, accepted) in [
        ("index < items.len", false),
        ("0 <= index && index < items.len", true),
    ] {
        check(
            &format!(
                "machine read(items: &[u8], index: i64) -> u8 {{
                 transition {guard} {{ true -> (items[index]) false -> 0 }}
             }}"
            ),
            accepted,
            "non-negative",
        );
    }
}

#[test]
fn writes_retire_old_lower_bounds_before_a_fresh_upper_guard() {
    for mutation in ["index = -1;", "set(&mut index);"] {
        check(
            &format!(
                "machine set(output: &mut i64) {{ output = -1; }}
             machine read(items: &[u8], mut index: i64) -> u8
             requires 0 <= index && index < items.len;
             {{
                 {mutation}
                 transition index < items.len {{ true -> (items[index]) false -> 0 }}
             }}"
            ),
            false,
            "non-negative",
        );
    }
}

#[test]
fn disjoint_writes_preserve_both_index_bounds() {
    check(
        "machine set(output: &mut i64) { output = -1; }
         machine read(items: &[u8], index: i64) -> u8
         requires 0 <= index && index < items.len;
         {
             let mut other: i64 = 0;
             set(&mut other);
             items[index]
         }",
        true,
        "",
    );
}

#[test]
fn lower_bound_does_not_supply_another_collections_upper_bound() {
    check(
        "machine read(items: &[u8], other: &[u8], index: i64) -> u8
         requires 0 <= index && index < items.len;
         { other[index] }",
        false,
        "within unknown slice length",
    );
}

#[test]
fn symbolic_windows_require_nonnegative_endpoints_for_both_geometries() {
    for collection in ["[u8]", "[u8; 4]"] {
        for (access, upper) in [
            ("start..end", "start <= end && end <= items.len"),
            ("start..=end", "start <= end && end < items.len"),
            ("start..", "start <= items.len"),
            ("..end", "end <= items.len"),
        ] {
            let lower = if access == "..end" {
                "0 <= end"
            } else {
                "0 <= start"
            };
            for (scalar, extra, accepted) in [
                ("i64", "".to_owned(), false),
                ("i64", format!(" && {lower}"), true),
                ("u64", "".to_owned(), true),
                ("i64 [0..=4]", "".to_owned(), true),
            ] {
                check(
                    &format!(
                        "machine window(items: &{collection}, start: {scalar}, end: {scalar})
                     requires {upper}{extra};
                     {{ let view: &[u8] = items[{access}]; }}"
                    ),
                    accepted,
                    "non-negative",
                );
            }
        }
    }
}

#[test]
fn declared_start_lower_bound_proves_an_ordered_signed_end() {
    for collection in ["[u8]", "[u8; 4]"] {
        check(
            &format!(
                "machine window(items: &{collection}, start: i64 [0..=4], end: i64)
                 requires start <= end && end <= items.len;
                 {{ let view: &[u8] = items[start..end]; }}"
            ),
            true,
            "",
        );
    }
}

#[test]
fn current_extent_minus_a_valid_offset_proves_both_bounds() {
    for access in ["0..items.len - offset", "..items.len - offset"] {
        check(
            &format!(
                "machine window(items: &[u8], offset: u64)
            requires offset <= items.len;
            {{ let view: &[u8] = items[{access}]; }}"
            ),
            true,
            "",
        );
    }
    check(
        "machine window(items: &[u8]) requires items.len > 0;
        { let view: &[u8] = items[0..items.len - 1]; }",
        true,
        "",
    );
}

#[test]
fn length_subtraction_never_borrows_another_extent_or_an_invalid_offset() {
    for (parameters, requirement, endpoint) in [
        ("offset: u64", "", "items.len - offset"),
        (
            "offset: i64",
            "requires offset <= items.len;",
            "items.len - offset",
        ),
        ("other: &[u8]", "requires other.len > 0;", "other.len - 1"),
        (
            "offset: u64",
            "requires offset <= items.len;",
            "items.len + offset",
        ),
    ] {
        check(
            &format!(
                "machine window(items: &[u8], {parameters}) {requirement}
            {{ let view: &[u8] = items[0..{endpoint}]; }}"
            ),
            false,
            "cannot prove",
        );
    }
}

#[test]
fn overwritten_offset_cannot_reuse_its_old_extent_bound() {
    check(
        "machine window(items: &[u8], mut offset: u64)
        requires offset <= items.len;
        { offset = 100; let view: &[u8] = items[0..items.len - offset]; }",
        false,
        "cannot prove",
    );
}

#[test]
fn authored_subtraction_does_not_inherit_length_geometry() {
    check(
        "operator - u64::subtract(left: u64, right: u64) -> u64;
        machine window(items: &[u8]) requires items.len > 0;
        { let view: &[u8] = items[0..items.len - 1]; }",
        false,
        "cannot prove",
    );
}

#[test]
fn zero_offset_does_not_make_the_extent_an_inclusive_endpoint() {
    for (separator, accepted) in [("..", true), ("..=", false)] {
        check(
            &format!(
                "machine window(items: &[u8]) requires items.len > 0;
            {{ let view: &[u8] = items[0{separator}items.len - 0]; }}"
            ),
            accepted,
            "cannot prove",
        );
    }
}
