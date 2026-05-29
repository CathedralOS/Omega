use super::*;

#[test]
fn exposes_checked_operation_acceptance_from_one_query_surface() {
    let source = r#"
        data Main {}

        machine Main::echo(&mut self)
        requires
            true
        {
        }

        machine Main::main(&mut self) {
            self.echo();
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source)).expect("program should check");
    let main = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("main machine");
    let main_state = checked.machine_states(main).first().expect("main state");

    let state_acceptance = checked
        .state_acceptance(main.symbol, main_state.symbol)
        .expect("main state acceptance should be queryable");

    assert!(state_acceptance.is_accepted());
    let state_summary = state_acceptance.summary();
    assert!(state_summary.is_accepted());
    assert_eq!(state_summary.checks().len(), 5);
    assert_eq!(state_summary.rejected_checks().count(), 0);
    assert!(state_summary.borrow.evidence_count > 0);
    assert_eq!(state_summary.borrow.diagnostic_count, 0);
    assert_eq!(
        state_summary.borrow.provenance,
        omega_checked_trees::AcceptanceCheckProvenance::AcceptedByEvidence
    );
    assert_eq!(state_summary.proof.evidence_count, 1);
    assert_eq!(state_acceptance.statements().len(), 1);
    assert_eq!(state_acceptance.calls().len(), 1);

    let statement_acceptance = state_acceptance
        .statement(0)
        .expect("call statement acceptance should be queryable");
    assert!(statement_acceptance.is_accepted());
    assert!(statement_acceptance.summary().borrow.evidence_count > 0);
    assert!(!statement_acceptance.entry_constraints().is_empty());

    let call_fact = state_acceptance.calls()[0].clone();
    let call_acceptance = state_acceptance
        .call(
            call_fact.statement_index,
            call_fact.call_ordinal,
            call_fact.target_symbol,
            call_fact.receiver_symbol,
        )
        .expect("call acceptance should be queryable");

    assert!(call_acceptance.is_accepted());
    let call_summary = call_acceptance.summary();
    assert!(call_summary.is_accepted());
    assert_eq!(call_summary.checks().len(), 5);
    assert_eq!(call_summary.rejected_checks().count(), 0);
    assert!(call_summary.borrow.evidence_count > 0);
    assert_eq!(call_summary.proof.evidence_count, 1);
    assert_eq!(
        call_summary.termination.verdict,
        omega_checked_trees::AcceptanceCheckVerdict::NotApplicable
    );
    assert_eq!(
        call_summary.termination.provenance,
        omega_checked_trees::AcceptanceCheckProvenance::NotRequired
    );
    assert!(!call_acceptance.entry_constraints().is_empty());
    assert!(!call_acceptance.requires_constraints().is_empty());
    assert_eq!(call_acceptance.requires().len(), 1);
    assert!(call_acceptance.boundary_edges().is_empty());
}

#[test]
fn acceptance_checks_have_diagnostic_provenance_shape_for_rejections() {
    let rejected = omega_checked_trees::AcceptanceCheck::rejected(
        omega_checked_trees::AcceptanceDimension::Borrow,
        2,
    );

    assert_eq!(
        rejected.verdict,
        omega_checked_trees::AcceptanceCheckVerdict::Rejected
    );
    assert_eq!(rejected.evidence_count, 0);
    assert_eq!(rejected.diagnostic_count, 2);
    assert_eq!(
        rejected.provenance,
        omega_checked_trees::AcceptanceCheckProvenance::RejectedByDiagnostic
    );
    assert!(!rejected.is_satisfied());

    let pending = omega_checked_trees::AcceptanceCheck::rejected(
        omega_checked_trees::AcceptanceDimension::Proof,
        0,
    );
    assert_eq!(
        pending.provenance,
        omega_checked_trees::AcceptanceCheckProvenance::DiagnosticPending
    );
}

#[test]
fn acceptance_summary_derives_rejection_from_dimension_records() {
    let summary = omega_checked_trees::AcceptanceSummary::with_checks(
        omega_checked_trees::AcceptanceCheck::accepted(
            omega_checked_trees::AcceptanceDimension::Borrow,
            3,
        ),
        omega_checked_trees::AcceptanceCheck::rejected(
            omega_checked_trees::AcceptanceDimension::Proof,
            1,
        ),
        omega_checked_trees::AcceptanceCheck::accepted(
            omega_checked_trees::AcceptanceDimension::Effects,
            1,
        ),
        omega_checked_trees::AcceptanceCheck::accepted(
            omega_checked_trees::AcceptanceDimension::Boundaries,
            0,
        ),
        omega_checked_trees::AcceptanceCheck::not_applicable(
            omega_checked_trees::AcceptanceDimension::Termination,
        ),
    );

    assert_eq!(
        summary.verdict,
        omega_checked_trees::AcceptanceVerdict::Rejected
    );
    assert!(!summary.is_accepted());
    assert_eq!(summary.rejected_checks().count(), 1);
    assert_eq!(
        summary
            .rejected_checks()
            .next()
            .map(|check| check.dimension),
        Some(omega_checked_trees::AcceptanceDimension::Proof)
    );
}

fn parse_typed_trees(source: &str) -> omega_typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}
