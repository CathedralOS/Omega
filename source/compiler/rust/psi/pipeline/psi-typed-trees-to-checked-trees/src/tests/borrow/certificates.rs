use super::super::*;

const SYMBOLIC_ADJACENCY: &str = r#"
    data Main { items: [i32; 4]; }

    machine Main::split(&mut self) -> u64 {
        let mid: u64 = 2;
        let cut: u64 = mid;
        let left: &mut [i32] = self.items[0..cut];
        let right: &mut [i32] = self.items[mid..4];
        left.len + right.len
    }
"#;

fn checked_symbolic_adjacency() -> psi_checked_trees::CheckedTrees {
    let tokens = Lexer::new(SYMBOLIC_ADJACENCY)
        .tokenize()
        .expect("tokenize symbolic adjacency");
    let syntax = parse_syntax_trees(&tokens).expect("parse symbolic adjacency");
    let resolved = lower_syntax_trees(&syntax).expect("resolve symbolic adjacency loan identities");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type symbolic adjacency");
    lower_typed_trees(typed).expect("automatic symbolic adjacency should remain admitted")
}

fn sole_certificate(
    checked: &psi_checked_trees::CheckedTrees,
) -> psi_checked_trees::CheckedBorrowCompatibilityCertificate {
    let certificates = checked
        .facts
        .borrow
        .compatibility_certificates
        .iter()
        .map(|(_, certificate)| certificate.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        certificates.len(),
        1,
        "only the newly formed right/active left loan pair is certified"
    );
    certificates.into_iter().next().expect("sole certificate")
}

#[test]
fn retains_zero_premise_structural_symbolic_adjacency_certificate() {
    let checked = checked_symbolic_adjacency();
    let certificate = sole_certificate(&checked);

    assert_eq!(
        certificate.derivation,
        psi_checked_trees::BorrowCompatibilityDerivation::Structural
    );
    assert_eq!(certificate.formation.statement_index, 3);
    assert_ne!(certificate.forming_loan, certificate.active_loan);
    assert_eq!(
        certificate.forming_place.root_symbol,
        certificate.active_place.root_symbol
    );
    assert!(certificate.conclusion.disjoint);
    assert_eq!(
        certificate.conclusion.containment,
        psi_checked_trees::CapturedPlaceContainment::None
    );
    assert!(certificate.conclusion.non_interfering);
    assert!(
        checked
            .facts
            .borrow
            .compatibility_certificate_matches_resources(&certificate)
    );
}

#[test]
fn rejects_symbolic_adjacency_certificate_with_changed_frozen_selector_identity() {
    let checked = checked_symbolic_adjacency();
    let mut certificate = sole_certificate(&checked);
    let active_selector = certificate
        .active_place
        .segments
        .iter()
        .find_map(|segment| match segment {
            psi_facts::PlaceSegment::Index { expression } => Some(*expression),
            _ => None,
        })
        .expect("active symbolic window selector");
    let forming_selector = certificate
        .forming_place
        .segments
        .iter_mut()
        .find_map(|segment| match segment {
            psi_facts::PlaceSegment::Index { expression } => Some(expression),
            _ => None,
        })
        .expect("forming symbolic window selector");
    assert_ne!(*forming_selector, active_selector);
    *forming_selector = active_selector;

    assert!(
        !checked
            .facts
            .borrow
            .compatibility_certificate_matches_resources(&certificate),
        "a symbolic-adjacency conclusion cannot move to a different frozen selector"
    );
}

#[test]
fn rejects_compatibility_certificate_with_changed_loan_identity() {
    let checked = checked_symbolic_adjacency();
    let mut certificate = sole_certificate(&checked);
    certificate.active_loan = certificate.forming_loan;

    assert!(
        !checked
            .facts
            .borrow
            .compatibility_certificate_matches_resources(&certificate),
        "the row must retain two exact, distinct loan identities"
    );
}

#[test]
fn rejects_compatibility_certificate_with_changed_formation_coordinate() {
    let checked = checked_symbolic_adjacency();
    let certificate = sole_certificate(&checked);

    let mut changed_machine = certificate.clone();
    changed_machine.formation.machine_symbol = psi_symbols::SymbolHandle::invalid();
    let mut changed_state = certificate.clone();
    changed_state.formation.state_symbol = psi_symbols::SymbolHandle::invalid();
    let mut changed_statement = certificate;
    changed_statement.formation.statement_index += 1;

    for changed in [changed_machine, changed_state, changed_statement] {
        assert!(
            !checked
                .facts
                .borrow
                .compatibility_certificate_matches_resources(&changed),
            "each formation coordinate is part of certificate identity"
        );
    }
}

#[test]
fn rebuilding_checked_borrow_certificates_is_idempotent() {
    let mut checked = checked_symbolic_adjacency();
    let before = sole_certificate(&checked);

    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        .expect("rerunning checked recording should preserve admission");

    assert_eq!(sole_certificate(&checked), before);
}
