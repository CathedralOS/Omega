use super::{checked_source, sole_certificate};
use checked_trees::{
    BorrowCompatibilityPlaceSide, BorrowCompatibilitySelectorPosition,
    BorrowCompatibilitySelectorValue,
};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TableLocalData};

fn split_source(declarations: &str, left: &str, right: &str, reverse: bool) -> String {
    let left = format!("let left: &mut [i32] = self.items[{left}];");
    let right = format!("let right: &mut [i32] = self.items[{right}];");
    let loans = if reverse {
        format!("{right}\n{left}")
    } else {
        format!("{left}\n{right}")
    };
    format!(
        "data Main {{ items: [i32; 4]; }}
         machine Main::split(&mut self) -> u64 {{
             {declarations}
             {loans}
             left.len + right.len
         }}"
    )
}

fn local(checked: &checked_trees::CheckedTrees, name: &str) -> TableLocalData {
    let machine_symbol = sole_certificate(checked).formation.machine_symbol;
    checked
        .typed
        .machines()
        .iter()
        .filter(|machine| machine.symbol == machine_symbol)
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
        .expect("fixture local in the certificate's forming machine")
}

fn selector(place: &checked_trees::CapturedPlace) -> ExpressionHandle {
    place
        .segments
        .iter()
        .find_map(|segment| match segment {
            facts::PlaceSegment::Index { expression } => Some(*expression),
            _ => None,
        })
        .expect("fixture range selector")
}

fn assert_adjacency_snapshot(checked: &checked_trees::CheckedTrees, reverse: bool) {
    let certificate = sole_certificate(checked);
    let mid = local(checked, "mid").symbol;
    assert!(mid.is_valid());
    let boundary = Some(BorrowCompatibilitySelectorValue::Symbol(mid));
    let zero = Some(BorrowCompatibilitySelectorValue::Integer(0));
    let four = Some(BorrowCompatibilitySelectorValue::Integer(4));
    let values = if reverse {
        [zero, boundary, boundary, four]
    } else {
        [boundary, four, zero, boundary]
    };
    let locations = [
        (
            BorrowCompatibilityPlaceSide::Forming,
            BorrowCompatibilitySelectorPosition::RangeStart,
        ),
        (
            BorrowCompatibilityPlaceSide::Forming,
            BorrowCompatibilitySelectorPosition::RangeExclusiveEnd,
        ),
        (
            BorrowCompatibilityPlaceSide::Active,
            BorrowCompatibilitySelectorPosition::RangeStart,
        ),
        (
            BorrowCompatibilityPlaceSide::Active,
            BorrowCompatibilitySelectorPosition::RangeExclusiveEnd,
        ),
    ];
    assert_eq!(
        certificate
            .selector_snapshot
            .iter()
            .map(|row| { (row.side, row.segment_index, row.position, row.value) })
            .collect::<Vec<_>>(),
        locations
            .into_iter()
            .zip(values)
            .map(|((side, position), value)| { (side, 0, position, value) })
            .collect::<Vec<_>>(),
        "both boundaries must freeze the computed declaration's exact symbol",
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

#[test]
fn computed_binding_and_finite_copies_freeze_the_original_symbol_in_both_orders() {
    for (declarations, left, right) in [
        ("let mid: u64 = 1 + 1;", "0..mid", "mid..4"),
        (
            "let mid: u64 = 1 + 1; let cut: u64 = mid;",
            "0..cut",
            "mid..4",
        ),
        (
            "let mid: u64 = 1 + 1; let cut: u64 = mid; let copied: u64 = cut;",
            "0..mid",
            "copied..4",
        ),
        (
            "let mid: u64 = 1 + 1; let cut: u64 = mid; let copied: u64 = cut; let last: u64 = copied;",
            "0..copied",
            "last..4",
        ),
    ] {
        for reverse in [false, true] {
            let mut checked = checked_source(&split_source(declarations, left, right, reverse));
            assert_adjacency_snapshot(&checked, reverse);
            let before = checked.facts.borrow.compatibility_certificates.clone();
            crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
                .expect("computed-boundary certificates replay without changing admission");
            assert_eq!(checked.facts.borrow.compatibility_certificates, before);
        }
    }
}

#[test]
fn copy_declared_after_the_first_loan_keeps_the_captured_computed_identity() {
    let source = split_source("let mid: u64 = 1 + 1;", "0..mid", "mid..4", false)
        .replace(
            "let right:",
            "let cut: u64 = mid; let copied: u64 = cut; let right:",
        )
        .replace("self.items[mid..4]", "self.items[copied..4]");
    let checked = checked_source(&source);
    assert_adjacency_snapshot(&checked, false);
    assert_ne!(
        local(&checked, "copied").symbol,
        local(&checked, "mid").symbol
    );
}

fn rejection(source: &str) -> Vec<diagnostics::Diagnostic> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize negative fixture");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse negative fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve negative fixture");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type negative fixture before checking borrow compatibility");
    let Err(diagnostics) = crate::lower_typed_trees(typed) else {
        panic!("these boundaries cannot license mutable adjacency: {source}");
    };
    diagnostics
}

