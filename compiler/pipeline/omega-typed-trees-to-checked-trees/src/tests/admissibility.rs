use super::*;
use omega_checked_trees::AcceptanceView;

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

    assert_acceptance_view_is_queryable(&state_acceptance);
    assert!(state_acceptance.is_accepted());
    let state_summary = state_acceptance.summary();
    assert!(state_summary.is_accepted());
    assert_eq!(
        state_summary.checks().len(),
        omega_checked_trees::AcceptanceDimension::ALL.len()
    );
    assert_eq!(state_summary.rejected_checks().count(), 0);
    assert_eq!(state_summary.rejected_check_count(), 0);
    assert_eq!(state_acceptance.rejected_check_count(), 0);
    assert_eq!(state_acceptance.diagnostic_count(), 0);
    assert!(!state_acceptance.has_diagnostics());
    assert!(state_acceptance.evidence_count() > 0);
    assert!(state_summary.borrow.evidence_count > 0);
    assert_eq!(
        state_summary
            .check(omega_checked_trees::AcceptanceDimension::Borrow)
            .evidence_count,
        state_summary.borrow.evidence_count
    );
    assert!(state_summary.is_dimension_satisfied(omega_checked_trees::AcceptanceDimension::Borrow));
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
    assert_acceptance_view_is_queryable(&statement_acceptance);
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

    assert_acceptance_view_is_queryable(&call_acceptance);
    assert!(call_acceptance.is_accepted());
    let call_summary = call_acceptance.summary();
    assert!(call_summary.is_accepted());
    assert_eq!(
        call_summary.checks().len(),
        omega_checked_trees::AcceptanceDimension::ALL.len()
    );
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
    assert_eq!(summary.evidence_count(), 4);
    assert_eq!(summary.diagnostic_count(), 1);
    assert!(summary.has_diagnostics());
    assert_eq!(summary.rejected_check_count(), 1);
    assert_eq!(summary.rejected_checks().count(), 1);
    assert_eq!(
        summary
            .rejected_checks()
            .next()
            .map(|check| check.dimension),
        Some(omega_checked_trees::AcceptanceDimension::Proof)
    );
}

#[test]
fn acceptance_dimensions_have_canonical_iteration_order_and_names() {
    let dimensions = omega_checked_trees::AcceptanceDimension::ALL;

    assert_eq!(dimensions.len(), 5);
    assert_eq!(
        dimensions.map(omega_checked_trees::AcceptanceDimension::as_str),
        ["borrow", "proof", "effects", "boundaries", "termination"]
    );
}

#[test]
fn exposes_exit_acceptance_through_shared_view_surface() {
    let source = r#"
        data Player {
            health: i32;
        }

        domain Player::Alive {
            self.health > 0;
        }

        data Main {
            player: Player;
        }

        machine Main::main(&mut self) -> i32
        requires
            self.player in Player::Alive
        ensures
            self.player in Player::Alive
        {
            0
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
    let exit_acceptance = state_acceptance
        .exit(0)
        .expect("terminal expression exit acceptance should be queryable");

    assert_acceptance_view_is_queryable(&exit_acceptance);
    assert!(exit_acceptance.is_accepted());
    assert_eq!(exit_acceptance.ensures().len(), 1);
    assert_eq!(exit_acceptance.summary().proof.evidence_count, 1);
}

fn parse_typed_trees(source: &str) -> omega_typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

fn assert_acceptance_view_is_queryable(view: &impl omega_checked_trees::AcceptanceView) {
    let summary = view.summary();

    assert_eq!(view.verdict(), summary.verdict);
    assert_eq!(view.is_accepted(), summary.is_accepted());
    assert_eq!(
        view.check(omega_checked_trees::AcceptanceDimension::Borrow),
        summary.borrow
    );
    assert_eq!(
        view.is_dimension_satisfied(omega_checked_trees::AcceptanceDimension::Proof),
        summary.proof.is_satisfied()
    );
    assert_eq!(view.evidence_count(), summary.evidence_count());
    assert_eq!(view.diagnostic_count(), summary.diagnostic_count());
    assert_eq!(view.rejected_check_count(), summary.rejected_check_count());
    assert_eq!(view.has_diagnostics(), summary.has_diagnostics());
    assert_eq!(
        summary.checks().len(),
        omega_checked_trees::AcceptanceDimension::ALL.len()
    );
}
