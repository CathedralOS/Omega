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

fn checked_symbolic_adjacency() -> checked_trees::CheckedTrees {
    checked_source(SYMBOLIC_ADJACENCY)
}

fn checked_source(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize borrow-certificate fixture");
    let syntax = parse_syntax_trees(&tokens).expect("parse borrow-certificate fixture");
    let resolved =
        lower_syntax_trees(&syntax).expect("resolve borrow-certificate fixture identities");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type borrow-certificate fixture");
    lower_typed_trees(typed).expect("automatic structural compatibility should remain admitted")
}

fn sole_certificate(
    checked: &checked_trees::CheckedTrees,
) -> checked_trees::CheckedBorrowCompatibilityCertificate {
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
        checked_trees::BorrowCompatibilityDerivation::Structural
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
        checked_trees::CapturedPlaceContainment::None
    );
    assert!(certificate.conclusion.non_interfering);
    assert_eq!(certificate.selector_snapshot.len(), 4);
    assert_eq!(
        certificate
            .selector_snapshot
            .iter()
            .map(|row| (row.side, row.segment_index, row.position, row.value))
            .collect::<Vec<_>>(),
        vec![
            (
                checked_trees::BorrowCompatibilityPlaceSide::Forming,
                0,
                checked_trees::BorrowCompatibilitySelectorPosition::RangeStart,
                Some(checked_trees::BorrowCompatibilitySelectorValue::Integer(2)),
            ),
            (
                checked_trees::BorrowCompatibilityPlaceSide::Forming,
                0,
                checked_trees::BorrowCompatibilitySelectorPosition::RangeExclusiveEnd,
                Some(checked_trees::BorrowCompatibilitySelectorValue::Integer(4)),
            ),
            (
                checked_trees::BorrowCompatibilityPlaceSide::Active,
                0,
                checked_trees::BorrowCompatibilitySelectorPosition::RangeStart,
                Some(checked_trees::BorrowCompatibilitySelectorValue::Integer(0)),
            ),
            (
                checked_trees::BorrowCompatibilityPlaceSide::Active,
                0,
                checked_trees::BorrowCompatibilitySelectorPosition::RangeExclusiveEnd,
                Some(checked_trees::BorrowCompatibilitySelectorValue::Integer(2)),
            ),
        ],
        "the certificate freezes only the four normalized bounds consulted by the structural judgment",
    );
    assert!(
        checked
            .facts
            .borrow
            .compatibility_certificate_matches_resources(&certificate)
    );
}

#[test]
fn rejects_typed_alias_value_drift_without_mutating_retained_certificates() {
    let mut checked = checked_symbolic_adjacency();
    let before = checked.facts.borrow.compatibility_certificates.clone();
    let three =
        checked
            .typed
            .expression_table
            .insert(typed_trees::expression::ExpressionNode::Integer(
                numerics::literals::IntegerLiteral::from_value(3),
            ));
    let statement_spans = checked
        .typed
        .machines()
        .iter()
        .flat_map(|machine| checked.typed.machine_states(machine).iter())
        .map(|state| state.statement_nodes)
        .collect::<Vec<_>>();
    let mut changed_mid = false;
    for span in statement_spans {
        for statement in checked.typed.statement_table.statements_mut(span) {
            let typed_trees::statement::StatementNode::LocalData(local) = statement else {
                continue;
            };
            if local.name.as_str() == "mid" {
                local.initial_value = three;
                changed_mid = true;
            }
        }
    }
    assert!(
        changed_mid,
        "fixture must retain the immutable mid declaration"
    );

    let diagnostics =
        crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
            .expect_err("changed formation normalization must reject the retained snapshot");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("selector snapshot drifted from its captured-place shape")
    }));
    assert_eq!(
        checked.facts.borrow.compatibility_certificates, before,
        "failed replay must preserve the exact retained proof ledger",
    );
}

#[test]
fn rejects_tampered_normalized_selector_value() {
    let mut checked = checked_symbolic_adjacency();
    let row = checked
        .facts
        .borrow
        .compatibility_certificates
        .iter()
        .next()
        .expect("certificate")
        .0;
    let certificate = checked.facts.borrow.compatibility_certificates.get_mut(row);
    let active_end = certificate
        .selector_snapshot
        .iter_mut()
        .find(|selector| {
            selector.side == checked_trees::BorrowCompatibilityPlaceSide::Active
                && selector.position
                    == checked_trees::BorrowCompatibilitySelectorPosition::RangeExclusiveEnd
        })
        .expect("active exclusive end snapshot");
    active_end.value = Some(checked_trees::BorrowCompatibilitySelectorValue::Integer(3));

    let diagnostics =
        crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
            .expect_err("changed frozen boundary must not retain the admitted conclusion");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("selector snapshot drifted from its captured-place shape")
    }));
}

