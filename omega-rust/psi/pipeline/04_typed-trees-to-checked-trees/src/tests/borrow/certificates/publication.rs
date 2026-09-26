//! The authored borrow certificate ledgers cross the publication boundary
//! inside the checked fact arenas: a consumer holding only the typed program
//! and the published facts replays every family independently, and replay
//! rejects a row that drifted from, duplicated or retargeted its recorded
//! subjects.

use crate::tests::front_end::checked_program;

/// Stated `i < cut` discharges the write and the exclusive call operand
/// beside the borrowed `[cut, 4)` window: the published facts retain a
/// premised mutation certificate, the forming call's compatibility
/// certificate, and the exact invocation's call-ledger row.
const PREMISED_WRITE: &str = r#"
    data Main { items: [i32; 4]; }

    machine take(slot: &mut i32) {
        slot = 7;
    }

    machine Main::main(&mut self, i: u64, cut: u64) -> u64
        requires i < cut && cut <= 4 && i < 4;
    {
        let left: &mut [i32] = self.items[cut..4];
        self.items[i] = 7;
        take(&mut self.items[i]);
        left.len
    }
"#;

#[test]
fn published_borrow_certificates_replay_independently() {
    let checked = checked_program(PREMISED_WRITE);
    let borrow = &checked.facts.borrow;
    assert!(
        borrow.mutation_certificates.iter().any(|(_, certificate)| {
            certificate.derivation == checked_trees::BorrowCompatibilityDerivation::Premised
        }),
        "the premised write must author a premised mutation certificate"
    );
    crate::replay_checked_borrow_certificates(&checked.typed, &checked.facts)
        .expect("published borrow certificates replay independently");

    let adjacent = checked_program(super::SYMBOLIC_ADJACENCY);
    assert!(
        adjacent
            .facts
            .borrow
            .compatibility_certificates
            .iter()
            .next()
            .is_some(),
        "the adjacent disjoint loans must author compatibility certificates"
    );
    crate::replay_checked_borrow_certificates(&adjacent.typed, &adjacent.facts)
        .expect("published compatibility certificates replay independently");
}

#[test]
fn premise_token_drift_rejects_post_publication() {
    let checked = checked_program(PREMISED_WRITE);
    let mut facts = checked.facts.clone();
    let (handle, _) = facts
        .borrow
        .mutation_certificates
        .iter()
        .find(|(_, certificate)| !certificate.premises.is_empty())
        .expect("a premised mutation certificate");
    facts
        .borrow
        .mutation_certificates
        .get_mut(handle)
        .premises
        .pop();

    let diagnostics = crate::replay_checked_borrow_certificates(&checked.typed, &facts)
        .expect_err("a certificate with a truncated premise ledger must not replay");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("derivation drifted")
                || diagnostic.message.contains("premise tokens drifted")
        }),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn fabricated_certificate_rejects_post_publication() {
    let checked = checked_program(PREMISED_WRITE);
    let mut facts = checked.facts.clone();
    let certificate = facts
        .borrow
        .mutation_certificates
        .iter()
        .next()
        .expect("a mutation certificate")
        .1
        .clone();
    facts.borrow.mutation_certificates.insert(certificate);

    let diagnostics = crate::replay_checked_borrow_certificates(&checked.typed, &facts)
        .expect_err("a fabricated row must not replay");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("duplicates the formation")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn retargeted_certificate_rejects_post_publication() {
    let checked = checked_program(PREMISED_WRITE);
    let mut facts = checked.facts.clone();
    let (handle, _) = facts
        .borrow
        .mutation_certificates
        .iter()
        .next()
        .expect("a mutation certificate");
    facts
        .borrow
        .mutation_certificates
        .get_mut(handle)
        .formation
        .statement_index += 1;

    let diagnostics = crate::replay_checked_borrow_certificates(&checked.typed, &facts)
        .expect_err("a retargeted row must not replay");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("does not rejoin")
                || diagnostic.message.contains("was not consumed")
                || diagnostic.message.contains("has no mutated place")
        }),
        "unexpected diagnostics: {diagnostics:?}"
    );
}
