use psi_core::{IntegerSign, IntegerType, IntegerValue, ScalarTerm};
use psi_proof_admission::{AdmissionProfile, EvidenceRoute, ProofRule};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::OperationKind;
use psi_terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use psi_terminal_fixed_fuel::{derive_fixed_entry_fuel, validate_fixed_entry_fuel};
use psi_terminal_interpreter::{
    AcceptTerminalEffects, TerminalExecutionResult, TerminalScalarValue, TerminalStructuralValue,
    interpret_terminal_artifact_with_effect_handler_measured,
};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}
    data Token {}
    machine Token::drop(&mut self) { Helper::touch(); }
    data Root {}
    machine Root::enter(token: Token, root: i8) -> bool
    requires 1i8 <= root
    {
        root < 1i8 || (6i8 / (root * 1i8)) <= 6i8
    }
"#;

const SIX_DEFINITION_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}
    data Token {}
    machine Token::drop(&mut self) { Helper::touch(); }
    data Root {}
    machine Root::divide_after_six_definitions(token: Token, root: i8) -> bool
    requires -5i8 <= root, root <= 121i8
    {
        let first: i8 = root + 1i8;
        let second: i8 = first + 1i8;
        let third: i8 = second + 1i8;
        let fourth: i8 = third + 1i8;
        let fifth: i8 = fourth + 1i8;
        let divisor: i8 = fifth + 1i8;
        let quotient: i8 = 6i8 / divisor;
        quotient <= 6i8
    }
"#;

const AFFINE_CAST_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}
    data Token {}
    machine Token::drop(&mut self) { Helper::touch(); }
    data Root {}
    machine Root::divide(token: Token, root: i16) -> bool
    requires root <= 32766i16, -129i16 <= root, root <= 126i16, 0i16 <= root
    {
        let affine: i16 = root + 1i16;
        let divisor: i8 = affine as i8;
        let quotient: i8 = 6i8 / divisor;
        let remainder: i8 = 6i8 % divisor;
        quotient == 2i8 && remainder == 0i8
    }
"#;

const CAST_AFFINE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}
    data Token {}
    machine Token::drop(&mut self) { Helper::touch(); }
    data Root {}
    machine Root::divide_after_cast(token: Token, root: i16) -> bool
    requires -128i16 <= root, root <= 127i16, -129i16 <= root,
        root <= 126i16, 0i16 <= root
    {
        let casted: i8 = root as i8;
        let divisor: i8 = casted + 1i8;
        let quotient: i8 = 6i8 / divisor;
        let remainder: i8 = 6i8 % divisor;
        quotient == 2i8 && remainder == 0i8
    }
"#;

const AFFINE_CAST_AFFINE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}
    data Token {}
    machine Token::drop(&mut self) { Helper::touch(); }
    data Root {}
    machine Root::divide_across_cast(token: Token, root: i16) -> bool
    requires root <= 32766i16, -129i16 <= root, root <= 126i16,
        root <= 125i16, -1i16 <= root
    {
        let before: i16 = root + 1i16;
        let casted: i8 = before as i8;
        let divisor: i8 = casted + 1i8;
        let quotient: i8 = 6i8 / divisor;
        let remainder: i8 = 6i8 % divisor;
        quotient == 2i8 && remainder == 0i8
    }
"#;

