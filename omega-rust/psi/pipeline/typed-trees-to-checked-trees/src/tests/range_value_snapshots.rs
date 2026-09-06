use super::*;

#[test]
fn captured_parameter_bound_validates_both_halves_after_source_mutation() {
    let source = r#"
        data Main { items: [i32; 4]; }
        machine Main::split(&mut self, mut original: u64) -> u64
            requires original <= 4;
        {
            let cut: u64 = original;
            let left: &mut [i32] = self.items[0..cut];
            original = 9;
            let copied: u64 = cut;
            let last: u64 = copied;
            let right: &mut [i32] = self.items[last..4];
            left.len + right.len
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    if let Err(diagnostics) = lower_typed_trees(typed) {
        panic!("captured range facts must survive: {diagnostics:#?}");
    }
}

fn fixture_result<T, E: std::fmt::Debug>(source: &str, stage: &str, result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("{stage}: {error:#?}\nsource:\n{source}"))
}

fn check(source: &str, rejection: Option<&str>) {
    let _ = checked_fixture(source, rejection);
}

fn checked_fixture(source: &str, rejection: Option<&str>) -> Option<checked_trees::CheckedTrees> {
    let tokens = fixture_result(source, "tokenize", Lexer::new(source).tokenize());
    let syntax = fixture_result(source, "parse", parse_syntax_trees(&tokens));
    let resolved = fixture_result(source, "resolve", lower_syntax_trees(&syntax));
    let typed = fixture_result(source, "type", lower_symbol_resolved_trees(&resolved));
    match lower_typed_trees(typed) {
        Ok(checked) => {
            assert!(rejection.is_none(), "expected {rejection:?}\n{source}");
            Some(checked)
        }
        Err(diagnostics) => {
            let Some(rejection) = rejection else {
                panic!("check: {diagnostics:#?}\nsource:\n{source}");
            };
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(rejection)),
                "expected {rejection}: {diagnostics:#?}\nsource:\n{source}"
            );
            if rejection == "creates local borrow" {
                assert!(
                    diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.message.contains("is still active")),
                    "expected active-loan conflict: {diagnostics:#?}\n{source}"
                );
                assert!(
                    diagnostics
                        .iter()
                        .all(|diagnostic| !diagnostic.message.contains("cannot prove")),
                    "bounds must validate before borrow rejection: {diagnostics:#?}\n{source}"
                );
            }
            None
        }
    }
}

fn window_source(scalar: &str, requirement: &str, body: &str, access: &str) -> String {
    let requirement = if requirement.is_empty() {
        String::new()
    } else {
        format!("requires {requirement};")
    };
    format!(
        "machine window(items: &[i32; 4], mut original: {scalar}) -> u64
         {requirement} {{ {body} let view: &[i32] = items[{access}]; view.len }}"
    )
}

fn snapshot_local(
    checked: &checked_trees::CheckedTrees,
    name: &str,
) -> typed_trees::statement::TableLocalData {
    checked
        .typed
        .machines()
        .iter()
        .flat_map(|machine| checked.typed.machine_states(machine))
        .flat_map(|state| {
            checked
                .typed
                .statement_table
                .statements(state.statement_nodes)
        })
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str() == name =>
            {
                Some(local.clone())
            }
            _ => None,
        })
        .expect("snapshot fixture local")
}

fn assert_captured_adjacency(checked: &mut checked_trees::CheckedTrees, reverse: bool) {
    use checked_trees::BorrowCompatibilitySelectorValue::{Integer, Symbol};

    let certificates = checked
        .facts
        .borrow
        .compatibility_certificates
        .iter()
        .map(|(_, certificate)| certificate.clone())
        .collect::<Vec<_>>();
    assert_eq!(certificates.len(), 1, "one simultaneously active loan pair");
    let certificate = &certificates[0];
    let cut = snapshot_local(checked, "cut");
    let typed_trees::expression::ExpressionNode::Name(original) =
        checked.typed.expression_table.expression(cut.initial_value)
    else {
        panic!("cut captures the nonconstant mutable parameter");
    };
    assert_ne!(cut.symbol, original.symbol);
    assert_ne!(cut.symbol, snapshot_local(checked, "last").symbol);
    let boundary = Some(Symbol(cut.symbol));
    assert_eq!(
        certificate
            .selector_snapshot
            .iter()
            .map(|row| row.value)
            .collect::<Vec<_>>(),
        if reverse {
            vec![Some(Integer(0)), boundary, boundary, Some(Integer(4))]
        } else {
            vec![boundary, Some(Integer(4)), Some(Integer(0)), boundary]
        }
    );
    assert!(certificate.conclusion.disjoint);
    assert!(certificate.conclusion.non_interfering);
    assert_eq!(
        certificate.conclusion.containment,
        checked_trees::CapturedPlaceContainment::None
    );
    assert!(
        checked
            .facts
            .borrow
            .compatibility_certificate_matches_resources(certificate)
    );
    let before = checked.facts.borrow.compatibility_certificates.clone();
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        .expect("captured range certificates replay after mutation");
    assert_eq!(checked.facts.borrow.compatibility_certificates, before);
}

