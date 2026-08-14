use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal_codec::{decode_module, encode_module, semantic_fingerprint};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const FORWARDED_SOURCE: &str = r#"
    trait Evidence<T> {
        machine witness(value: T);
    }

    proposition ready<T>() evidence Evidence<T>;

    data Token { value: u64; }
    data Root {}
    machine Root::forward(first: Token, second: Token)
    requires
        incoming: ready<i32>()
    ensures
        outgoing: ready<i32>()
    {
        outgoing = incoming;
    }
"#;

const PRODUCED_SOURCE: &str = r#"
    trait Evidence<T> {
        machine witness(value: T);
    }

    proposition ready<T>() evidence Evidence<T>;

    ConcreteEvidence: satisfies Evidence<i32> {
        machine witness(value: i32) {}
    }

    data Token { value: u64; }
    data Root {}
    machine Root::produce(first: Token, second: Token)
    ensures
        outgoing: ready<i32>()
    {
        outgoing = ConcreteEvidence;
    }
"#;

#[test]
fn source_forwarding_preserves_one_exact_terminal_evidence_identity() {
    let checked = check(FORWARDED_SOURCE);
    assert_eq!(checked.facts.proof.evidence_terms.len(), 2);
    let expected_argument = checked
        .facts
        .proof
        .evidence_terms
        .iter()
        .next()
        .and_then(|(_, term)| term.evidence_interface.as_ref())
        .and_then(|interface| interface.arguments.first())
        .expect("the checked endpoint has one exact instantiated type argument")
        .as_str()
        .to_owned();

    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::forward")
        .expect("forwarded witness identity should cross terminal Psi");
    assert_eq!(lowered.semantic_module.evidence_terms.len(), 1);
    let term = &lowered.semantic_module.evidence_terms[0];
    assert_eq!(term.interface.trait_identity, "Evidence");
    assert_eq!(term.interface.arguments, vec![expected_argument]);
    assert_eq!(
        term.proposition,
        lowered.semantic_module.proposition_applications[0].id
    );
    assert_eq!(
        lowered.semantic_module.proposition_applications[0]
            .evidence_interface
            .as_ref(),
        Some(&term.interface)
    );

    let bytes = encode_module(&lowered.semantic_module).expect("terminal evidence should encode");
    assert_eq!(
        decode_module(&bytes).expect("terminal evidence should decode"),
        lowered.semantic_module
    );
    let baseline = semantic_fingerprint(&lowered.semantic_module).expect("terminal identity");

    let mut drifted_term = lowered.semantic_module.clone();
    drifted_term.evidence_terms[0].interface.trait_identity = "OtherEvidence".to_owned();
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&drifted_term),
        Err(psi_terminal_verifier::ModuleError::EvidenceTermInterfaceMismatch(_))
    ));

    let mut changed = lowered.semantic_module.clone();
    changed.evidence_terms[0].interface.trait_identity = "OtherEvidence".to_owned();
    changed.proposition_applications[0]
        .evidence_interface
        .as_mut()
        .expect("witness application interface")
        .trait_identity = "OtherEvidence".to_owned();
    assert_ne!(
        semantic_fingerprint(&changed).expect("changed terminal identity"),
        baseline,
        "the structured exact interface, not its diagnostic spelling, enters identity"
    );

    let mut unresolved = lowered.semantic_module.clone();
    unresolved.evidence_terms[0]
        .interface
        .trait_identity
        .clear();
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&unresolved),
        Err(psi_terminal_verifier::ModuleError::InvalidEvidenceInterface(_))
    ));

    let mut malformed_application = lowered.semantic_module.clone();
    malformed_application.proposition_applications[0]
        .evidence_interface
        .as_mut()
        .expect("witness application interface")
        .trait_identity
        .clear();
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&malformed_application),
        Err(psi_terminal_verifier::ModuleError::InvalidPropositionEvidenceInterface(_))
    ));

    let mut fact_only_application = lowered.semantic_module.clone();
    fact_only_application.proposition_declarations[0].evidence =
        psi_terminal::PropositionEvidence::FactOnly;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&fact_only_application),
        Err(psi_terminal_verifier::ModuleError::InvalidPropositionEvidenceInterface(_))
    ));

    fact_only_application.proposition_applications[0].evidence_interface = None;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&fact_only_application),
        Err(psi_terminal_verifier::ModuleError::FactOnlyEvidenceTerm(_))
    ));

    let mut unknown_proposition = lowered.semantic_module.clone();
    unknown_proposition.evidence_terms[0].proposition =
        psi_core::PropositionId::new(99).expect("test proposition identity");
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&unknown_proposition),
        Err(psi_terminal_verifier::ModuleError::UnknownEvidenceTermProposition(_))
    ));
}

#[test]
fn source_producer_provenance_remains_fail_closed_at_terminal_boundary() {
    let checked = check(PRODUCED_SOURCE);
    let error = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::produce")
        .expect_err("producer provenance has no terminal proof-bundle row yet");
    assert!(
        matches!(
            error,
            psi_checked_trees_to_terminal::LoweringError::Unsupported(
                "terminal evidence producer provenance is not yet serialized separately"
            )
        ),
        "unexpected terminal fence: {error:?}"
    );
}

fn check(source: &str) -> psi_checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}