#[test]
fn landed_affine_sibling_custody_crosses_source_codec_and_independent_verification() {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("the landed affine sibling completes the source certificate");

    let exact_divide = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerDivide { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("source retains one exact divide");
    let evidence = lowered
        .proof_bundle
        .evidence
        .iter()
        .find(|evidence| evidence.obligation == exact_divide)
        .expect("exact divide has evidence");
    let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
        panic!("the exact divide uses the canonical certificate route")
    };
    let ProofRule::DisjunctionIntroduction { disjunct, index: 1 } = &certificate.proof.rule else {
        panic!("the nonnegative root selects the signed positive-divisor arm")
    };
    let ProofRule::IntegerAffineBound { witness, .. } = &disjunct.rule else {
        panic!("the positive-divisor arm retains its affine custody")
    };
    assert_eq!(witness.definition_axioms.len(), 1);
    assert_eq!(witness.literal_axioms.len(), 1);
    assert!(witness.literal_axioms[0].is_some());

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("independent verification replays the landed sibling");

    let module_bytes = encode_module(&lowered.semantic_module).expect("encode module");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("encode proof bundle");
    let decoded_module = decode_module(&module_bytes).expect("decode module");
    let decoded_proof = decode_proof_bundle(&proof_bytes).expect("decode proof bundle v19");
    assert_eq!(decoded_module, lowered.semantic_module);
    assert_eq!(decoded_proof, lowered.proof_bundle);

    let mut stale = decoded_proof;
    let stale_evidence = stale
        .evidence
        .iter_mut()
        .find(|evidence| evidence.obligation == exact_divide)
        .expect("decoded exact-divide evidence");
    let EvidenceRoute::CertificateDerived(certificate) = &mut stale_evidence.route else {
        unreachable!("selected certificate-derived evidence")
    };
    let ProofRule::DisjunctionIntroduction { disjunct, .. } = &mut certificate.proof.rule else {
        unreachable!("selected signed disjunction proof")
    };
    let ProofRule::IntegerAffineBound { witness, .. } = &mut disjunct.rule else {
        unreachable!("selected affine child")
    };
    witness.literal_axioms[0] = None;
    assert!(
        psi_terminal_verifier::verify_module(
            &decoded_module,
            &stale,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "removing the landed-sibling citation invalidates the certificate",
    );
}

#[test]
fn six_definition_affine_divisor_crosses_source_codec_and_independent_verification() {
    let tokens = Lexer::new(SIX_DEFINITION_SOURCE)
        .tokenize()
        .expect("tokenize six-definition source");
    let syntax = parse_syntax_trees(&tokens).expect("parse six-definition source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve six-definition source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type six-definition affine divisor");
    let checked = lower_typed_trees(typed).expect("check six-definition affine divisor");
    let lowered = psi_checked_trees_to_terminal::lower_machine(
        &checked,
        "Root::divide_after_six_definitions",
    )
    .expect("six-definition affine divisor lowers with a checked certificate");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [root] = entry.parameters.as_slice() else {
        panic!("entry retains one scalar root")
    };
    let (exact_divide, exact_divisor) = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerDivide {
                right, obligation, ..
            } => Some((obligation, right)),
            _ => None,
        })
        .expect("source retains one exact divide");
    let evidence = lowered
        .proof_bundle
        .evidence
        .iter()
        .find(|evidence| evidence.obligation == exact_divide)
        .expect("six-definition exact divide has evidence");
    let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
        panic!("the exact divide uses the canonical certificate route")
    };
    let ProofRule::DisjunctionIntroduction { disjunct, index: 1 } = &certificate.proof.rule else {
        panic!("the six-definition divisor selects the signed positive-divisor arm")
    };
    let ProofRule::IntegerAffineBound { witness, .. } = &disjunct.rule else {
        panic!("the positive-divisor arm retains its affine custody")
    };
    assert_eq!(witness.root, ScalarTerm::value(root.id, root.scalar_type),);
    assert_eq!(
        witness.target,
        ScalarTerm::value(exact_divisor, root.scalar_type),
    );
    assert_eq!(witness.definition_axioms, vec![1, 3, 5, 7, 9, 11]);
    assert_eq!(
        witness.literal_axioms,
        vec![Some(0), Some(2), Some(4), Some(6), Some(8), Some(10)],
    );

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("independent verification replays all six affine definitions");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("the six-definition entry has fixed fuel");
    validate_fixed_entry_fuel(&verified, &fixed)
        .expect("the six-definition fixed-fuel ceiling independently recomputes");
    drop(verified);

    let module_bytes = encode_module(&lowered.semantic_module).expect("encode module");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("encode proof bundle");
    let decoded_module = decode_module(&module_bytes).expect("decode module");
    let decoded_proof = decode_proof_bundle(&proof_bytes).expect("decode proof bundle v19");
    assert_eq!(decoded_module, lowered.semantic_module);
    assert_eq!(decoded_proof, lowered.proof_bundle);

    let [token] = entry.structural_parameters.as_slice() else {
        panic!("entry retains the Token cleanup root")
    };
    let structural_arguments = [TerminalStructuralValue {
        opaque_identity: token.place.get(),
        structural_type: token.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    }];
    let scalar_arguments = [TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Signed, 8).expect("i8"),
        value: IntegerValue::Signed(0),
    }];
    let mut handler = AcceptTerminalEffects;
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &module_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &scalar_arguments,
        &structural_arguments,
        &mut handler,
    )
    .expect("verified six-definition artifact interprets");
    assert_eq!(
        measured.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true)),
    );
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());
    assert!(measured.effects().is_empty());

    let mut reordered = decoded_proof;
    let reordered_evidence = reordered
        .evidence
        .iter_mut()
        .find(|evidence| evidence.obligation == exact_divide)
        .expect("decoded six-definition exact-divide evidence");
    let EvidenceRoute::CertificateDerived(certificate) = &mut reordered_evidence.route else {
        unreachable!("selected certificate-derived evidence")
    };
    let ProofRule::DisjunctionIntroduction { disjunct, .. } = &mut certificate.proof.rule else {
        unreachable!("selected signed disjunction proof")
    };
    let ProofRule::IntegerAffineBound { witness, .. } = &mut disjunct.rule else {
        unreachable!("selected affine child")
    };
    witness.definition_axioms.swap(4, 5);
    assert!(
        psi_terminal_verifier::verify_module(
            &decoded_module,
            &reordered,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "reordering the final two definition rows invalidates the certificate",
    );
}

