//! Immutable boundary copies retain their value when the original changes.

use super::sole_certificate;
use checked_trees::BorrowCompatibilitySelectorValue;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TableLocalData};

fn fixture_result<T, E: std::fmt::Debug>(source: &str, stage: &str, result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| {
        let compact_source = source.split_whitespace().collect::<Vec<_>>().join(" ");
        panic!("{stage} failed: {error:#?}\nsource: {compact_source}");
    })
}

fn typed_source(source: &str) -> typed_trees::TypedTrees {
    let tokens = fixture_result(
        source,
        "tokenize",
        source_files_to_tokens::Lexer::new(source).tokenize(),
    );
    let syntax = fixture_result(
        source,
        "parse",
        tokens_to_syntax_trees::parse_syntax_trees(&tokens),
    );
    let resolved = fixture_result(
        source,
        "resolve",
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax),
    );
    fixture_result(
        source,
        "type",
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved),
    )
}

fn checked_source(source: &str) -> checked_trees::CheckedTrees {
    fixture_result(
        source,
        "check snapshot fixture",
        crate::lower_typed_trees(typed_source(source)),
    )
}

fn split_source(parameter: bool, body: &str) -> String {
    let signature = if parameter {
        "machine Main::split(&mut self, mut original: u64) -> u64 requires original == 2 && original <= 4;"
    } else {
        "machine Main::split(&mut self) -> u64"
    };
    let original = if parameter {
        // Establish range facts by assignment so these identity fixtures do not
        // depend on precondition range propagation. The standalone direct-
        // parameter regression still exercises the precondition route.
        "original = 2;"
    } else {
        "let mut original: u64 = 2;"
    };
    format!(
        "data Main {{ items: [i32; 4]; }}
         {signature} {{ {original} {body} left.len + right.len }}"
    )
}

fn local(checked: &checked_trees::CheckedTrees, name: &str) -> TableLocalData {
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
            StatementNode::LocalData(local) if local.name.as_str() == name => Some(local.clone()),
            _ => None,
        })
        .expect("fixture local")
}

fn change_local(
    checked: &mut checked_trees::CheckedTrees,
    name: &str,
    mut change: impl FnMut(&mut TableLocalData),
) {
    let spans = checked
        .typed
        .machines()
        .iter()
        .flat_map(|machine| checked.typed.machine_states(machine))
        .map(|state| state.statement_nodes)
        .collect::<Vec<_>>();
    let mut changed = 0;
    for span in spans {
        for statement in checked.typed.statement_table.statements_mut(span) {
            if let StatementNode::LocalData(local) = statement
                && local.name.as_str() == name
            {
                change(local);
                changed += 1;
            }
        }
    }
    assert_eq!(changed, 1, "change one exact fixture declaration");
}

fn assert_snapshot(checked: &checked_trees::CheckedTrees, reverse: bool) {
    let certificate = sole_certificate(checked);
    let cut = local(checked, "cut");
    let ExpressionNode::Name(original) =
        checked.typed.expression_table.expression(cut.initial_value)
    else {
        panic!("snapshot initializer must name its mutable source");
    };
    assert!(cut.symbol.is_valid());
    assert!(original.symbol.is_valid());
    assert_ne!(cut.symbol, original.symbol);
    let boundary = Some(BorrowCompatibilitySelectorValue::Symbol(cut.symbol));
    let zero = Some(BorrowCompatibilitySelectorValue::Integer(0));
    let four = Some(BorrowCompatibilitySelectorValue::Integer(4));
    assert_eq!(
        certificate
            .selector_snapshot
            .iter()
            .map(|row| row.value)
            .collect::<Vec<_>>(),
        if reverse {
            vec![zero, boundary, boundary, four]
        } else {
            vec![boundary, four, zero, boundary]
        },
        "both loans retain Symbol(cut), including through copies and source mutation"
    );
    assert!(certificate.conclusion.disjoint);
    assert!(certificate.conclusion.non_interfering);
    assert_eq!(
        certificate.conclusion.containment,
        checked_trees::CapturedPlaceContainment::None
    );
    assert_eq!(
        certificate.derivation,
        checked_trees::BorrowCompatibilityDerivation::Structural
    );
    assert!(
        checked
            .facts
            .borrow
            .compatibility_certificate_matches_resources(&certificate)
    );
}

