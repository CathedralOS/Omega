//! Immutable boundary copies retain their value when the original changes.

use super::sole_certificate;
use crate::tests::front_end::checked_program_result;
use typed_trees::statement::{StatementNode, TableLocalData};

fn fixture_result<T, E: std::fmt::Debug>(source: &str, stage: &str, result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| {
        let compact_source = source.split_whitespace().collect::<Vec<_>>().join(" ");
        panic!("{stage} failed: {error:#?}\nsource: {compact_source}");
    })
}

fn checked_source(source: &str) -> checked_trees::CheckedTrees {
    fixture_result(
        source,
        "check snapshot fixture",
        checked_program_result(source),
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

#[test]
fn immutable_copy_windows_certify_same_extent_and_replay() {
    for parameter in [false, true] {
        for reverse in [false, true] {
            let (first, second) = if reverse {
                ("self.items[0..last]", "self.items[0..cut]")
            } else {
                ("self.items[0..cut]", "self.items[0..last]")
            };
            let source = split_source(
                parameter,
                &format!(
                    "
                    let cut: u64 = original;
                    let last: u64 = cut;
                    let left: &[i32] = {first};
                    original = 1;
                    let right: &[i32] = {second};
                "
                ),
            );
            let mut checked = checked_source(&source);
            let certificate = sole_certificate(&checked);
            assert_eq!(
                certificate.conclusion.containment,
                checked_trees::CapturedPlaceContainment::Same,
                "immutable copies of one boundary certify the same extent"
            );
            assert!(!certificate.conclusion.disjoint);
            assert!(certificate.conclusion.non_interfering);
            // The containment bounds reuse the overlap selector rows; the
            // snapshot stays the four window bounds.
            assert_eq!(certificate.selector_snapshot.len(), 4);
            let before = checked.facts.borrow.compatibility_certificates.clone();
            crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
                .expect("same-extent certificates replay their selector rows");
            assert_eq!(checked.facts.borrow.compatibility_certificates, before);
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
fn ambiguous_snapshot_declaration_identity_rejects_replay() {
    let mut checked = checked_with_spare_snapshot();
    let cut = local(&checked, "cut").symbol;
    change_local(&mut checked, "later", |local| local.symbol = cut);
    assert_replay_rejects_snapshot_drift(&mut checked);
}