fn assert_borrow_conflict(source: &str) {
    let diagnostics = rejection(source);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("creates local borrow")
                && diagnostic.message.contains("is still active")
        }),
        "expected the overlapping-loan rejection, got {diagnostics:#?}"
    );
}

#[test]
fn independent_computed_bindings_with_identical_initializers_do_not_prove_adjacency() {
    for declarations in [
        "let mid: u64 = 1 + 1; let other: u64 = 1 + 1;",
        "let other: u64 = 1 + 1; let mid: u64 = 1 + 1;",
    ] {
        for reverse in [false, true] {
            assert_borrow_conflict(&split_source(declarations, "0..mid", "other..4", reverse));
        }
    }
}

#[test]
fn shared_loans_retain_distinct_computed_symbols_even_for_identical_initializers() {
    let source = split_source(
        "let mid: u64 = 1 + 1; let other: u64 = 1 + 1; let copied: u64 = other;",
        "0..mid",
        "copied..4",
        false,
    )
    .replace("&mut", "&");
    let checked = checked_source(&source);
    let certificate = sole_certificate(&checked);
    let mid = local(&checked, "mid").symbol;
    let other = local(&checked, "other").symbol;
    assert_ne!(mid, other);
    assert_eq!(certificate.selector_snapshot.len(), 4);
    assert_eq!(
        certificate.selector_snapshot[0].value,
        Some(BorrowCompatibilitySelectorValue::Symbol(other))
    );
    assert_eq!(
        certificate.selector_snapshot[3].value,
        Some(BorrowCompatibilitySelectorValue::Symbol(mid))
    );
    assert!(!certificate.conclusion.disjoint);
    assert!(certificate.conclusion.non_interfering);
}

#[test]
fn inclusive_computed_upper_bound_cannot_certify_half_open_adjacency() {
    for reverse in [false, true] {
        assert_borrow_conflict(&split_source(
            "let mid: u64 = 1 + 1;",
            "0..=mid",
            "mid..4",
            reverse,
        ));
    }
}

#[test]
fn mutable_computed_boundary_cannot_license_adjacent_mutable_loans() {
    for declarations in [
        "let mut mid: u64 = 1 + 1;",
        "let mid: u64 = 1 + 1; let mut cut: u64 = mid;",
    ] {
        let boundary = if declarations.contains("cut") {
            "cut"
        } else {
            "mid"
        };
        let diagnostics = rejection(&split_source(
            declarations,
            &format!("0..{boundary}"),
            &format!("{boundary}..4"),
            false,
        ));
        assert!(
            diagnostics.iter().any(|diagnostic| {
                (diagnostic.message.contains("creates local borrow")
                    && diagnostic.message.contains("is still active"))
                    || (diagnostic.message.contains("cannot prove subslice range")
                        && diagnostic.message.contains("within slice length"))
            }),
            "mutable bounds may fail range validation before borrow admission: {diagnostics:#?}"
        );
    }
}

