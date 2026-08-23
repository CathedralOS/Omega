use super::*;

const SOURCE: &str = r#"
    data Token {}
    machine Token::drop(&mut self) {}

    data Root {}
    machine Root::enter(token: Token) {}
"#;

const SCALAR_SOURCE: &str = r#"
    data Token { flag: bool; tag: u8; delta: i16; payload: u64; address: addr; }
    machine Token::drop(&mut self) {}

    data Root {}
    machine Root::enter(token: Token) {}
"#;

const CONTEXTUAL_SOURCE: &str = r#"
    data Token { ready: bool; }
    machine Token::drop(&mut self)
    requires self.ready
    {}

    data Root {}
    machine Root::enter(token: Token)
    requires token.ready
    {}
"#;

const FINITE_CONTEXTUAL_SOURCE: &str = r#"
    data Token { ready: bool; audited: bool; armed: bool; }
    machine Token::drop(&mut self)
    requires
        self.armed;
        self.ready
    {}

    data Root {}
    machine Root::enter(token: Token)
    requires
        token.armed;
        token.audited;
        token.ready
    {}
"#;

const CALLER_ONLY_CONTEXTUAL_SOURCE: &str = r#"
    data Token { observed: bool; }
    machine Token::drop(&mut self) {}

    data Root {}
    machine Root::enter(token: Token)
    requires token.observed
    {}
"#;

const TWO_ROOT_SOURCE: &str = r#"
    data Token {}
    machine Token::drop(&mut self) {}

    data Root {}
    machine Root::enter(first: Token, second: Token) {}
"#;

const TWO_ROOT_CONTEXTUAL_SOURCE: &str = r#"
    data Token { ready: bool; }
    machine Token::drop(&mut self)
    requires self.ready
    {}

    data Root {}
    machine Root::enter(first: Token, second: Token)
    requires first.ready, second.ready
    {}
"#;

const TWO_ROOT_ONE_EXECUTABLE_SOURCE: &str = r#"
    data FirstHelper {}
    machine FirstHelper::touch() {}
    data SecondHelper {}
    machine SecondHelper::touch() {}
    data ThirdHelper {}
    machine ThirdHelper::touch() {}

    data First {}
    machine First::drop(&mut self) {
        FirstHelper::touch();
        SecondHelper::touch();
        ThirdHelper::touch();
    }
    data Second {}
    machine Second::drop(&mut self) {}

    data Root {}
    machine Root::enter(first: First, second: Second) {}
"#;

const TWO_ROOT_TWO_EXECUTABLE_SOURCE: &str = r#"
    data FirstHelper {}
    machine FirstHelper::touch() {}
    data SecondHelper {}
    machine SecondHelper::touch() {}

    data First {}
    machine First::drop(&mut self) { FirstHelper::touch(); }
    data Second {}
    machine Second::drop(&mut self) { SecondHelper::touch(); }

    data Root {}
    machine Root::enter(first: First, second: Second) {}
"#;

const TWO_ROOT_SHARED_EXECUTABLE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token {}
    machine Token::drop(&mut self) { Helper::touch(); }

    data Root {}
    machine Root::enter(first: Token, second: Token) {}
"#;

const THREE_ROOT_DISTINCT_SOURCE: &str = r#"
    data First {}
    machine First::drop(&mut self) {}
    data Second {}
    machine Second::drop(&mut self) {}
    data Third {}
    machine Third::drop(&mut self) {}

    data Root {}
    machine Root::enter(first: First, second: Second, third: Third) {}
"#;

const THREE_ROOT_SHARED_EXECUTABLE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token {}
    machine Token::drop(&mut self) { Helper::touch(); }

    data Root {}
    machine Root::enter(first: Token, second: Token, third: Token) {}
"#;

const EXECUTABLE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token { flag: bool; }
    machine Token::drop(&mut self) {
        Helper::touch();
    }

    data Root {}
    machine Root::enter(token: Token) {}
"#;

const CONTEXTUAL_EXECUTABLE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token { ready: bool; padding: u8; }
    machine Token::drop(&mut self)
    requires self.ready
    {
        Helper::touch();
    }

    data Root {}
    machine Root::enter(first: Token, second: Token)
    requires second.ready, first.ready
    {}
"#;

