//! The crash-obligation roster is untrusted evidence against questions the
//! verifier reconstructs from the module itself.
//!
//! A proof bundle's `crash_obligations` answer verifier-reconstructed `Site`
//! and `Continuation` questions: the roster must follow canonical owner
//! order, name exactly the reconstructed owners, and carry one coverage or
//! refutation slot per reconstructed goal. Each supplied node is replayed
//! through its recorded denotation lane under the question's own goal,
//! requirements and path axioms — no search runs on the receiving side, so
//! a producer-made certificate bound to one question cannot answer another,
//! a premise index cites the reconstructed roster, and a missing answer is
//! a missing answer rather than a removed path. These tests pin each
//! boundary refusal the roster contract names.

use proof_admission::{AdmissionProfile, PrimitiveJudgment, ProofNode, ProofRule};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, MachineId, OperationId, Proposition, PropositionContext,
    ScalarTerm, ScalarType, ValueId,
};
use terminal_psi::{
    Block, CrashCause, CrashCertificate, CrashObligationEvidence, CrashObligationOwner,
    CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard, MachineContract, Operation,
    OperationKind, OperationResult, SuccessorEdge, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{
    CrashObligationQuestion, ReconstructedCrashObligation, VerificationError,
    crash_obligation_discharged, verify_module,
};

fn value(identity: u64) -> ValueId {
    ValueId::new(identity).unwrap()
}

fn declaration(identity: u64) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id: value(identity),
        scalar_type: ScalarType::Boolean,
    }
}

fn boolean(identity: u64, expected: bool) -> Proposition {
    let mut terms = [
        ScalarTerm::value(value(identity), ScalarType::Boolean),
        ScalarTerm::boolean(expected),
    ];
    terms.sort();
    Proposition::Equal(terms[0].clone(), terms[1].clone())
}

fn crash(edge: u64, guards: Vec<Proposition>) -> Terminator {
    let mut site_guard = guards
        .into_iter()
        .map(CrashPredicateTerm::new)
        .collect::<Vec<_>>();
    site_guard.sort();
    Terminator::Crash {
        edge: EdgeId::new(edge).unwrap(),
        cause: CrashCause::Trap,
        site_guard,
        frontier_lower_bound: Vec::new(),
    }
}

fn block(identity: u64, terminator: Terminator) -> Block {
    Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: Vec::new(),
        id: BlockId::new(identity).unwrap(),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator,
    }
}

fn successor(edge: u64, target: u64) -> SuccessorEdge {
    SuccessorEdge {
        structural_arguments: Vec::new(),
        edge: EdgeId::new(edge).unwrap(),
        target: BlockId::new(target).unwrap(),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

fn machine(identity: u64) -> TerminalMachine {
    TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(identity).unwrap(),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(identity).unwrap(),
        blocks: vec![block(
            identity,
            Terminator::ReturnUnit {
                edge: EdgeId::new(identity).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        )],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            id: ContractId::new(identity).unwrap(),
            requires: Vec::new(),
            ensures: Vec::new(),
            crash_routes: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

fn module(machines: Vec<TerminalMachine>) -> TerminalModule {
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machines[0].id,
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines,
    }
}

/// Two crash sites on opposite arms of one conditional. Reconstruction asks
/// two `Site` questions in canonical owner order — `machine:1, block:2` and
/// `machine:1, block:3` — whose certificates differ in kind: the true arm's
/// guard is a path fact cited by `SemanticAxiom`, while the false arm's
/// guard names the second parameter and is provable only by citing the
/// machine's entry requirement as `Assumption`.
fn two_site_module() -> TerminalModule {
    let mut caller = machine(1);
    caller.parameters = vec![declaration(1), declaration(2)];
    caller.contract.requires = vec![boolean(2, true)];
    caller.contract.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    caller.blocks = vec![
        block(
            1,
            Terminator::Conditional {
                condition: value(1),
                when_true: successor(1, 2),
                when_false: successor(2, 3),
            },
        ),
        block(2, crash(3, vec![boolean(1, true)])),
        block(3, crash(4, vec![boolean(2, true)])),
    ];
    module(vec![caller])
}

/// A caller whose unit call forwards an uncovered `Trap` continuation from a
/// callee that crashes unconditionally. Reconstruction asks one
/// `Continuation` question whose coverage goal is the caller's published
/// predicate, dischargable only by citing the entry requirement.
fn continuation_module() -> TerminalModule {
    let mut caller = machine(1);
    caller.parameters = vec![declaration(1)];
    caller.contract.requires = vec![boolean(1, true)];
    caller.contract.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            boolean(1, true),
        ))],
    }];
    let mut callee = machine(2);
    callee.contract.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    callee.blocks[0].terminator = crash(2, Vec::new());
    caller.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            callee: callee.id,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: callee.contract.crash_routes.clone(),
        },
    });
    module(vec![caller, callee])
}