fn assert_replay_rejects_snapshot_drift(checked: &mut checked_trees::CheckedTrees) {
    let before = checked.facts.borrow.compatibility_certificates.clone();
    let diagnostics =
        crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
            .expect_err("changed snapshot identity must reject replay");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("selector snapshot drifted from its captured-place shape")),
        "expected exact snapshot drift: {diagnostics:#?}"
    );
    assert_eq!(checked.facts.borrow.compatibility_certificates, before);
}

fn assert_borrow_conflict(source: &str) {
    let Err(diagnostics) = crate::lower_typed_trees(typed_source(source)) else {
        panic!("overlapping mutable windows must reject: {source}");
    };
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("creates local borrow")
                && diagnostic.message.contains("is still active")
        }),
        "expected a borrow conflict, not an earlier range failure: {diagnostics:#?}"
    );
}

#[test]
fn direct_mutable_parameter_cannot_retarget_an_earlier_bound() {
    assert_borrow_conflict(
        r#"
        data Main { items: [i32; 4]; }
        machine Main::split(&mut self, mut original: u64) -> u64
            requires original == 2 && original <= 4;
        {
            let left: &mut [i32] = self.items[0..original];
            original = 1;
            let right: &mut [i32] = self.items[original..4];
            left.len + right.len
        }
    "#,
    );
}

#[test]
fn immutable_boundary_snapshot_survives_mutation_of_its_source() {
    let checked = checked_source(
        r#"
        data Main { items: [i32; 4]; }
        machine Main::split(&mut self) -> u64 {
            let mut original: u64 = 2;
            let cut: u64 = original;
            let left: &mut [i32] = self.items[0..cut];
            original = 1;
            let right: &mut [i32] = self.items[cut..4];
            left.len + right.len
        }
    "#,
    );
    assert_snapshot(&checked, false);
}

#[test]
fn snapshot_copies_survive_local_and_parameter_mutation_in_both_formation_orders() {
    for parameter in [false, true] {
        for reverse in [false, true] {
            for copy_after_formation in [false, true] {
                let copies = "let copied: u64 = cut; let last: u64 = copied;";
                let first = if reverse {
                    "let right: &mut [i32] = self.items[cut..4];"
                } else {
                    "let left: &mut [i32] = self.items[0..cut];"
                };
                let second = if reverse {
                    "let left: &mut [i32] = self.items[0..last];"
                } else {
                    "let right: &mut [i32] = self.items[last..4];"
                };
                let body = if copy_after_formation {
                    format!("let cut: u64 = original; {first} original = 1; {copies} {second}")
                } else {
                    format!("let cut: u64 = original; {copies} {first} original = 1; {second}")
                };
                let mut checked = checked_source(&split_source(parameter, &body));
                assert_snapshot(&checked, reverse);
                assert_ne!(
                    local(&checked, "last").symbol,
                    local(&checked, "cut").symbol
                );
                let before = checked.facts.borrow.compatibility_certificates.clone();
                crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
                    .expect("snapshot certificates replay after source mutation");
                assert_eq!(checked.facts.borrow.compatibility_certificates, before);
            }
        }
    }
}

#[test]
fn separately_captured_snapshots_cannot_license_overlapping_mutable_windows() {
    for parameter in [false, true] {
        assert_borrow_conflict(&split_source(
            parameter,
            "
            let cut: u64 = original;
            let left: &mut [i32] = self.items[0..cut];
            original = 1;
            let later: u64 = original;
            let copied: u64 = later;
            let right: &mut [i32] = self.items[copied..4];
        ",
        ));
    }
}

#[test]
fn shared_windows_retain_distinct_snapshots_of_one_mutable_source() {
    for parameter in [false, true] {
        let source = split_source(
            parameter,
            "
            let cut: u64 = original;
            let left: &[i32] = self.items[0..cut];
            original = 1;
            let later: u64 = original;
            let copied: u64 = later;
            let right: &[i32] = self.items[copied..4];
        ",
        );
        let checked = checked_source(&source);
        let certificate = sole_certificate(&checked);
        let cut = local(&checked, "cut").symbol;
        let later = local(&checked, "later").symbol;
        assert_ne!(cut, later);
        assert_eq!(certificate.selector_snapshot.len(), 4);
        assert_eq!(
            certificate.selector_snapshot[0].value,
            Some(BorrowCompatibilitySelectorValue::Symbol(later))
        );
        assert_eq!(
            certificate.selector_snapshot[3].value,
            Some(BorrowCompatibilitySelectorValue::Symbol(cut))
        );
        assert!(!certificate.conclusion.disjoint);
        assert!(certificate.conclusion.non_interfering);
    }
}

