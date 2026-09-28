//! Stated `requires` ordering premises certify relational conclusions the
//! structural selector judgment cannot reach.
//!
//! A premise is exact relational evidence only: it retains the
//! `ContractProofFact` row it was decomposed from and its normalized
//! immutable-bound operands, replays positionally against the formation
//! scope's re-derived contracts, and never mints, extends, or transfers loan
//! authority.

use crate::tests::front_end::{checked_program, checked_program_result};

const DISEQUAL_ELEMENTS: &str = r#"
    data Entry { value: i32; }
    data Main { entries: [Entry; 2]; }

    machine Main::main(&mut self, left_index: u64 [0..=1], right_index: u64 [0..=1])
        requires left_index != right_index;
    {
        let view: &mut [Entry] = self.entries.as_mut_slice();
        let left: &mut Entry = &mut view[left_index];
        let right: &mut Entry = &mut view[right_index];
        left = Entry { value: 1 };
        right = Entry { value: 2 };
    }
"#;

#[test]
fn disequality_cannot_replace_missing_strictness_or_current_subjects() {
    for relation in [
        "left_index <= right_index",
        "left_index == right_index",
        "left_index <= 1",
    ] {
        assert_borrow_conflict(&DISEQUAL_ELEMENTS.replace("left_index != right_index", relation));
    }
    assert_borrow_conflict(
        &DISEQUAL_ELEMENTS
            .replace("right_index: u64", "mut right_index: u64")
            .replace("let view:", "right_index = left_index; let view:"),
    );
    assert_borrow_conflict(&DISJOINT_WINDOWS.replace("cut <= last", "cut != last"));
}

#[test]
fn disequality_certificate_rejects_missing_retargeted_and_reordered_evidence() {
    for corruption in [
        "missing",
        "relation",
        "operand",
        "fact",
        "selectors",
        "conclusion",
    ] {
        let mut checked = checked_program(DISEQUAL_ELEMENTS);
        let handle = checked
            .facts
            .borrow
            .compatibility_certificates
            .iter()
            .find(|(_, certificate)| {
                certificate.derivation == checked_trees::BorrowCompatibilityDerivation::Premised
            })
            .expect("premised pair")
            .0;
        let certificate = checked
            .facts
            .borrow
            .compatibility_certificates
            .get_mut(handle);
        let expected = match corruption {
            "missing" => {
                certificate.premises.clear();
                "derivation drifted"
            }
            "relation" => {
                certificate.premises[0].relation =
                    checked_trees::BorrowCompatibilityPremiseRelation::Equal;
                "premise tokens drifted"
            }
            "operand" => {
                certificate.premises[0].right = certificate.premises[0].left.clone();
                "premise tokens drifted"
            }
            "fact" => {
                certificate.premises[0].source =
                    checked_trees::BorrowCompatibilityPremiseSource::Requires(
                        arena::Handle::invalid(),
                    );
                "premise tokens drifted"
            }
            "selectors" => {
                certificate.selector_snapshot.reverse();
                "selector snapshot drifted"
            }
            _ => {
                certificate.conclusion.containment = checked_trees::CapturedPlaceContainment::Same;
                "conclusion drifted"
            }
        };
        assert_recording_rejects(&mut checked, expected);
    }
}

/// `cut <= last` makes `[0, cut)` and `[last, 4)` provably disjoint for two
/// distinct symbolic parameters; the structural judgment alone cannot order
/// them.
const DISJOINT_WINDOWS: &str = r#"
    data Main { items: [i32; 4]; }

    machine Main::split(&mut self, cut: u64, last: u64) -> u64
        requires cut <= 4 && cut <= last && last <= 4;
    {
        let left: &mut [i32] = self.items[0..cut];
        let right: &mut [i32] = self.items[last..4];
        left.len + right.len
    }
"#;

fn assert_recording_rejects(
    checked: &mut checked_trees::CheckedTrees,
    message: &str,
) -> Vec<diagnostics::Diagnostic> {
    let before = checked.facts.borrow.compatibility_certificates.clone();
    let before_mutations = checked.facts.borrow.mutation_certificates.clone();
    let diagnostics =
        crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
            .expect_err("tampered premised evidence must reject replay");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(message)),
        "expected `{message}`: {diagnostics:#?}"
    );
    assert_eq!(
        checked.facts.borrow.compatibility_certificates, before,
        "failed replay must preserve the retained proof ledger",
    );
    assert_eq!(
        checked.facts.borrow.mutation_certificates, before_mutations,
        "failed replay must preserve the retained mutation ledger",
    );
    diagnostics
}

fn assert_borrow_conflict(source: &str) {
    let Err(diagnostics) = checked_program_result(source) else {
        panic!("premise-insufficient windows must reject: {source}");
    };
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("creates local borrow")
                && diagnostic.message.contains("is still active")
        }),
        "expected a borrow conflict, not an earlier failure: {diagnostics:#?}"
    );
}

#[test]
fn equality_premise_does_not_license_a_second_exclusive_loan() {
    // `cut == last` makes the windows the same extent; the same extent is
    // exactly what two exclusive loans cannot share.
    assert_borrow_conflict(
        r#"
        data Main { items: [i32; 4]; }

        machine Main::split(&mut self, cut: u64, last: u64) -> u64
            requires cut <= 4 && cut == last && last <= 4;
        {
            let left: &mut [i32] = self.items[0..cut];
            let right: &mut [i32] = self.items[0..last];
            left.len + right.len
        }
    "#,
    );
}

/// `i + j < cut` states the ordering on a two-term bound: the summed index
/// normalizes to one canonical pair whose terms separate the write's point
/// index from the `[cut, 4)` window.
const SUMMED_INDEX: &str = r#"
    data Main { items: [i32; 4]; }

    machine Main::split(&mut self, i: u64 [0..2], j: u64 [0..2], cut: u64 [0..4]) -> u64
        requires i + j < cut;
    {
        let left: &mut [i32] = self.items[cut..4];
        self.items[i + j] = 7;
        left.len
    }
"#;

#[test]
fn rejects_retained_sum_operand_retarget() {
    // Reordering the recorded pair breaks its canonical member order, so the
    // re-derived premise tokens drift.
    let mut checked = checked_program(SUMMED_INDEX);
    let row = checked
        .facts
        .borrow
        .mutation_certificates
        .iter()
        .next()
        .expect("certificate")
        .0;
    let certificate = checked.facts.borrow.mutation_certificates.get_mut(row);
    let checked_trees::BorrowCompatibilitySelectorValue::SymbolSum { first, second, .. } =
        &mut certificate.premises[0].left
    else {
        panic!("the summed premise records a two-symbol bound");
    };
    std::mem::swap(first, second);

    assert_recording_rejects(&mut checked, "premise tokens drifted");
}