const TWO_CALL_SOURCE: &str = r#"
    data First {}
    machine First::touch() {}
    data Second {}
    machine Second::touch() {}

    data Token { flag: bool; }
    machine Token::drop(&mut self) {
        First::touch();
        Second::touch();
    }

    data Root {}
    machine Root::enter(token: Token) {}
"#;

const THREE_CALL_SOURCE: &str = r#"
    data First {}
    machine First::touch() {}
    data Second {}
    machine Second::touch() {}
    data Third {}
    machine Third::touch() {}

    data Token { flag: bool; }
    machine Token::drop(&mut self) {
        First::touch();
        Second::touch();
        Third::touch();
    }

    data Root {}
    machine Root::enter(token: Token) {}
"#;

#[test]
fn empty_nominal_cleanup_crosses_source_lowering_codec_and_verifier() {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("empty nominal cleanup lowers");

    assert_eq!(
        lowered.semantic_module.machines.len(),
        2,
        "the cleanup target is part of the executable terminal closure"
    );
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [root] = entry.structural_parameters.as_slice() else {
        panic!("nominal cleanup source slice has one structural root")
    };
    assert_eq!(root.multiplicity, StructuralMultiplicity::Affine);
    assert!(root.qualifications.is_empty());
    let [block] = entry.blocks.as_slice() else {
        panic!("nominal cleanup source slice has one block")
    };
    assert!(block.operations.is_empty());
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &block.terminator else {
        panic!("expected executable nominal cleanup return")
    };
    assert_eq!(cleanups[0].place, root.place);
    assert_eq!(cleanups[0].structural_type, root.structural_type);

    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanups[0].cleanup_machine)
        .expect("cleanup target machine");
    assert_eq!(target.attachment, Some(cleanups[0].structural_type));
    assert!(target.structural_parameters.is_empty());
    assert!(target.blocks[0].operations.is_empty());
    assert!(matches!(
        &target.blocks[0].terminator,
        Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } if trivial_affine_discards.is_empty()
    ));

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts exact nominal cleanup closure");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&bytes).expect("semantic module decodes"),
        lowered.semantic_module,
        "nominal cleanup target identity is canonical artifact data"
    );
}

#[test]
fn contextual_nominal_cleanup_crosses_source_lowering_codec_and_verifier() {
    let tokens = Lexer::new(CONTEXTUAL_SOURCE)
        .tokenize()
        .expect("tokenize contextual cleanup");
    let syntax = parse_syntax_trees(&tokens).expect("parse contextual cleanup");
    let resolved = lower_syntax_trees(&syntax).expect("resolve contextual cleanup");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type contextual cleanup source");
    let checked = lower_typed_trees(typed).expect("check contextual cleanup source");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("contextual nominal cleanup lowers");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [parameter] = entry.structural_parameters.as_slice() else {
        panic!("contextual cleanup caller has one structural parameter")
    };
    let [caller_requirement] = entry.contract.requires.as_slice() else {
        panic!("contextual cleanup caller retains one required premise")
    };
    let [block] = entry.blocks.as_slice() else {
        panic!("contextual cleanup caller has one block")
    };
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &block.terminator else {
        panic!("contextual cleanup uses the nominal return carrier")
    };
    let [cleanup] = cleanups.as_slice() else {
        panic!("contextual cleanup has one action")
    };
    let receiver = cleanup
        .cleanup_receiver
        .expect("contextual cleanup carries a proof-only receiver root");
    let [obligation] = cleanup.requirement_obligations.as_slice() else {
        panic!("contextual cleanup carries one requirement obligation")
    };
    assert_ne!(receiver, parameter.place);
    assert_eq!(lowered.proof_bundle.evidence.len(), 1);
    assert_eq!(lowered.proof_bundle.evidence[0].obligation, *obligation);

    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanup.cleanup_machine)
        .expect("cleanup target");
    assert!(target.structural_parameters.is_empty());
    assert!(target.structural_places.is_empty());
    let [target_requirement] = target.contract.requires.as_slice() else {
        panic!("cleanup target retains one contextual requirement")
    };
    let psi_core::Proposition::Equal(target_left, target_right) = target_requirement else {
        panic!("target contextual requirement is an equality")
    };
    assert_eq!(target_left, &psi_core::ScalarTerm::boolean(true));
    let psi_core::ScalarTerm::BooleanField {
        root: target_root,
        path: target_path,
    } = target_right
    else {
        panic!("target contextual requirement names its Boolean field")
    };
    let [psi_core::CanonicalStructuralPathSegment::Field(target_field)] = target_path.as_slice()
    else {
        panic!("target contextual requirement names one direct Boolean field")
    };
    assert_eq!(*target_root, receiver);
    let psi_core::Proposition::Equal(caller_left, caller_right) = caller_requirement else {
        panic!("caller contextual requirement is an equality")
    };
    assert_eq!(caller_left, &psi_core::ScalarTerm::boolean(true));
    assert_eq!(
        caller_right,
        &psi_core::ScalarTerm::boolean_field(parameter.place, *target_field),
        "the caller assumption is the cleanup target premise rebased to the owned root",
    );

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier discharges contextual cleanup from the caller requirement");
    let bytes = encode_module(&lowered.semantic_module).expect("contextual module encodes");
    assert_eq!(
        decode_module(&bytes).expect("contextual module decodes"),
        lowered.semantic_module,
        "contextual cleanup premise and obligation are canonical terminal data",
    );
    let proof_bytes =
        encode_proof_bundle(&lowered.proof_bundle).expect("contextual proof bundle encodes");
    assert_eq!(
        decode_proof_bundle(&proof_bytes).expect("contextual proof bundle decodes"),
        lowered.proof_bundle,
        "contextual cleanup evidence is canonical proof-artifact data",
    );
}