#[test]
fn direct_mutable_local_cannot_retarget_an_earlier_bound() {
    assert_borrow_conflict(&split_source(
        false,
        "
        let left: &mut [i32] = self.items[0..original];
        original = 1;
        let right: &mut [i32] = self.items[original..4];
    ",
    ));
}

#[test]
fn direct_mutable_bounds_do_not_license_adjacency_without_a_snapshot() {
    for parameter in [false, true] {
        assert_borrow_conflict(&split_source(
            parameter,
            "
            let left: &mut [i32] = self.items[0..original];
            let right: &mut [i32] = self.items[original..4];
        ",
        ));
    }
}

#[test]
fn direct_mutable_bounds_remain_unknown_even_when_shared_loans_are_admitted() {
    for parameter in [false, true] {
        for mutation in ["", "original = 1;"] {
            let source = split_source(
                parameter,
                &format!(
                    "
                let left: &[i32] = self.items[0..original];
                {mutation}
                let right: &[i32] = self.items[original..4];
            "
                ),
            );
            let checked = checked_source(&source);
            let certificate = sole_certificate(&checked);
            assert_eq!(certificate.selector_snapshot.len(), 4);
            assert_eq!(certificate.selector_snapshot[0].value, None);
            assert_eq!(certificate.selector_snapshot[3].value, None);
            assert!(!certificate.conclusion.disjoint);
            assert_eq!(
                certificate.conclusion.containment,
                checked_trees::CapturedPlaceContainment::None
            );
            assert!(certificate.conclusion.non_interfering);
        }
    }
}

fn checked_with_spare_snapshot() -> checked_trees::CheckedTrees {
    checked_source(&split_source(
        false,
        "
        let cut: u64 = original;
        let copied: u64 = cut;
        let left: &mut [i32] = self.items[0..copied];
        original = 1;
        let later: u64 = original;
        let spare: u64 = later;
        let right: &mut [i32] = self.items[cut..4];
    ",
    ))
}

#[test]
fn changing_a_copy_to_a_later_snapshot_rejects_replay() {
    let mut checked = checked_with_spare_snapshot();
    let later_reference = local(&checked, "spare").initial_value;
    change_local(&mut checked, "copied", |local| {
        local.initial_value = later_reference
    });
    assert_replay_rejects_snapshot_drift(&mut checked);
}

#[test]
fn changing_snapshot_or_copy_mutability_rejects_replay() {
    for name in ["cut", "copied"] {
        let mut checked = checked_with_spare_snapshot();
        assert!(!local(&checked, name).is_mutable);
        change_local(&mut checked, name, |local| local.is_mutable = true);
        assert_replay_rejects_snapshot_drift(&mut checked);
    }
}

#[test]
fn co_tampering_frozen_bounds_to_the_mutable_source_rejects_replay() {
    let mut checked = checked_with_spare_snapshot();
    let original = local(&checked, "original").symbol;
    let row = checked
        .facts
        .borrow
        .compatibility_certificates
        .iter()
        .next()
        .expect("certificate")
        .0;
    let certificate = checked.facts.borrow.compatibility_certificates.get_mut(row);
    let mut changed = 0;
    for selector in &mut certificate.selector_snapshot {
        if matches!(
            selector.value,
            Some(BorrowCompatibilitySelectorValue::Symbol(_))
        ) {
            selector.value = Some(BorrowCompatibilitySelectorValue::Symbol(original));
            changed += 1;
        }
    }
    assert_eq!(changed, 2);
    assert!(certificate.conclusion.disjoint);
    assert_replay_rejects_snapshot_drift(&mut checked);
}

#[test]
fn cyclic_snapshot_copy_rejects_replay() {
    let mut checked = checked_with_spare_snapshot();
    let cut_reference = local(&checked, "copied").initial_value;
    change_local(&mut checked, "cut", |local| {
        local.initial_value = cut_reference
    });
    assert_replay_rejects_snapshot_drift(&mut checked);
}

#[test]
fn missing_snapshot_initializer_rejects_replay() {
    let mut checked = checked_with_spare_snapshot();
    change_local(&mut checked, "cut", |local| {
        local.initial_value = ExpressionHandle::invalid()
    });
    assert_replay_rejects_snapshot_drift(&mut checked);
}

#[test]
fn ambiguous_snapshot_declaration_identity_rejects_replay() {
    let mut checked = checked_with_spare_snapshot();
    let cut = local(&checked, "cut").symbol;
    change_local(&mut checked, "later", |local| local.symbol = cut);
    assert_replay_rejects_snapshot_drift(&mut checked);
}