#[test]
fn the_produced_roster_discharges_each_reconstructed_question() {
    for module in [two_site_module(), continuation_module()] {
        let bundle = crate::support::produced_crash_bundle(&module);
        verify_module(&module, &bundle, &AdmissionProfile::default())
            .expect("the produced roster discharges the reconstructed questions");
    }
}

#[test]
fn roster_order_is_canonical_not_producer_chosen() {
    let module = two_site_module();
    let mut bundle = crate::support::produced_crash_bundle(&module);
    assert_eq!(bundle.crash_obligations.len(), 2);
    bundle.crash_obligations.reverse();
    assert!(
        matches!(
            verify_module(&module, &bundle, &AdmissionProfile::default()),
            Err(VerificationError::NonCanonicalCrashObligationEvidence)
        ),
        "a reordered roster must reject before its rows are examined"
    );
}

#[test]
fn a_duplicated_owner_row_rejects_as_noncanonical() {
    let module = two_site_module();
    let mut bundle = crate::support::produced_crash_bundle(&module);
    let duplicated = bundle.crash_obligations[0].clone();
    bundle.crash_obligations.push(duplicated);
    assert!(
        matches!(
            verify_module(&module, &bundle, &AdmissionProfile::default()),
            Err(VerificationError::NonCanonicalCrashObligationEvidence)
        ),
        "a repeated owner breaks the strictly increasing roster order"
    );
}

#[test]
fn a_surplus_owner_row_is_unknown_evidence() {
    let module = continuation_module();
    let mut bundle = crate::support::produced_crash_bundle(&module);
    let surplus = CrashObligationOwner::Continuation {
        machine: MachineId::new(9).unwrap(),
        operation: OperationId::new(9).unwrap(),
        cause: CrashCause::Trap,
    };
    bundle.crash_obligations.push(CrashObligationEvidence {
        owner: surplus,
        coverage: Vec::new(),
        refutation: Vec::new(),
    });
    assert!(
        matches!(
            verify_module(&module, &bundle, &AdmissionProfile::default()),
            Err(VerificationError::UnknownCrashObligationEvidence(owner)) if owner == surplus
        ),
        "evidence for a question the module never asks is unknown authority"
    );
}

#[test]
fn an_omitted_question_is_missing_evidence_not_an_erased_path() {
    let module = two_site_module();
    let mut bundle = crate::support::produced_crash_bundle(&module);
    let omitted = bundle
        .crash_obligations
        .pop()
        .expect("two reconstructed questions");
    assert!(
        matches!(
            verify_module(&module, &bundle, &AdmissionProfile::default()),
            Err(VerificationError::MissingCrashObligationEvidence(owner)) if owner == omitted.owner
        ),
        "the roster cannot remove a reconstructed question by leaving it out"
    );
}

