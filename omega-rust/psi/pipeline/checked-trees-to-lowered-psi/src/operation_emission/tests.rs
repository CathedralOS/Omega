use super::*;

fn fixture(obligation_count: usize) -> LoweredPsi {
    let mut lowered = lower_source("machine root(value: u64) -> u64 { value }");
    lowered.semantic_module.machines[0].contract.ensures = (0..obligation_count)
        .map(|position| terminal_psi::ContractClause {
            obligation: obligation_id(1_000_000 + position as u64),
            proposition: Proposition::Truth,
        })
        .collect();
    lowered.proof_bundle.evidence.clear();
    lowered
}

fn lower_source(source: &str) -> LoweredPsi {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    lower_machine(&checked, "root").unwrap()
}

// Retain the previous per-obligation preparation as a same-process reference.
// The fixture uses contract clauses so each pending row takes the canonical path.
fn uncached_reference(lowered: &mut LoweredPsi) -> usize {
    let validated = terminal_verifier::validate_module(&lowered.semantic_module).unwrap();
    let obligations =
        terminal_verifier::reconstruct_terminal_obligations(&lowered.semantic_module).unwrap();
    assert_eq!(
        obligations,
        terminal_verifier::reconstruct_execution_terminal_obligations(validated).unwrap()
    );
    let mut preparations = 0;
    for site in obligations.obligations() {
        if lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == site.obligation.id)
        {
            continue;
        }
        let machine = validated.machine(site.owner.machine()).unwrap();
        let context = validated.value_context(machine).unwrap();
        let parameters = machine
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect();
        preparations += 1;
        let proof = produce_checked_canonical_integer_proof(
            &context,
            &site.obligation.proposition,
            &site.requirements,
            &site.semantic_axioms,
            &parameters,
        )
        .unwrap();
        lowered.proof_bundle.evidence.push(ObligationEvidence {
            obligation: site.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(site.obligation.id.get()).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        });
    }
    lowered
        .proof_bundle
        .evidence
        .sort_by_key(|evidence| evidence.obligation);
    crate::control_cycle_proofs::finalize(lowered).unwrap();
    preparations
}

#[test]
fn preparation_is_once_per_demanded_machine_and_evidence_matches_uncached() {
    for obligation_count in [
        0,
        1,
        PARALLEL_PROOF_THRESHOLD - 1,
        PARALLEL_PROOF_THRESHOLD,
        64,
    ] {
        let mut lowered = fixture(obligation_count);
        let mut reference = lowered.clone();
        let preparations = AtomicUsize::new(0);
        finalize_operation_proofs_inner(&mut lowered, &|_| {
            preparations.fetch_add(1, Ordering::Relaxed);
        })
        .unwrap();
        assert_eq!(uncached_reference(&mut reference), obligation_count);
        assert_eq!(
            preparations.load(Ordering::Relaxed),
            usize::from(obligation_count != 0)
        );
        assert_eq!(lowered, reference);
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap();
    }
}

#[test]
fn retained_evidence_needs_no_preparation_and_mutated_proof_still_rejects() {
    let mut lowered = fixture(17);
    uncached_reference(&mut lowered);
    let original = lowered.clone();
    finalize_operation_proofs_inner(&mut lowered, &|_| panic!("no pending evidence")).unwrap();
    assert_eq!(lowered, original);

    let EvidenceRoute::CertificateDerived(certificate) =
        &mut lowered.proof_bundle.evidence[0].route
    else {
        panic!("fixture certificate");
    };
    certificate.proof.conclusion = Proposition::Falsehood;
    let corrupted = lowered.proof_bundle.clone();
    finalize_operation_proofs_inner(&mut lowered, &|_| {
        panic!("retained evidence is not regenerated")
    })
    .unwrap();
    assert_eq!(lowered.proof_bundle, corrupted);
    assert!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err()
    );
}

#[test]
fn invalid_module_rejects_before_preparation() {
    let mut lowered = fixture(17);
    lowered.semantic_module.machines[0].entry = block_id(u64::MAX);
    assert!(matches!(
        finalize_operation_proofs_inner(&mut lowered, &|_| panic!("invalid module")),
        Err(LoweringError::InvalidTerminalModule(_))
    ));
}

#[test]
fn supplied_source_evidence_is_preserved_without_preparation() {
    let mut lowered = lower_source(
        r#"
        machine root(value: bool) -> bool
        requires true == true
        ensures true == true
        { value }
        "#,
    );
    assert!(!lowered.proof_bundle.evidence.is_empty());
    let original = lowered.clone();
    finalize_operation_proofs_inner(&mut lowered, &|_| {
        panic!("source evidence already supplied")
    })
    .unwrap();
    assert_eq!(lowered, original);
}

#[test]
fn different_machines_prepare_their_own_context_once() {
    let mut lowered = lower_source(
        r#"
        machine helper(other: u64) -> u64 { other }
        machine root(value: u64) -> u64 { helper(value) }
        "#,
    );
    assert_eq!(lowered.semantic_module.machines.len(), 2);
    let preparations = lowered
        .semantic_module
        .machines
        .iter()
        .map(|machine| (machine.id, AtomicUsize::new(0)))
        .collect::<BTreeMap<_, _>>();
    let mut next_obligation = 1_000_000;
    for machine in &mut lowered.semantic_module.machines {
        // Cite each machine's own parameter: sharing another machine's context
        // would fail the canonical proof kernel's value lookup.
        let parameter = machine.parameters[0];
        let term = semantic_vocabulary::ScalarTerm::value(parameter.id, parameter.scalar_type);
        machine.contract.ensures = (0..17)
            .map(|_| {
                let obligation = obligation_id(next_obligation);
                next_obligation += 1;
                terminal_psi::ContractClause {
                    obligation,
                    proposition: Proposition::Equal(term.clone(), term.clone()),
                }
            })
            .collect();
    }
    lowered.proof_bundle.evidence.clear();
    let mut reference = lowered.clone();
    finalize_operation_proofs_inner(&mut lowered, &|machine| {
        preparations[&machine].fetch_add(1, Ordering::Relaxed);
    })
    .unwrap();
    assert!(
        preparations
            .values()
            .all(|count| count.load(Ordering::Relaxed) == 1)
    );
    assert_eq!(uncached_reference(&mut reference), 34);
    assert_eq!(lowered, reference);
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
}

#[test]
fn parallel_failures_keep_the_first_pending_obligation_and_publish_nothing() {
    let mut lowered = fixture(17);
    for position in [3, 12] {
        lowered.semantic_module.machines[0].contract.ensures[position].proposition =
            Proposition::Falsehood;
    }
    let first = lowered.semantic_module.machines[0].contract.ensures[3].obligation;
    let original = lowered.proof_bundle.clone();
    assert!(
        matches!(finalize_operation_proofs(&mut lowered), Err(LoweringError::OperationProofUnavailable(obligation)) if obligation == first)
    );
    assert_eq!(lowered.proof_bundle, original);
}