#[test]
fn direct_parameter_start_end_and_tail_use_proven_numeric_bounds() {
    for (scalar, requirement) in [
        ("u64", "original <= 4"),
        ("i64", "0 <= original && original <= 4"),
    ] {
        for access in ["0..original", "..original", "original..4", "original.."] {
            check(&window_source(scalar, requirement, "", access), None);
        }
    }
}

#[test]
fn finite_copies_keep_bounds_in_both_loan_orders_and_copy_schedules() {
    for (scalar, requirement, mutation) in [
        ("u64", "original <= 4", "original = 9;"),
        ("i64", "0 <= original && original <= 4", "original = 9;"),
        ("i64", "0 <= original && original <= 4", "original = -1;"),
    ] {
        for reverse in [false, true] {
            for copy_after_formation in [false, true] {
                let copies = format!("let copied: {scalar} = cut; let last: {scalar} = copied;");
                let (first, second) = if reverse {
                    (
                        "let right: &mut [i32] = self.items[cut..4];",
                        "let left: &mut [i32] = self.items[0..last];",
                    )
                } else {
                    (
                        "let left: &mut [i32] = self.items[0..cut];",
                        "let right: &mut [i32] = self.items[last..4];",
                    )
                };
                let schedule = if copy_after_formation {
                    format!("{first} {mutation} {copies} {second}")
                } else {
                    format!("{copies} {first} {mutation} {second}")
                };
                let source = format!(
                    "data Main {{ items: [i32; 4]; }}
                     machine Main::split(&mut self, mut original: {scalar}) -> u64
                     requires {requirement}; {{
                         let cut: {scalar} = original;
                         {schedule}
                         left.len + right.len
                     }}"
                );
                let mut checked = checked_fixture(&source, None).expect("accepted snapshots");
                assert_captured_adjacency(&mut checked, reverse);
            }
        }
    }
}

#[test]
fn copies_captured_before_mutation_validate_without_an_earlier_loan() {
    for (scalar, requirement, mutation) in [
        ("u64", "original <= 4", "original = 9;"),
        ("i64", "0 <= original && original <= 4", "original = -1;"),
    ] {
        for access in ["0..last", "..last", "last..4", "last.."] {
            let body = format!(
                "let cut: {scalar} = original; let copied: {scalar} = cut;
                 let last: {scalar} = copied; {mutation}"
            );
            check(&window_source(scalar, requirement, &body, access), None);
        }
    }
}

#[test]
fn snapshots_do_not_supply_missing_signed_lower_or_numeric_upper_bounds() {
    for (scalar, requirement, rejection) in [
        ("i64", "original <= 4", "non-negative"),
        ("i64", "0 <= original", "within slice length 4"),
        ("u64", "", "within slice length 4"),
    ] {
        for copied in [false, true] {
            let body = if copied {
                format!("let cut: {scalar} = original; let last: {scalar} = cut;")
            } else {
                String::new()
            };
            let boundary = if copied { "last" } else { "original" };
            for access in [
                format!("0..{boundary}"),
                format!("{boundary}..4"),
                format!("{boundary}.."),
            ] {
                check(
                    &window_source(scalar, requirement, &body, &access),
                    Some(rejection),
                );
            }
        }
    }
}

#[test]
fn independently_bounded_endpoints_still_require_ordering() {
    for (scalar, lower) in [("u64", ""), ("i64", "0 <= start && 0 <= end &&")] {
        for (ordering, rejection) in [
            ("", Some("within slice length 4")),
            ("&& start <= end", None),
        ] {
            let source = format!(
                "machine window(items: &[i32; 4], start: {scalar}, end: {scalar}) -> u64
                 requires {lower} start <= 4 && end <= 4 {ordering};
                 {{ let view: &[i32] = items[start..end]; view.len }}"
            );
            check(&source, rejection);
        }
    }
}

