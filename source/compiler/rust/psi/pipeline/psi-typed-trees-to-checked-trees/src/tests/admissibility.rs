use super::*;
use psi_checked_trees::AcceptanceView;

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
        psi_checked_trees::AcceptanceDimension::ALL.len()
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
            .check(psi_checked_trees::AcceptanceDimension::Borrow)
            .evidence_count,
        state_summary.borrow.evidence_count
    );
    assert!(state_summary.is_dimension_satisfied(psi_checked_trees::AcceptanceDimension::Borrow));
    assert_eq!(state_summary.borrow.diagnostic_count, 0);
    assert_eq!(
        state_summary.borrow.provenance,
        psi_checked_trees::AcceptanceCheckProvenance::AcceptedByEvidence
    );
    assert_eq!(state_summary.proof.evidence_count, 1);
    assert_eq!(state_acceptance.statements().len(), 1);
    assert_eq!(state_acceptance.calls().len(), 1);
    assert_eq!(state_acceptance.operations().count(), 2);

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
        psi_checked_trees::AcceptanceDimension::ALL.len()
    );
    assert_eq!(call_summary.rejected_checks().count(), 0);
    assert!(call_summary.borrow.evidence_count > 0);
    assert_eq!(call_summary.proof.evidence_count, 1);
    assert_eq!(
        call_summary.termination.verdict,
        psi_checked_trees::AcceptanceCheckVerdict::NotApplicable
    );
    assert_eq!(
        call_summary.termination.provenance,
        psi_checked_trees::AcceptanceCheckProvenance::NotRequired
    );
    assert!(!call_acceptance.entry_constraints().is_empty());
    assert!(!call_acceptance.requires_constraints().is_empty());
    assert_eq!(call_acceptance.requires().len(), 1);
    assert!(call_acceptance.boundary_edges().is_empty());

    let operations: Vec<_> = state_acceptance.operations().collect();
    assert_eq!(
        operations
            .iter()
            .map(psi_checked_trees::StateOperationAcceptance::kind)
            .collect::<Vec<_>>(),
        vec![
            psi_checked_trees::StateOperationAcceptanceKind::Statement,
            psi_checked_trees::StateOperationAcceptanceKind::Call,
        ]
    );
    assert!(operations.iter().all(|operation| operation.is_accepted()));
    assert!(operations[0].as_statement().is_some());
    assert!(operations[0].as_call().is_none());
    assert!(operations[1].as_call().is_some());
    assert_eq!(operations[0].statement_index(), 0);
    assert_eq!(operations[1].statement_index(), 0);
}

#[test]
fn acceptance_checks_have_diagnostic_provenance_shape_for_rejections() {
    let rejected = psi_checked_trees::AcceptanceCheck::rejected(
        psi_checked_trees::AcceptanceDimension::Borrow,
        2,
    );

    assert_eq!(
        rejected.verdict,
        psi_checked_trees::AcceptanceCheckVerdict::Rejected
    );
    assert_eq!(rejected.evidence_count, 0);
    assert_eq!(rejected.diagnostic_count, 2);
    assert_eq!(
        rejected.provenance,
        psi_checked_trees::AcceptanceCheckProvenance::RejectedByDiagnostic
    );
    assert!(!rejected.is_satisfied());

    let pending = psi_checked_trees::AcceptanceCheck::rejected(
        psi_checked_trees::AcceptanceDimension::Proof,
        0,
    );
    assert_eq!(
        pending.provenance,
        psi_checked_trees::AcceptanceCheckProvenance::DiagnosticPending
    );
}

#[test]
fn acceptance_summary_derives_rejection_from_dimension_records() {
    let summary = psi_checked_trees::AcceptanceSummary::with_checks(
        psi_checked_trees::AcceptanceCheck::accepted(
            psi_checked_trees::AcceptanceDimension::Borrow,
            3,
        ),
        psi_checked_trees::AcceptanceCheck::rejected(
            psi_checked_trees::AcceptanceDimension::Proof,
            1,
        ),
        psi_checked_trees::AcceptanceCheck::accepted(
            psi_checked_trees::AcceptanceDimension::ServiceReach,
            1,
        ),
        psi_checked_trees::AcceptanceCheck::accepted(
            psi_checked_trees::AcceptanceDimension::Suspension,
            0,
        ),
        psi_checked_trees::AcceptanceCheck::accepted(
            psi_checked_trees::AcceptanceDimension::Blocking,
            0,
        ),
        psi_checked_trees::AcceptanceCheck::accepted(
            psi_checked_trees::AcceptanceDimension::Boundaries,
            0,
        ),
        psi_checked_trees::AcceptanceCheck::not_applicable(
            psi_checked_trees::AcceptanceDimension::Termination,
        ),
    );

    assert_eq!(
        summary.verdict,
        psi_checked_trees::AcceptanceVerdict::Rejected
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
        Some(psi_checked_trees::AcceptanceDimension::Proof)
    );
}