#[test]
fn unrelated_same_spelled_local_in_another_machine_cannot_supply_boundary_identity() {
    let split = split_source(
        "let mid: u64 = 1 + 1; let cut: u64 = mid;",
        "0..cut",
        "mid..4",
        false,
    );
    let unrelated = "machine unrelated() -> u64 { let mid: u64 = 1 + 1; mid }";
    // Source scopes retain distinct declaration identities even when local
    // spellings agree. The certificate must name this machine's binding.
    for source in [
        format!("{unrelated}\n{split}"),
        format!("{split}\n{unrelated}"),
    ] {
        let checked = checked_source(&source);
        assert_adjacency_snapshot(&checked, false);
        let selected = local(&checked, "mid").symbol;
        let same_spelled = checked
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
            .filter_map(|statement| match statement {
                StatementNode::LocalData(local) if local.name.as_str() == "mid" => {
                    Some(local.symbol)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(same_spelled.len(), 2);
        assert_ne!(same_spelled[0], same_spelled[1]);
        assert!(same_spelled.contains(&selected));
    }
}

fn checked_with_spare_boundary() -> checked_trees::CheckedTrees {
    checked_source(&split_source(
        "let mid: u64 = 1 + 1; let cut: u64 = mid; let other: u64 = 1 + 1; let other_copy: u64 = other;",
        "0..cut",
        "mid..4",
        false,
    ))
}

fn assert_replay_rejects_snapshot_drift(checked: &mut checked_trees::CheckedTrees) {
    let before = checked.facts.borrow.compatibility_certificates.clone();
    let diagnostics =
        crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
            .expect_err("computed binding identity drift must reject retained evidence");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("selector snapshot drifted from its captured-place shape")),
        "expected exact snapshot drift: {diagnostics:#?}"
    );
    assert_eq!(checked.facts.borrow.compatibility_certificates, before);
}

#[test]
fn changed_typed_computed_selector_identity_rejects_certificate_replay() {
    let mut checked = checked_with_spare_boundary();
    let certificate = sole_certificate(&checked);
    let other_reference = local(&checked, "other_copy").initial_value;
    assert_ne!(
        local(&checked, "mid").symbol,
        local(&checked, "other").symbol
    );
    let ExpressionNode::Range(range) = checked
        .typed
        .expression_table
        .expression_mut(selector(&certificate.forming_place))
    else {
        panic!("forming range selector");
    };
    range.start = other_reference;
    assert_replay_rejects_snapshot_drift(&mut checked);
}

#[test]
fn co_tampering_both_frozen_computed_symbols_cannot_preserve_adjacency_evidence() {
    let mut checked = checked_with_spare_boundary();
    let other = local(&checked, "other").symbol;
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
            selector.value = Some(BorrowCompatibilitySelectorValue::Symbol(other));
            changed += 1;
        }
    }
    assert_eq!(changed, 2);
    assert!(certificate.conclusion.disjoint);
    assert_replay_rejects_snapshot_drift(&mut checked);
}

#[test]
fn a_cycle_in_the_computed_copy_chain_rejects_certificate_replay() {
    let mut checked = checked_with_spare_boundary();
    let mid = local(&checked, "mid").symbol;
    let mid_reference = local(&checked, "cut").initial_value;
    let spans = checked
        .typed
        .machines()
        .iter()
        .flat_map(|machine| checked.typed.machine_states(machine))
        .map(|state| state.statement_nodes)
        .collect::<Vec<_>>();
    let mut changed = false;
    for span in spans {
        for statement in checked.typed.statement_table.statements_mut(span) {
            if let StatementNode::LocalData(local) = statement
                && local.symbol == mid
            {
                local.initial_value = mid_reference;
                changed = true;
            }
        }
    }
    assert!(changed);
    assert_replay_rejects_snapshot_drift(&mut checked);
}