#[test]
fn affine_to_partial_cast_exact_division_and_remainder_cross_source_codec_verification_and_interpretation()
 {
    let tokens = Lexer::new(AFFINE_CAST_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::divide")
        .expect("affine-to-partial-cast divisor lowers from real source");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let exact_obligations = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerDivide { obligation, .. }
            | OperationKind::ExactIntegerRemainder { obligation, .. } => Some(obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(exact_obligations.len(), 2, "one divide and one remainder");
    for obligation in &exact_obligations {
        let evidence = lowered
            .proof_bundle
            .evidence
            .iter()
            .find(|evidence| evidence.obligation == *obligation)
            .expect("exact divide/remainder has evidence");
        let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
            panic!("affine-cast exact divide/remainder is certificate-derived")
        };
        let ProofRule::DisjunctionIntroduction { disjunct, index: 1 } = &certificate.proof.rule
        else {
            panic!("signed positive-divisor arm is selected")
        };
        let ProofRule::IntegerCastBound {
            root_bound: affine_bound,
            witness: cast_witness,
        } = &disjunct.rule
        else {
            panic!("partial-cast custody is the outer proof")
        };
        let ProofRule::IntegerAffineBound {
            witness: affine_witness,
            ..
        } = &affine_bound.rule
        else {
            panic!("affine custody is independently retained below the cast")
        };
        assert_eq!(cast_witness.definition_axioms.len(), 1);
        assert_eq!(affine_witness.definition_axioms.len(), 1);
        assert_eq!(affine_witness.literal_axioms.len(), 1);
        assert!(affine_witness.literal_axioms[0].is_some());
    }
    let exact_divide = exact_obligations[0];

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("independent verifier reconstructs affine then cast custody");
    let module_bytes = encode_module(&lowered.semantic_module).expect("encode module");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("encode proof bundle");
    assert_eq!(
        decode_module(&module_bytes).expect("decode module"),
        lowered.semantic_module,
    );
    assert_eq!(
        decode_proof_bundle(&proof_bytes).expect("decode proof bundle"),
        lowered.proof_bundle,
    );

    let mut stale = decode_proof_bundle(&proof_bytes).expect("decode proof for mutation");
    let stale_evidence = stale
        .evidence
        .iter_mut()
        .find(|evidence| evidence.obligation == exact_divide)
        .expect("decoded exact-divide evidence");
    let EvidenceRoute::CertificateDerived(certificate) = &mut stale_evidence.route else {
        unreachable!("selected certificate-derived evidence")
    };
    let ProofRule::DisjunctionIntroduction { disjunct, .. } = &mut certificate.proof.rule else {
        unreachable!("selected signed disjunction proof")
    };
    let ProofRule::IntegerCastBound {
        root_bound,
        witness: cast_witness,
    } = &mut disjunct.rule
    else {
        unreachable!("selected cast proof")
    };
    let ProofRule::IntegerAffineBound {
        witness: affine_witness,
        ..
    } = &root_bound.rule
    else {
        unreachable!("selected affine child")
    };
    cast_witness.definition_axioms[0] = affine_witness.definition_axioms[0];
    assert!(
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &stale,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "a cast witness redirected to the affine definition rejects",
    );

    let [token] = entry.structural_parameters.as_slice() else {
        panic!("entry retains the Token cleanup root")
    };
    let structural_arguments = [TerminalStructuralValue {
        opaque_identity: token.place.get(),
        structural_type: token.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    }];
    let scalar_arguments = [TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16"),
        value: IntegerValue::Signed(2),
    }];
    let mut handler = AcceptTerminalEffects;
    let execution = interpret_terminal_artifact_with_effect_handler_measured(
        &module_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &scalar_arguments,
        &structural_arguments,
        &mut handler,
    )
    .expect("verified affine-cast artifact interprets");
    assert_eq!(
        execution.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true)),
    );
    assert!(execution.effects().is_empty());
}