#[test]
fn a_certificate_answers_only_the_question_it_was_produced_for() {
    let module = two_site_module();
    let mut bundle = crate::support::produced_crash_bundle(&module);
    // Moving each row's certificates onto the other owner keeps the roster
    // canonical and keeps every node producer-made; each is then replayed
    // under a different goal and premise roster than the one it proves. The
    // two questions' answers differ in kind here — a path-fact `SemanticAxiom`
    // citation and an entry-requirement `Assumption` citation — so neither
    // roster's nodes can cover the other's question.
    let (first, rest) = bundle.crash_obligations.split_at_mut(1);
    let second = &mut rest[0];
    std::mem::swap(&mut first[0].coverage, &mut second.coverage);
    std::mem::swap(&mut first[0].refutation, &mut second.refutation);
    let owner = first[0].owner;
    assert!(
        matches!(
            verify_module(&module, &bundle, &AdmissionProfile::default()),
            Err(VerificationError::RejectedCrashObligationEvidence { owner: rejected }) if rejected == owner
        ),
        "a node re-decided under a different reconstructed goal must reject"
    );
}

#[test]
fn a_forged_proof_node_is_rejected_not_repaired_by_search() {
    let module = two_site_module();
    let mut bundle = crate::support::produced_crash_bundle(&module);
    let owner = bundle.crash_obligations[0].owner;
    // Every lane's node is replaced by one claiming a proposition that is not
    // the goal: the verifier replays the supplied nodes exactly, and no
    // producer search on this side can substitute a working certificate.
    for certificate in &mut bundle.crash_obligations[0].coverage[0] {
        certificate.proof.conclusion = Proposition::Falsehood;
    }
    assert!(
        matches!(
            verify_module(&module, &bundle, &AdmissionProfile::default()),
            Err(VerificationError::RejectedCrashObligationEvidence { owner: rejected }) if rejected == owner
        ),
        "the verifier replays the supplied node; it never searches for a working one"
    );
}

#[test]
fn a_premise_index_binds_the_reconstructed_requirements() {
    let module = continuation_module();
    let owner = CrashObligationOwner::Continuation {
        machine: MachineId::new(1).unwrap(),
        operation: OperationId::new(1).unwrap(),
        cause: CrashCause::Trap,
    };
    // The produced certificates cite `Assumption { index: 0 }` — the caller's
    // only entry requirement. Reindexing every lane's node names a premise
    // the reconstructed question never established.
    let mut bundle = crate::support::produced_crash_bundle(&module);
    for certificate in &mut bundle.crash_obligations[0].coverage[0] {
        certificate.proof.rule = ProofRule::Assumption { index: 7 };
    }
    assert!(
        matches!(
            verify_module(&module, &bundle, &AdmissionProfile::default()),
            Err(VerificationError::RejectedCrashObligationEvidence { owner: rejected }) if rejected == owner
        ),
        "premise indices resolve against the reconstructed requirements, not producer claims"
    );
    // The same holds for the reconstructed axiom roster: continuation
    // questions cite no reconstructed control-flow facts, so no axiom index
    // is in scope at all.
    let mut bundle = crate::support::produced_crash_bundle(&module);
    for certificate in &mut bundle.crash_obligations[0].coverage[0] {
        certificate.proof.rule = ProofRule::SemanticAxiom { index: 0 };
    }
    assert!(
        matches!(
            verify_module(&module, &bundle, &AdmissionProfile::default()),
            Err(VerificationError::RejectedCrashObligationEvidence { owner: rejected }) if rejected == owner
        ),
        "a continuation question supplies no reconstructed axiom roster to cite"
    );
}

#[test]
fn roster_slot_arity_matches_the_reconstructed_row_shape() {
    let module = continuation_module();
    let mut bundle = crate::support::produced_crash_bundle(&module);
    let owner = bundle.crash_obligations[0].owner;
    bundle.crash_obligations[0].coverage.clear();
    assert!(
        matches!(
            verify_module(&module, &bundle, &AdmissionProfile::default()),
            Err(VerificationError::RejectedCrashObligationEvidence { owner: rejected }) if rejected == owner
        ),
        "a coverage roster shorter than the reconstructed goal count rejects"
    );
}