#[test]
fn distinct_captures_do_not_prove_mutable_window_adjacency() {
    for reverse in [false, true] {
        for mutation in ["", "original = replacement;"] {
            let (first, second) = if reverse {
                (
                    "let right: &mut [i32] = self.items[cut..4];",
                    "let left: &mut [i32] = self.items[0..last];",
                )
            } else {
                (
                    "let left: &mut [i32] = self.items[0..cut];",
                    "let right: &mut [i32] = self.items[last..4];",
                )
            };
            let source = format!(
                "data Main {{ items: [i32; 4]; }}
                 machine Main::split(&mut self, mut original: u64, replacement: u64) -> u64
                 requires original <= 4 && replacement <= 4; {{
                     let cut: u64 = original;
                     {first} {mutation}
                     let later: u64 = original;
                     let last: u64 = later;
                     {second}
                     left.len + right.len
                 }}"
            );
            check(&source, Some("creates local borrow"));
        }
    }
}

#[test]
fn source_writes_expire_bounds_for_direct_uses_and_later_captures() {
    for (scalar, requirement, rejection) in [
        ("u64", "original <= 4", "within slice length 4"),
        (
            "i64",
            "0 <= original && original <= 4 && replacement <= 4",
            "non-negative",
        ),
    ] {
        for copied in [false, true] {
            let body = if copied {
                format!(
                    "let cut: {scalar} = original; original = replacement; let last: {scalar} = original;"
                )
            } else {
                "original = replacement;".to_owned()
            };
            let boundary = if copied { "last" } else { "original" };
            for access in [
                format!("0..{boundary}"),
                format!("{boundary}..4"),
                format!("{boundary}.."),
            ] {
                let source = format!(
                    "machine window(items: &[i32; 4], mut original: {scalar}, replacement: {scalar}) -> u64
                     requires {requirement}; {{
                         {body} let view: &[i32] = items[{access}]; view.len
                     }}"
                );
                check(&source, Some(rejection));
            }
        }
    }
}

#[test]
fn inclusive_snapshots_require_a_strict_upper_limit() {
    for (scalar, lower, mutation) in [
        ("u64", "", "original = 9;"),
        ("i64", "0 <= original &&", "original = -1;"),
    ] {
        for (comparison, inclusive, rejection) in [
            ("<=", false, None),
            ("<", false, None),
            ("<", true, None),
            ("<=", true, Some("within slice length 4")),
        ] {
            for copied in [false, true] {
                let body = if copied {
                    format!("let cut: {scalar} = original; {mutation} let last: {scalar} = cut;")
                } else {
                    String::new()
                };
                let boundary = if copied { "last" } else { "original" };
                let access = format!("0..{}{boundary}", if inclusive { "=" } else { "" });
                check(
                    &window_source(
                        scalar,
                        &format!("{lower} original {comparison} 4"),
                        &body,
                        &access,
                    ),
                    rejection,
                );
            }
        }
    }
}

#[test]
fn inclusive_normalization_overflow_never_wraps_into_a_valid_window() {
    for (access, rejection) in [
        ("0..=9223372036854775807", "inclusive end overflow"),
        ("0..9223372036854775807", "end bound"),
        ("0..=4", "end bound"),
    ] {
        check(
            &window_source("u64", "original <= 4", "", access),
            Some(rejection),
        );
    }
    check(&window_source("u64", "original <= 4", "", "0..4"), None);
    check(&window_source("u64", "original <= 4", "", "0..=3"), None);
}

#[test]
fn arithmetic_snapshot_cannot_copy_a_pre_assignment_lower_bound() {
    for before_mutation in [false, true] {
        let capture = "let cut: i64 = original - 1;";
        let mutation = "original = replacement;";
        let body = if before_mutation {
            format!("{capture} {mutation}")
        } else {
            format!("{mutation} {capture}")
        };
        let source = format!(
            "machine window(items: &[i32; 4], mut original: i64 [0..=5], replacement: i64 [0..=4]) -> u64
             requires 0 <= original - 1 && original - 1 <= 4
                 && 0 <= replacement && replacement <= 4;
             {{ {body} let view: &[i32] = items[0..cut]; view.len }}"
        );
        check(&source, (!before_mutation).then_some("cannot prove"));
    }
}