#[test]
fn partial_cast_to_affine_exact_division_and_remainder_cross_source_codec_verification_and_interpretation()
 {
    let tokens = Lexer::new(CAST_AFFINE_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::divide_after_cast")
        .expect("partial-cast-to-affine divisor lowers from real source");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let exact_obligations = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerDivide { obligation, .. }
            | OperationKind::ExactIntegerRemainder { obligation, .. } => Some(obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(exact_obligations.len(), 2, "one divide and one remainder");
    for obligation in &exact_obligations {
        let evidence = lowered
            .proof_bundle
            .evidence
            .iter()
            .find(|evidence| evidence.obligation == *obligation)
            .expect("exact divide/remainder has evidence");
        let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
            panic!("post-cast affine exact operation is certificate-derived")
        };
        let ProofRule::DisjunctionIntroduction { disjunct, index: 1 } = &certificate.proof.rule
        else {
            panic!("signed positive-divisor arm is selected")
        };
        let ProofRule::IntegerAffineBound {
            root_bound: cast_bound,
            witness: affine_witness,
        } = &disjunct.rule
        else {
            panic!("affine custody is the outer proof")
        };
        let ProofRule::IntegerCastBound {
            witness: cast_witness,
            ..
        } = &cast_bound.rule
        else {
            panic!("direct cast custody is independently retained below affine")
        };
        assert_eq!(cast_witness.definition_axioms.len(), 1);
        assert_eq!(affine_witness.definition_axioms.len(), 1);
        assert_eq!(affine_witness.literal_axioms.len(), 1);
        assert!(affine_witness.literal_axioms[0].is_some());
        assert!(cast_witness.definition_axioms[0] < affine_witness.definition_axioms[0]);
    }

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("independent verifier reconstructs cast then affine custody");
    let module_bytes = encode_module(&lowered.semantic_module).expect("encode module");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("encode proof bundle");
    assert_eq!(
        decode_module(&module_bytes).expect("decode module"),
        lowered.semantic_module,
    );
    assert_eq!(
        decode_proof_bundle(&proof_bytes).expect("decode proof bundle"),
        lowered.proof_bundle,
    );

    let mut stale = decode_proof_bundle(&proof_bytes).expect("decode proof for mutation");
    let stale_evidence = stale
        .evidence
        .iter_mut()
        .find(|evidence| evidence.obligation == exact_obligations[0])
        .expect("decoded exact evidence");
    let EvidenceRoute::CertificateDerived(certificate) = &mut stale_evidence.route else {
        unreachable!("selected certificate-derived evidence")
    };
    let ProofRule::DisjunctionIntroduction { disjunct, .. } = &mut certificate.proof.rule else {
        unreachable!("selected signed disjunction proof")
    };
    let ProofRule::IntegerAffineBound {
        root_bound,
        witness: affine_witness,
    } = &mut disjunct.rule
    else {
        unreachable!("selected affine proof")
    };
    let ProofRule::IntegerCastBound {
        witness: cast_witness,
        ..
    } = &root_bound.rule
    else {
        unreachable!("selected cast child")
    };
    affine_witness.definition_axioms[0] = cast_witness.definition_axioms[0];
    assert!(
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &stale,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "redirecting the affine witness to the earlier cast definition rejects",
    );

    let [token] = entry.structural_parameters.as_slice() else {
        panic!("entry retains the Token cleanup root")
    };
    let structural_arguments = [TerminalStructuralValue {
        opaque_identity: token.place.get(),
        structural_type: token.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    }];
    let scalar_arguments = [TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16"),
        value: IntegerValue::Signed(2),
    }];
    let mut handler = AcceptTerminalEffects;
    let execution = interpret_terminal_artifact_with_effect_handler_measured(
        &module_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &scalar_arguments,
        &structural_arguments,
        &mut handler,
    )
    .expect("verified post-cast affine artifact interprets");
    assert_eq!(
        execution.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true)),
    );
    assert!(execution.effects().is_empty());
}