#[test]
fn acceptance_dimensions_have_canonical_iteration_order_and_names() {
    let dimensions = psi_checked_trees::AcceptanceDimension::ALL;

    assert_eq!(dimensions.len(), 7);
    assert_eq!(
        dimensions.map(psi_checked_trees::AcceptanceDimension::as_str),
        [
            "borrow",
            "proof",
            "service_reach",
            "suspension",
            "blocking",
            "boundaries",
            "termination",
        ]
    );
}

#[test]
fn exposes_exit_acceptance_through_shared_view_surface() {
    let source = r#"
        data Player {
            health: i32;
        }

        domain Player::Alive
        requires
            self.health > 0;

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

    let operations: Vec<_> = state_acceptance.operations().collect();
    assert_eq!(operations.len(), 2);
    assert_eq!(
        operations
            .iter()
            .map(psi_checked_trees::StateOperationAcceptance::kind)
            .collect::<Vec<_>>(),
        vec![
            psi_checked_trees::StateOperationAcceptanceKind::Statement,
            psi_checked_trees::StateOperationAcceptanceKind::Exit,
        ]
    );
    assert!(operations[0].as_statement().is_some());
    assert!(operations[1].as_exit().is_some());
    assert_eq!(operations[1].summary().proof.evidence_count, 1);
}

#[test]
fn acceptance_views_publish_exact_state_owned_borrow_compatibility_certificates() {
    let source = r#"
        data Main { items: [i32; 4]; }
        data Other {}

        machine Main::split(&mut self) -> u64 {
            let mid: u64 = 2;
            let cut: u64 = mid;
            let left: &mut [i32] = self.items[0..cut];
            let right: &mut [i32] = self.items[mid..4];
            left.len + right.len
        }

        machine Other::idle(&mut self) {}
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("symbolic adjacency should retain one structural borrow certificate");
    let split = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::split")
        .expect("split machine");
    let split_state = checked.machine_states(split).first().expect("split state");
    let split_machine_symbol = split.symbol;
    let split_state_symbol = split_state.symbol;
    let split_acceptance = checked
        .state_acceptance(split_machine_symbol, split_state_symbol)
        .expect("split state acceptance");

    let certificates = split_acceptance
        .borrow_compatibility_certificates()
        .collect::<Vec<_>>();
    let [certificate] = certificates.as_slice() else {
        panic!("split state should publish exactly one compatibility certificate")
    };
    assert_eq!(certificate.formation.machine_symbol, split_machine_symbol);
    assert_eq!(certificate.formation.state_symbol, split_state_symbol);
    assert_eq!(certificate.formation.statement_index, 3);

    let forming_statement = split_acceptance
        .statement(3)
        .expect("forming statement acceptance");
    assert_eq!(
        forming_statement
            .borrow_compatibility_certificates()
            .count(),
        1
    );
    assert_eq!(
        split_acceptance
            .statement(2)
            .expect("sibling statement acceptance")
            .borrow_compatibility_certificates()
            .count(),
        0,
        "a certificate must not leak to a sibling statement"
    );

    let state_borrow_evidence = split_acceptance.summary().borrow.evidence_count;
    let statement_borrow_evidence = forming_statement.summary().borrow.evidence_count;
    let mut without_certificate = checked.clone();
    without_certificate
        .facts
        .borrow
        .compatibility_certificates
        .reset_retain_capacity();
    let without_state = without_certificate
        .state_acceptance(split_machine_symbol, split_state_symbol)
        .expect("split state acceptance without retained certificate");
    assert_eq!(
        state_borrow_evidence,
        without_state.summary().borrow.evidence_count + 1,
        "state acceptance should count the retained certificate exactly once"
    );
    assert_eq!(
        statement_borrow_evidence,
        without_state
            .statement(3)
            .expect("forming statement without retained certificate")
            .summary()
            .borrow
            .evidence_count
            + 1,
        "statement acceptance should count the retained certificate exactly once"
    );

    let idle = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Other::idle")
        .expect("idle machine");
    let idle_state = checked.machine_states(idle).first().expect("idle state");
    assert_eq!(
        checked
            .state_acceptance(idle.symbol, idle_state.symbol)
            .expect("idle state acceptance")
            .borrow_compatibility_certificates()
            .count(),
        0,
        "a certificate must not leak to another state"
    );
}

fn parse_typed_trees(source: &str) -> psi_typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

fn assert_acceptance_view_is_queryable(view: &impl psi_checked_trees::AcceptanceView) {
    let summary = view.summary();

    assert_eq!(view.verdict(), summary.verdict);
    assert_eq!(view.is_accepted(), summary.is_accepted());
    assert_eq!(
        view.check(psi_checked_trees::AcceptanceDimension::Borrow),
        summary.borrow
    );
    assert_eq!(
        view.is_dimension_satisfied(psi_checked_trees::AcceptanceDimension::Proof),
        summary.proof.is_satisfied()
    );
    assert_eq!(view.evidence_count(), summary.evidence_count());
    assert_eq!(view.diagnostic_count(), summary.diagnostic_count());
    assert_eq!(view.rejected_check_count(), summary.rejected_check_count());
    assert_eq!(view.has_diagnostics(), summary.has_diagnostics());
    assert_eq!(
        summary.checks().len(),
        psi_checked_trees::AcceptanceDimension::ALL.len()
    );
}