fn site_question(
    guards: Vec<Proposition>,
    paths: Vec<Vec<Proposition>>,
) -> ReconstructedCrashObligation {
    ReconstructedCrashObligation {
        owner: CrashObligationOwner::Site {
            machine: MachineId::new(1).unwrap(),
            block: BlockId::new(2).unwrap(),
            edge: EdgeId::new(3).unwrap(),
        },
        context: Some(PropositionContext::default()),
        requirements: Vec::new(),
        question: CrashObligationQuestion::Site { guards, paths },
    }
}

fn truth_certificate() -> CrashCertificate {
    CrashCertificate {
        with_value_equalities: false,
        proof: ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        },
    }
}

fn site_evidence(
    owner: CrashObligationOwner,
    coverage: Vec<Vec<CrashCertificate>>,
    refutation: Vec<Vec<CrashCertificate>>,
) -> CrashObligationEvidence {
    CrashObligationEvidence {
        owner,
        coverage,
        refutation,
    }
}

#[test]
fn a_site_with_no_reconstructed_path_is_not_vacuously_covered() {
    let question = site_question(vec![Proposition::Truth], Vec::new());
    let evidence = site_evidence(question.owner, vec![vec![truth_certificate()]], Vec::new());
    assert!(
        !crash_obligation_discharged(&question, &evidence),
        "a missing reconstructed path is exactly what proof supply must not erase"
    );
}

#[test]
fn a_supplied_certificate_discharges_only_at_exact_slot_arity() {
    let question = site_question(vec![Proposition::Truth], vec![Vec::new()]);
    let short_coverage = site_evidence(question.owner, Vec::new(), vec![Vec::new()]);
    assert!(
        !crash_obligation_discharged(&question, &short_coverage),
        "one coverage roster per asserted guard is canonical row shape"
    );
    let short_refutation =
        site_evidence(question.owner, vec![vec![truth_certificate()]], Vec::new());
    assert!(
        !crash_obligation_discharged(&question, &short_refutation),
        "one refutation roster per reconstructed path is canonical row shape"
    );
    let discharged = site_evidence(
        question.owner,
        vec![vec![truth_certificate()]],
        vec![Vec::new()],
    );
    assert!(
        crash_obligation_discharged(&question, &discharged),
        "at exact arity a kernel-checked node discharges the question"
    );
}

#[test]
fn a_continuation_row_is_covered_by_any_one_published_goal() {
    let owner = CrashObligationOwner::Continuation {
        machine: MachineId::new(1).unwrap(),
        operation: OperationId::new(2).unwrap(),
        cause: CrashCause::Trap,
    };
    let question = ReconstructedCrashObligation {
        owner,
        context: Some(PropositionContext::default()),
        requirements: Vec::new(),
        question: CrashObligationQuestion::Continuation {
            coverage_goals: vec![Proposition::Falsehood, Proposition::Truth],
            refutation_goal: None,
        },
    };
    // Each published bucket is an independent cover: proving one coverage
    // goal discharges the row, an empty roster never does.
    let unproved = site_evidence(owner, vec![Vec::new(), Vec::new()], Vec::new());
    assert!(!crash_obligation_discharged(&question, &unproved));
    let covered = site_evidence(
        owner,
        vec![Vec::new(), vec![truth_certificate()]],
        Vec::new(),
    );
    assert!(crash_obligation_discharged(&question, &covered));
}

#[test]
fn a_question_without_context_accepts_no_certificate() {
    let question = ReconstructedCrashObligation {
        owner: CrashObligationOwner::Continuation {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(2).unwrap(),
            cause: CrashCause::Trap,
        },
        context: None,
        requirements: Vec::new(),
        question: CrashObligationQuestion::Continuation {
            coverage_goals: vec![Proposition::Truth],
            refutation_goal: None,
        },
    };
    let supplied = site_evidence(question.owner, vec![vec![truth_certificate()]], Vec::new());
    assert!(
        !crash_obligation_discharged(&question, &supplied),
        "a malformed entry context keeps the obligation undischarged by construction"
    );
}