#[test]
fn rejects_co_tampered_selector_snapshot_and_conclusion() {
    let mut checked = checked_symbolic_adjacency();
    let row = checked
        .facts
        .borrow
        .compatibility_certificates
        .iter()
        .next()
        .expect("certificate")
        .0;
    let certificate = checked.facts.borrow.compatibility_certificates.get_mut(row);
    certificate
        .selector_snapshot
        .iter_mut()
        .find(|selector| {
            selector.side == checked_trees::BorrowCompatibilityPlaceSide::Active
                && selector.position
                    == checked_trees::BorrowCompatibilitySelectorPosition::RangeExclusiveEnd
        })
        .expect("active exclusive end snapshot")
        .value = Some(checked_trees::BorrowCompatibilitySelectorValue::Integer(3));
    certificate.conclusion.disjoint = false;
    certificate.conclusion.non_interfering = false;

    let diagnostics =
        crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
            .expect_err("co-tampering snapshot and conclusion cannot forge compatibility");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("selector snapshot drifted from its captured-place shape")
    }));
}

#[test]
fn rejects_duplicate_certificate_key_transactionally() {
    let mut checked = checked_symbolic_adjacency();
    let duplicate = sole_certificate(&checked);
    checked
        .facts
        .borrow
        .compatibility_certificates
        .insert(duplicate);
    let before = checked.facts.borrow.compatibility_certificates.clone();

    let diagnostics =
        crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
            .expect_err("one formation loan pair cannot retain duplicate certificates");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("duplicates the formation loan-pair key")
    }));
    assert_eq!(checked.facts.borrow.compatibility_certificates, before);
}

#[test]
fn rejects_unconsumed_retained_certificate_transactionally() {
    let mut checked = checked_symbolic_adjacency();
    let certificate = sole_certificate(&checked);
    let state = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| {
            state.machine_symbol == certificate.formation.machine_symbol
                && state.state_symbol == certificate.formation.state_symbol
        })
        .expect("formation flow state");
    let statement = checked
        .facts
        .flow
        .control
        .statements
        .span_or_empty(state.statements)
        .iter()
        .find(|statement| statement.statement_index == certificate.formation.statement_index)
        .expect("formation flow statement");
    let active_constraint = (0..statement.entry_constraints.count())
        .map(|offset| {
            arena::Handle::from_parts(
                statement.entry_constraints.start().arena_index() + offset,
                statement.entry_constraints.start().generation(),
            )
        })
        .find(|handle| {
            matches!(
                checked.facts.flow.contexts.constraint_refs.get(*handle).kind,
                checked_trees::FlowConstraintKind::BorrowLoan { loan }
                    if loan == certificate.active_loan
            )
        })
        .expect("active-loan entry constraint");
    checked
        .facts
        .flow
        .contexts
        .constraint_refs
        .get_mut(active_constraint)
        .kind = checked_trees::FlowConstraintKind::Unknown;
    let before = checked.facts.borrow.compatibility_certificates.clone();

    let diagnostics =
        crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
            .expect_err("a retained certificate must be consumed by its exact formation pair");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("was not consumed by its exact formation loan pair")
    }));
    assert_eq!(checked.facts.borrow.compatibility_certificates, before);
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
            facts::PlaceSegment::Index { expression } => Some(*expression),
            _ => None,
        })
        .expect("active symbolic window selector");
    let forming_selector = certificate
        .forming_place
        .segments
        .iter_mut()
        .find_map(|segment| match segment {
            facts::PlaceSegment::Index { expression } => Some(expression),
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
fn rejects_each_changed_compatibility_conclusion_axis() {
    for axis in 0..3 {
        let mut checked = checked_symbolic_adjacency();
        let row = checked
            .facts
            .borrow
            .compatibility_certificates
            .iter()
            .next()
            .expect("certificate")
            .0;
        let certificate = checked.facts.borrow.compatibility_certificates.get_mut(row);
        match axis {
            0 => certificate.conclusion.disjoint = false,
            1 => certificate.conclusion.containment = checked_trees::CapturedPlaceContainment::Same,
            2 => certificate.conclusion.non_interfering = false,
            _ => unreachable!(),
        }

        let diagnostics =
            crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
                .expect_err("each independently replayed conclusion axis must be exact");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("conclusion drifted"))
        );
    }
}

#[test]
fn rejects_raw_loan_access_drift_from_joined_resource_polarity() {
    let mut checked = checked_shared_overlap();
    let certificate = sole_certificate(&checked);
    assert!(!certificate.conclusion.disjoint);
    assert_eq!(
        certificate.conclusion.containment,
        checked_trees::CapturedPlaceContainment::Same
    );
    assert!(certificate.conclusion.non_interfering);

    checked
        .facts
        .borrow
        .loans
        .get_mut(certificate.forming_loan)
        .kind = checked_trees::BorrowAccessKind::Mutable;

    let diagnostics =
        crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
            .expect_err("the direct resource must replay from the authoritative loan polarity");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("resource closure drifted"))
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
    changed_machine.formation.machine_symbol = symbols::SymbolHandle::invalid();
    let mut changed_state = certificate.clone();
    changed_state.formation.state_symbol = symbols::SymbolHandle::invalid();
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

fn checked_shared_overlap() -> checked_trees::CheckedTrees {
    let source = r#"
        data Main { value: i32; }

        machine observe(value: &i32) {}

        machine Main::read_twice(&self) {
            let left: &i32 = &self.value;
            let right: &i32 = &self.value;
            observe(left);
            observe(right);
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize shared overlap");
    let syntax = parse_syntax_trees(&tokens).expect("parse shared overlap");
    let resolved = lower_syntax_trees(&syntax).expect("resolve shared overlap");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type shared overlap");
    lower_typed_trees(typed).expect("two overlapping shared loans should remain admitted")
}