#[test]
fn finite_contextual_nominal_cleanup_preserves_caller_superset_and_canonical_artifacts() {
    let tokens = Lexer::new(FINITE_CONTEXTUAL_SOURCE)
        .tokenize()
        .expect("tokenize finite contextual cleanup");
    let syntax = parse_syntax_trees(&tokens).expect("parse finite contextual cleanup");
    let resolved = lower_syntax_trees(&syntax).expect("resolve finite contextual cleanup");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type finite contextual cleanup source");
    let checked = lower_typed_trees(typed).expect("check finite contextual cleanup source");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("finite contextual nominal cleanup lowers");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [parameter] = entry.structural_parameters.as_slice() else {
        panic!("finite contextual cleanup caller has one structural parameter")
    };
    let [block] = entry.blocks.as_slice() else {
        panic!("finite contextual cleanup caller has one block")
    };
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &block.terminator else {
        panic!("finite contextual cleanup uses the nominal return carrier")
    };
    let [cleanup] = cleanups.as_slice() else {
        panic!("finite contextual cleanup has one action")
    };
    let receiver = cleanup
        .cleanup_receiver
        .expect("finite contextual cleanup carries a proof-only receiver root");
    assert_ne!(receiver, parameter.place);
    assert_eq!(
        cleanup
            .requirement_obligations
            .iter()
            .map(|obligation| obligation.get())
            .collect::<Vec<_>>(),
        vec![1, 2],
        "cleanup obligations are stable and dense in target-clause order",
    );

    let token_type = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == cleanup.structural_type)
        .expect("Token terminal type");
    let StructuralTypeShape::Record { fields } = &token_type.shape else {
        panic!("Token is a record")
    };
    let field = |identity: &str| {
        fields
            .iter()
            .find(|field| field.identity == identity)
            .unwrap_or_else(|| panic!("{identity} terminal field"))
            .id
    };
    let ready = field("ready");
    let armed = field("armed");
    let audited = field("audited");
    let caller_requires = vec![
        psi_core::Proposition::Equal(
            psi_core::ScalarTerm::boolean(true),
            psi_core::ScalarTerm::boolean_field(parameter.place, ready),
        ),
        psi_core::Proposition::Equal(
            psi_core::ScalarTerm::boolean(true),
            psi_core::ScalarTerm::boolean_field(parameter.place, audited),
        ),
        psi_core::Proposition::Equal(
            psi_core::ScalarTerm::boolean(true),
            psi_core::ScalarTerm::boolean_field(parameter.place, armed),
        ),
    ];
    assert_eq!(
        entry.contract.requires, caller_requires,
        "the full caller set is canonically ordered by terminal field identity",
    );

    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanup.cleanup_machine)
        .expect("cleanup target");
    assert!(target.structural_parameters.is_empty());
    assert!(target.structural_places.is_empty());
    assert_eq!(
        target.contract.requires,
        vec![
            psi_core::Proposition::Equal(
                psi_core::ScalarTerm::boolean(true),
                psi_core::ScalarTerm::boolean_field(receiver, ready),
            ),
            psi_core::Proposition::Equal(
                psi_core::ScalarTerm::boolean(true),
                psi_core::ScalarTerm::boolean_field(receiver, armed),
            ),
        ],
        "the cleanup target retains only its canonical requirement subset",
    );
    assert_eq!(lowered.proof_bundle.evidence.len(), 2);
    for (obligation_index, evidence) in lowered.proof_bundle.evidence.iter().enumerate() {
        assert_eq!(
            evidence.obligation,
            cleanup.requirement_obligations[obligation_index]
        );
        let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
            panic!("contextual cleanup evidence is certificate-derived")
        };
        let assumption_index = [0, 2][obligation_index];
        assert_eq!(
            certificate.proof.conclusion,
            caller_requires[assumption_index]
        );
        assert!(matches!(
            certificate.proof.rule,
            ProofRule::Assumption { index: assumption } if assumption == assumption_index
        ));
    }

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier discharges the finite cleanup subset from the caller superset");
    let semantic_bytes =
        encode_module(&lowered.semantic_module).expect("finite contextual module encodes");
    assert_eq!(
        decode_module(&semantic_bytes).expect("finite contextual module decodes"),
        lowered.semantic_module,
        "finite contextual cleanup semantic data is canonical",
    );
    let proof_bytes =
        encode_proof_bundle(&lowered.proof_bundle).expect("finite contextual proof bundle encodes");
    assert_eq!(
        decode_proof_bundle(&proof_bytes).expect("finite contextual proof bundle decodes"),
        lowered.proof_bundle,
        "finite contextual cleanup proof data is canonical",
    );
}