#[test]
fn affine_cast_affine_exact_division_and_remainder_cross_source_codec_verification_and_interpretation()
 {
    let tokens = Lexer::new(AFFINE_CAST_AFFINE_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered =
        psi_checked_trees_to_terminal::lower_machine(&checked, "Root::divide_across_cast")
            .expect("affine-cast-affine divisor lowers from real source");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let exact_obligations = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerDivide { obligation, .. }
            | OperationKind::ExactIntegerRemainder { obligation, .. } => Some(obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(exact_obligations.len(), 2, "one divide and one remainder");
    for obligation in &exact_obligations {
        let evidence = lowered
            .proof_bundle
            .evidence
            .iter()
            .find(|evidence| evidence.obligation == *obligation)
            .expect("exact divide/remainder has evidence");
        let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
            panic!("affine-cast-affine exact operation is certificate-derived")
        };
        let ProofRule::DisjunctionIntroduction { disjunct, index: 1 } = &certificate.proof.rule
        else {
            panic!("signed positive-divisor arm is selected")
        };
        let ProofRule::IntegerAffineBound {
            root_bound: cast_bound,
            witness: after_witness,
        } = &disjunct.rule
        else {
            panic!("post-cast affine custody is outermost")
        };
        let ProofRule::IntegerCastBound {
            root_bound: before_bound,
            witness: cast_witness,
        } = &cast_bound.rule
        else {
            panic!("partial-cast custody is nested between affine proofs")
        };
        let ProofRule::IntegerAffineBound {
            witness: before_witness,
            ..
        } = &before_bound.rule
        else {
            panic!("pre-cast affine custody is innermost")
        };
        assert_eq!(before_witness.definition_axioms.len(), 1);
        assert_eq!(cast_witness.definition_axioms.len(), 1);
        assert_eq!(after_witness.definition_axioms.len(), 1);
        assert!(before_witness.definition_axioms[0] < cast_witness.definition_axioms[0]);
        assert!(cast_witness.definition_axioms[0] < after_witness.definition_axioms[0]);
    }

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("independent verifier reconstructs affine-cast-affine custody");
    let module_bytes = encode_module(&lowered.semantic_module).expect("encode module");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("encode proof bundle");
    assert_eq!(
        decode_module(&module_bytes).expect("decode module"),
        lowered.semantic_module,
    );
    assert_eq!(
        decode_proof_bundle(&proof_bytes).expect("decode proof bundle"),
        lowered.proof_bundle,
    );

    let mut stale = decode_proof_bundle(&proof_bytes).expect("decode proof for mutation");
    let stale_evidence = stale
        .evidence
        .iter_mut()
        .find(|evidence| evidence.obligation == exact_obligations[0])
        .expect("decoded exact evidence");
    let EvidenceRoute::CertificateDerived(certificate) = &mut stale_evidence.route else {
        unreachable!("selected certificate-derived evidence")
    };
    let ProofRule::DisjunctionIntroduction { disjunct, .. } = &mut certificate.proof.rule else {
        unreachable!("selected signed disjunction proof")
    };
    let ProofRule::IntegerAffineBound {
        root_bound: cast_bound,
        witness: after_witness,
    } = &mut disjunct.rule
    else {
        unreachable!("selected post-cast affine proof")
    };
    let ProofRule::IntegerCastBound {
        root_bound: before_bound,
        witness: cast_witness,
    } = &mut cast_bound.rule
    else {
        unreachable!("selected cast child")
    };
    let ProofRule::IntegerAffineBound {
        witness: before_witness,
        ..
    } = &mut before_bound.rule
    else {
        unreachable!("selected pre-cast affine child")
    };
    after_witness.definition_axioms[0] = before_witness.definition_axioms[0];
    assert_ne!(
        after_witness.definition_axioms[0],
        cast_witness.definition_axioms[0],
    );
    assert!(
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &stale,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "redirecting the post-cast witness before the cast rejects",
    );

    let [token] = entry.structural_parameters.as_slice() else {
        panic!("entry retains the Token cleanup root")
    };
    let structural_arguments = [TerminalStructuralValue {
        opaque_identity: token.place.get(),
        structural_type: token.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    }];
    let scalar_arguments = [TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16"),
        value: IntegerValue::Signed(1),
    }];
    let mut handler = AcceptTerminalEffects;
    let execution = interpret_terminal_artifact_with_effect_handler_measured(
        &module_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &scalar_arguments,
        &structural_arguments,
        &mut handler,
    )
    .expect("verified affine-cast-affine artifact interprets");
    assert_eq!(
        execution.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true)),
    );
    assert!(execution.effects().is_empty());

    let missing_root_bound =
        AFFINE_CAST_AFFINE_SOURCE.replace("root <= 125i16, -1i16 <= root", "root <= 125i16");
    let tokens = Lexer::new(&missing_root_bound)
        .tokenize()
        .expect("tokenize near-miss source");
    let syntax = parse_syntax_trees(&tokens).expect("parse near-miss source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve near-miss source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type near-miss source");
    let checked = lower_typed_trees(typed).expect("check near-miss source");
    assert!(
        psi_checked_trees_to_terminal::lower_machine(&checked, "Root::divide_across_cast").is_err(),
        "the sandwich cannot manufacture its missing source-root lower bound",
    );
}
