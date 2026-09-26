use super::sole_certificate;
use crate::tests::front_end::checked_program;
use checked_trees::BorrowCompatibilitySelectorValue;
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

/// `shifted_rows` counts the snapshot rows whose exclusive boundary is
/// `mid + 1`: one for `0..mid`, two for `0..=mid` because the inclusive end
/// is frozen as its exclusive equivalent.
fn assert_offset_adjacency_snapshot(
    checked: &mut checked_trees::CheckedTrees,
    shifted_rows: usize,
) {
    let certificate = sole_certificate(checked);
    let mid = local(checked, "mid").symbol;
    let shifted = Some(BorrowCompatibilitySelectorValue::SymbolOffset {
        symbol: mid,
        offset: 1,
    });
    let boundary = Some(BorrowCompatibilitySelectorValue::Symbol(mid));
    assert_eq!(
        certificate
            .selector_snapshot
            .iter()
            .filter(|row| row.value == shifted)
            .count(),
        shifted_rows,
        "exact shifted boundaries must be retained",
    );
    assert_eq!(
        certificate
            .selector_snapshot
            .iter()
            .filter(|row| row.value == boundary)
            .count(),
        2 - shifted_rows,
        "plain symbolic boundaries must be retained",
    );
    assert_eq!(
        certificate
            .selector_snapshot
            .iter()
            .filter(|row| {
                matches!(
                    row.value,
                    Some(
                        BorrowCompatibilitySelectorValue::Symbol(_)
                            | BorrowCompatibilitySelectorValue::SymbolOffset { .. }
                    )
                )
            })
            .count(),
        2
    );
    assert!(certificate.conclusion.disjoint);
    assert!(certificate.conclusion.non_interfering);
    assert_eq!(
        certificate.conclusion.containment,
        checked_trees::CapturedPlaceContainment::None
    );
    let before = checked.facts.borrow.compatibility_certificates.clone();
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        .expect("offset certificate replay");
    assert_eq!(checked.facts.borrow.compatibility_certificates, before);
}

#[test]
fn offset_computed_boundary_licenses_adjacent_mutable_loans() {
    for (left, right, shifted_rows) in [
        ("0..mid", "mid + 1..4", 1),
        ("0..=mid", "mid + 1..4", 2),
        ("0..mid", "1 + mid..4", 1),
    ] {
        for reverse in [false, true] {
            let mut checked =
                checked_program(&split_source("let mid: u64 = 1 + 1;", left, right, reverse));
            assert_offset_adjacency_snapshot(&mut checked, shifted_rows);
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
    let checked = checked_program(&source);
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

fn checked_with_spare_boundary() -> checked_trees::CheckedTrees {
    checked_program(&split_source(
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