#[test]
fn caller_only_contextual_fact_does_not_invent_a_cleanup_receiver_or_obligation() {
    let tokens = Lexer::new(CALLER_ONLY_CONTEXTUAL_SOURCE)
        .tokenize()
        .expect("tokenize caller-only contextual cleanup");
    let syntax = parse_syntax_trees(&tokens).expect("parse caller-only contextual cleanup");
    let resolved = lower_syntax_trees(&syntax).expect("resolve caller-only contextual cleanup");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type caller-only contextual cleanup source");
    let checked = lower_typed_trees(typed).expect("check caller-only contextual cleanup source");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("caller-only contextual nominal cleanup lowers");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [parameter] = entry.structural_parameters.as_slice() else {
        panic!("caller-only contextual cleanup has one structural parameter")
    };
    let [caller_requirement] = entry.contract.requires.as_slice() else {
        panic!("caller-only contextual fact is retained")
    };
    let [block] = entry.blocks.as_slice() else {
        panic!("caller-only contextual cleanup has one block")
    };
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &block.terminator else {
        panic!("caller-only contextual cleanup uses the nominal return carrier")
    };
    let [cleanup] = cleanups.as_slice() else {
        panic!("caller-only contextual cleanup has one action")
    };
    assert!(cleanup.cleanup_receiver.is_none());
    assert!(cleanup.requirement_obligations.is_empty());
    assert!(lowered.proof_bundle.evidence.is_empty());

    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanup.cleanup_machine)
        .expect("cleanup target");
    assert!(target.contract.requires.is_empty());
    let psi_core::Proposition::Equal(
        psi_core::ScalarTerm::Boolean(true),
        psi_core::ScalarTerm::BooleanField { root, .. },
    ) = caller_requirement
    else {
        panic!("caller-only contextual fact retains its Boolean-field shape")
    };
    assert_eq!(*root, parameter.place);

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts a caller-only fact without a cleanup obligation");
    let semantic_bytes =
        encode_module(&lowered.semantic_module).expect("caller-only contextual module encodes");
    assert_eq!(
        decode_module(&semantic_bytes).expect("caller-only contextual module decodes"),
        lowered.semantic_module,
    );
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("caller-only contextual proof bundle encodes");
    assert_eq!(
        decode_proof_bundle(&proof_bytes).expect("caller-only contextual proof bundle decodes"),
        lowered.proof_bundle,
    );
}

#[test]
fn wide_mixed_primitive_record_crosses_source_lowering_codec_and_verifier() {
    let tokens = Lexer::new(SCALAR_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("wide flat scalar nominal cleanup lowers");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("expected executable nominal cleanup return")
    };
    let cleanup_type = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == cleanups[0].structural_type)
        .expect("cleanup structural type");
    let StructuralTypeShape::Record { fields } = &cleanup_type.shape else {
        panic!("cleanup type remains a record")
    };
    let [flag, tag, delta, payload, address] = fields.as_slice() else {
        panic!("bounded cleanup record retains all five fields")
    };
    for (field, identity, scalar_type) in [
        (flag, "flag", ScalarType::Boolean),
        (
            tag,
            "tag",
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")),
        ),
        (
            delta,
            "delta",
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 16).expect("i16")),
        ),
        (
            payload,
            "payload",
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64")),
        ),
        (
            address,
            "address",
            ScalarType::Integer(IntegerType::address(64).expect("addr")),
        ),
    ] {
        assert_eq!(field.identity, identity);
        assert!(!field.relevance.is_erased());
        let StructuralFieldType::Scalar(actual) = &field.field_type else {
            panic!("wide cleanup record retains scalar carriers")
        };
        assert_eq!(*actual, scalar_type);
    }

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts wide flat scalar nominal cleanup closure");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&bytes).expect("semantic module decodes"),
        lowered.semantic_module,
        "the primitive field and nominal cleanup identity are canonical artifact data"
    );
}

#[test]
fn two_nominal_roots_cleanup_in_reverse_parameter_order_and_may_share_a_target() {
    let tokens = Lexer::new(TWO_ROOT_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("two nominal roots lower");

    assert_eq!(
        lowered.semantic_module.machines.len(),
        2,
        "same-type roots share one exact cleanup target"
    );
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [first, second] = entry.structural_parameters.as_slice() else {
        panic!("two source roots remain structural parameters")
    };
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("expected ordered nominal cleanup return")
    };
    let [second_cleanup, first_cleanup] = cleanups.as_slice() else {
        panic!("both roots require nominal cleanup")
    };
    assert_eq!(second_cleanup.place, second.place);
    assert_eq!(first_cleanup.place, first.place);
    assert_eq!(second_cleanup.structural_type, second.structural_type);
    assert_eq!(first_cleanup.structural_type, first.structural_type);
    assert_eq!(
        second_cleanup.cleanup_machine, first_cleanup.cleanup_machine,
        "same-type roots reuse the same exact cleanup target"
    );

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts ordered two-root nominal cleanup");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&bytes).expect("semantic module decodes"),
        lowered.semantic_module
    );
}

#[test]
fn contextual_multi_root_nominal_cleanup_crosses_source_codec_and_verifier() {
    let tokens = Lexer::new(TWO_ROOT_CONTEXTUAL_SOURCE)
        .tokenize()
        .expect("tokenize contextual two-root cleanup");
    let syntax = parse_syntax_trees(&tokens).expect("parse contextual two-root cleanup");
    let resolved = lower_syntax_trees(&syntax).expect("resolve contextual two-root cleanup");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type contextual two-root cleanup source");
    let checked = lower_typed_trees(typed).expect("check contextual two-root cleanup source");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("contextual two-root cleanup lowers");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("terminal entry");
    let [first, second] = entry.structural_parameters.as_slice() else {
        panic!("contextual caller retains two owned roots")
    };
    assert_eq!(entry.contract.requires.len(), 2);
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("contextual multi-root cleanup uses nominal return")
    };
    let [second_cleanup, first_cleanup] = cleanups.as_slice() else {
        panic!("contextual multi-root cleanup retains both actions")
    };
    assert_eq!(second_cleanup.place, second.place);
    assert_eq!(first_cleanup.place, first.place);
    assert_eq!(
        second_cleanup.cleanup_machine,
        first_cleanup.cleanup_machine
    );
    assert_eq!(
        second_cleanup.cleanup_receiver,
        first_cleanup.cleanup_receiver
    );
    assert!(second_cleanup.cleanup_receiver.is_some());
    assert_eq!(second_cleanup.requirement_obligations.len(), 1);
    assert_eq!(first_cleanup.requirement_obligations.len(), 1);
    assert_ne!(
        second_cleanup.requirement_obligations,
        first_cleanup.requirement_obligations
    );
    assert_eq!(lowered.proof_bundle.evidence.len(), 2);
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently discharges both root-specific cleanup goals");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&bytes).expect("semantic module decodes"),
        lowered.semantic_module
    );
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("proof bundle encodes");
    assert_eq!(
        decode_proof_bundle(&proof_bytes).expect("proof bundle decodes"),
        lowered.proof_bundle
    );
}

#[test]
fn two_nominal_roots_allow_one_executable_cleanup_in_reverse_order() {
    let tokens = Lexer::new(TWO_ROOT_ONE_EXECUTABLE_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("one executable cleanup in a two-root list lowers");

    assert_eq!(lowered.semantic_module.machines.len(), 6);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [first, second] = entry.structural_parameters.as_slice() else {
        panic!("two source roots remain structural parameters")
    };
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("expected ordered nominal cleanup return")
    };
    let [second_cleanup, first_cleanup] = cleanups.as_slice() else {
        panic!("both roots require nominal cleanup")
    };
    assert_eq!(second_cleanup.place, second.place);
    assert_eq!(first_cleanup.place, first.place);
    let second_target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == second_cleanup.cleanup_machine)
        .expect("second cleanup target");
    let first_target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == first_cleanup.cleanup_machine)
        .expect("first cleanup target");
    assert!(second_target.blocks[0].operations.is_empty());
    let [first_helper_call, second_helper_call, third_helper_call] =
        first_target.blocks[0].operations.as_slice()
    else {
        panic!("exactly one cleanup body retains all three ordered helper calls")
    };
    let helper_callees =
        [first_helper_call, second_helper_call, third_helper_call].map(|operation| {
            let OperationKind::CallUnit {
                callee,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } = &operation.kind
            else {
                panic!("cleanup helper operation remains an ordinary Unit call")
            };
            assert!(structural_arguments.is_empty());
            assert!(claim_transfers.is_empty());
            assert!(requirement_obligations.is_empty());
            assert!(crash_continuations.is_empty());
            *callee
        });
    assert_ne!(helper_callees[0], helper_callees[1]);
    assert_ne!(helper_callees[0], helper_callees[2]);
    assert_ne!(helper_callees[1], helper_callees[2]);

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts one executable cleanup in an ordered list");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&bytes).expect("semantic module decodes"),
        lowered.semantic_module
    );
}

#[test]
fn two_nominal_roots_run_distinct_executable_cleanups_in_reverse_order() {
    let tokens = Lexer::new(TWO_ROOT_TWO_EXECUTABLE_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("two distinct executable cleanup actions lower");

    assert_eq!(lowered.semantic_module.machines.len(), 5);
    let entry = &lowered.semantic_module.machines[0];
    let [first, second] = entry.structural_parameters.as_slice() else {
        panic!("two source roots remain structural parameters")
    };
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("ordered nominal cleanup return")
    };
    assert_eq!(
        [cleanups[0].place, cleanups[1].place],
        [second.place, first.place]
    );
    let helper_ids = cleanups
        .iter()
        .map(|cleanup| {
            let target = lowered
                .semantic_module
                .machines
                .iter()
                .find(|machine| machine.id == cleanup.cleanup_machine)
                .expect("cleanup target");
            let [operation] = target.blocks[0].operations.as_slice() else {
                panic!("each cleanup body has one helper call")
            };
            let OperationKind::CallUnit { callee, .. } = operation.kind else {
                panic!("cleanup helper call")
            };
            callee
        })
        .collect::<Vec<_>>();
    assert_ne!(helper_ids[0], helper_ids[1]);

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("two executable cleanup actions verify");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
}

#[test]
fn two_nominal_roots_may_repeat_one_executable_cleanup_target_and_helper() {
    let tokens = Lexer::new(TWO_ROOT_SHARED_EXECUTABLE_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("shared executable cleanup target lowers");

    assert_eq!(
        lowered.semantic_module.machines.len(),
        3,
        "caller, shared cleanup target, and shared helper form the exact closure"
    );
    let entry = &lowered.semantic_module.machines[0];
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("ordered nominal cleanup return")
    };
    assert_eq!(cleanups[0].cleanup_machine, cleanups[1].cleanup_machine);
    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanups[0].cleanup_machine)
        .expect("shared cleanup target");
    assert_eq!(target.blocks[0].operations.len(), 1);

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("shared executable cleanup target verifies");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
}

#[test]
fn contextual_roots_may_share_one_executable_cleanup_target_and_helper() {
    let tokens = Lexer::new(CONTEXTUAL_EXECUTABLE_SOURCE)
        .tokenize()
        .expect("tokenize contextual executable cleanup");
    let syntax = parse_syntax_trees(&tokens).expect("parse contextual executable cleanup");
    let resolved = lower_syntax_trees(&syntax).expect("resolve contextual executable cleanup");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type contextual executable cleanup");
    let checked = lower_typed_trees(typed).expect("check contextual executable cleanup");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("contextual executable cleanup lowers");

    let entry = &lowered.semantic_module.machines[0];
    let [first, second] = entry.structural_parameters.as_slice() else {
        panic!("contextual executable caller retains two roots")
    };
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("contextual executable cleanup uses nominal return")
    };
    let [second_cleanup, first_cleanup] = cleanups.as_slice() else {
        panic!("contextual executable cleanup retains both actions")
    };
    assert_eq!(
        [second_cleanup.place, first_cleanup.place],
        [second.place, first.place]
    );
    assert_eq!(
        second_cleanup.cleanup_machine,
        first_cleanup.cleanup_machine
    );
    assert_eq!(
        second_cleanup.cleanup_receiver,
        first_cleanup.cleanup_receiver
    );
    assert!(second_cleanup.cleanup_receiver.is_some());
    assert_ne!(
        second_cleanup.requirement_obligations,
        first_cleanup.requirement_obligations
    );
    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == second_cleanup.cleanup_machine)
        .expect("shared contextual cleanup target");
    assert_eq!(target.contract.requires.len(), 1);
    assert_eq!(target.blocks[0].operations.len(), 1);
    assert_eq!(lowered.proof_bundle.evidence.len(), 2);

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("contextual executable cleanup verifies");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("proof bundle encodes");
    assert_eq!(
        decode_proof_bundle(&proof_bytes).unwrap(),
        lowered.proof_bundle
    );
    let structural_arguments = [
        TerminalStructuralValue {
            opaque_identity: 1,
            structural_type: first.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        },
        TerminalStructuralValue {
            opaque_identity: 2,
            structural_type: second.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        },
    ];
    let mut handler = AcceptTerminalEffects;
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[],
        &structural_arguments,
        &mut handler,
    )
    .expect("contextual executable cleanup interprets from canonical artifact sections");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), 7);
    assert!(measured.effects().is_empty());
}

#[test]
fn three_distinct_nominal_roots_cross_source_codec_and_verifier_in_reverse_order() {
    let tokens = Lexer::new(THREE_ROOT_DISTINCT_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("three distinct cleanup targets lower");

    assert_eq!(lowered.semantic_module.machines.len(), 4);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [first, second, third] = entry.structural_parameters.as_slice() else {
        panic!("three source roots remain structural parameters")
    };
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("ordered nominal cleanup return")
    };
    let [third_cleanup, second_cleanup, first_cleanup] = cleanups.as_slice() else {
        panic!("all three roots require nominal cleanup")
    };
    assert_eq!(
        [
            third_cleanup.place,
            second_cleanup.place,
            first_cleanup.place
        ],
        [third.place, second.place, first.place]
    );
    assert_ne!(
        third_cleanup.cleanup_machine,
        second_cleanup.cleanup_machine
    );
    assert_ne!(third_cleanup.cleanup_machine, first_cleanup.cleanup_machine);
    assert_ne!(
        second_cleanup.cleanup_machine,
        first_cleanup.cleanup_machine
    );

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("three distinct ordered cleanup actions verify");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
}

#[test]
fn three_nominal_roots_may_share_one_executable_target_and_helper() {
    let tokens = Lexer::new(THREE_ROOT_SHARED_EXECUTABLE_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("three shared executable cleanup actions lower");

    assert_eq!(lowered.semantic_module.machines.len(), 3);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [first, second, third] = entry.structural_parameters.as_slice() else {
        panic!("three source roots remain structural parameters")
    };
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("ordered nominal cleanup return")
    };
    assert_eq!(
        cleanups
            .iter()
            .map(|cleanup| cleanup.place)
            .collect::<Vec<_>>(),
        vec![third.place, second.place, first.place]
    );
    assert!(
        cleanups
            .iter()
            .all(|cleanup| cleanup.cleanup_machine == cleanups[0].cleanup_machine)
    );
    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanups[0].cleanup_machine)
        .expect("shared cleanup target");
    assert_eq!(target.blocks[0].operations.len(), 1);

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("three shared executable cleanup actions verify");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
}

#[test]
fn one_call_nominal_cleanup_crosses_source_lowering_codec_and_verifier() {
    let tokens = Lexer::new(EXECUTABLE_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("one-call nominal cleanup lowers");

    assert_eq!(lowered.semantic_module.machines.len(), 3);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("expected executable nominal cleanup return")
    };
    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanups[0].cleanup_machine)
        .expect("cleanup target");
    let [call] = target.blocks[0].operations.as_slice() else {
        panic!("cleanup target must contain exactly one call")
    };
    assert_eq!(call.result, OperationResult::Unit);
    let OperationKind::CallUnit {
        callee,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = &call.kind
    else {
        panic!("cleanup operation must be an ordinary Unit call")
    };
    assert!(structural_arguments.is_empty());
    assert!(claim_transfers.is_empty());
    assert!(requirement_obligations.is_empty());
    assert!(crash_continuations.is_empty());
    let helper = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == *callee)
        .expect("cleanup helper");
    assert_ne!(helper.id, target.id);
    assert_ne!(helper.id, entry.id);
    assert!(helper.structural_parameters.is_empty());
    assert!(helper.blocks[0].operations.is_empty());
    assert!(matches!(
        &helper.blocks[0].terminator,
        Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } if trivial_affine_discards.is_empty()
    ));

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts exact executable nominal cleanup closure");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&bytes).expect("semantic module decodes"),
        lowered.semantic_module,
        "the three-machine cleanup closure is canonical artifact data"
    );
}

#[test]
fn two_call_nominal_cleanup_preserves_source_order_through_codec_and_verifier() {
    let tokens = Lexer::new(TWO_CALL_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("two-call nominal cleanup lowers");

    assert_eq!(lowered.semantic_module.machines.len(), 4);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("expected executable nominal cleanup return")
    };
    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanups[0].cleanup_machine)
        .expect("cleanup target");
    let [first_call, second_call] = target.blocks[0].operations.as_slice() else {
        panic!("cleanup target must contain exactly two calls")
    };
    let callees = [first_call, second_call].map(|operation| {
        assert_eq!(operation.result, OperationResult::Unit);
        let OperationKind::CallUnit {
            callee,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } = &operation.kind
        else {
            panic!("cleanup operation must be an ordinary Unit call")
        };
        assert!(structural_arguments.is_empty());
        assert!(claim_transfers.is_empty());
        assert!(requirement_obligations.is_empty());
        assert!(crash_continuations.is_empty());
        *callee
    });
    assert_ne!(callees[0], callees[1]);
    assert_eq!(
        lowered
            .semantic_module
            .machines
            .iter()
            .map(|machine| machine.id)
            .collect::<Vec<_>>(),
        vec![entry.id, target.id, callees[0], callees[1]],
        "the exact closure retains source call order"
    );
    for callee in callees {
        let helper = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == callee)
            .expect("cleanup helper");
        assert!(helper.structural_parameters.is_empty());
        assert!(helper.blocks[0].operations.is_empty());
    }

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts exact ordered two-call nominal cleanup closure");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&bytes).expect("semantic module decodes"),
        lowered.semantic_module,
        "the four-machine ordered cleanup closure is canonical artifact data"
    );
}

#[test]
fn three_call_nominal_cleanup_preserves_exact_source_order_through_codec_and_verifier() {
    let tokens = Lexer::new(THREE_CALL_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("three-call nominal cleanup lowers");

    assert_eq!(lowered.semantic_module.machines.len(), 5);
    let entry = &lowered.semantic_module.machines[0];
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("expected executable nominal cleanup return")
    };
    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanups[0].cleanup_machine)
        .expect("cleanup target");
    let [first, second, third] = target.blocks[0].operations.as_slice() else {
        panic!("cleanup target retains exactly three calls")
    };
    let callees = [first, second, third].map(|operation| {
        let OperationKind::CallUnit { callee, .. } = operation.kind else {
            panic!("cleanup helper is an ordinary Unit call")
        };
        callee
    });
    assert_eq!(
        lowered
            .semantic_module
            .machines
            .iter()
            .map(|machine| machine.id)
            .collect::<Vec<_>>(),
        vec![entry.id, target.id, callees[0], callees[1], callees[2]]
    );

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("three-call nominal cleanup verifies");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
}
