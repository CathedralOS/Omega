use super::{
    CALLER_ONLY_CONTEXTUAL_SOURCE, CONTEXTUAL_EXECUTABLE_SOURCE, CONTEXTUAL_SOURCE,
    FINITE_CONTEXTUAL_SOURCE, SCALAR_SOURCE, SOURCE, TWO_ROOT_CONTEXTUAL_SOURCE,
    TWO_ROOT_ONE_EXECUTABLE_SOURCE, TWO_ROOT_SHARED_EXECUTABLE_SOURCE, TWO_ROOT_SOURCE,
    TWO_ROOT_TWO_EXECUTABLE_SOURCE,
};
use crate::nominal_affine_source::{
    AcceptTerminalEffects, AdmissionProfile, EvidenceRoute, IntegerSign, IntegerType, Lexer,
    OperationKind, OperationResult, ProofRule, ResolutionRequest, ScalarType, StructuralFieldType,
    StructuralMultiplicity, StructuralTypeShape, TerminalAffineCleanupAction,
    TerminalExecutionResult, TerminalMachineResult, TerminalStructuralValue, Terminator,
    decode_module, decode_proof_bundle, encode_module, encode_proof_section,
    interpret_terminal_artifact_measured, lower_symbol_resolved_trees, lower_typed_trees,
    parse_syntax_trees, resolve,
};
use terminal_interpreter::TerminalStructuralInputs;

#[test]
fn empty_nominal_cleanup_crosses_source_lowering_codec_and_verifier() {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
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

    terminal_verifier::verify_module(
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
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve contextual cleanup");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type contextual cleanup source");
    let checked = lower_typed_trees(typed).expect("check contextual cleanup source");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
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
    let semantic_vocabulary::Proposition::Equal(target_left, target_right) = target_requirement
    else {
        panic!("target contextual requirement is an equality")
    };
    assert_eq!(target_left, &semantic_vocabulary::ScalarTerm::boolean(true));
    let semantic_vocabulary::ScalarTerm::BooleanField {
        root: target_root,
        path: target_path,
    } = target_right
    else {
        panic!("target contextual requirement names its Boolean field")
    };
    let [semantic_vocabulary::CanonicalStructuralPathSegment::Field(target_field)] =
        target_path.as_slice()
    else {
        panic!("target contextual requirement names one direct Boolean field")
    };
    assert_eq!(*target_root, receiver);
    let semantic_vocabulary::Proposition::Equal(caller_left, caller_right) = caller_requirement
    else {
        panic!("caller contextual requirement is an equality")
    };
    assert_eq!(caller_left, &semantic_vocabulary::ScalarTerm::boolean(true));
    assert_eq!(
        caller_right,
        &semantic_vocabulary::ScalarTerm::boolean_field(parameter.place, *target_field),
        "the caller observation is the cleanup target premise rebased to the owned root",
    );

    terminal_verifier::verify_module(
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
    let proof_bytes = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("contextual proof bundle encodes");
    assert_eq!(
        decode_proof_bundle(&proof_bytes).expect("contextual proof bundle decodes"),
        lowered.proof_bundle,
        "contextual cleanup evidence is canonical proof-artifact data",
    );

    // The generated certificate must not turn the entry observation into an
    // enduring assumption. A mutable call writes the field before cleanup.
    let mut changed = lowered.semantic_module.clone();
    let mut mutator = entry.clone();
    mutator.id = semantic_vocabulary::MachineId::new(9003).unwrap();
    mutator.entry = semantic_vocabulary::BlockId::new(9003).unwrap();
    mutator.contract.id = semantic_vocabulary::ContractId::new(9003).unwrap();
    mutator.contract.requires.clear();
    let borrowed_place = semantic_vocabulary::PlaceId::new(9003).unwrap();
    mutator.structural_parameters[0].place = borrowed_place;
    mutator.structural_parameters[0].access = terminal_psi::StructuralAccess::MutableBorrow;
    mutator.structural_places[0].id = borrowed_place;
    mutator.blocks[0].id = mutator.entry;
    mutator.blocks[0].terminator = Terminator::ReturnUnit {
        edge: semantic_vocabulary::EdgeId::new(9003).unwrap(),
        trivial_affine_discards: Vec::new(),
    };
    let value = semantic_vocabulary::ValueId::new(9001).unwrap();
    mutator.blocks[0].operations.extend([
        terminal_psi::Operation {
            static_reach_binding: None,
            id: semantic_vocabulary::OperationId::new(9001).unwrap(),
            result: OperationResult::Scalar(terminal_psi::ValueDeclaration {
                qualifications: Default::default(),
                id: value,
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanConstant { value: false },
        },
        terminal_psi::Operation {
            static_reach_binding: None,
            id: semantic_vocabulary::OperationId::new(9002).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::StructuralScalarFieldStore {
                range_obligation: None,
                destination: borrowed_place,
                path: Vec::new(),
                field: *target_field,
                value,
            },
        },
    ]);
    let changed_entry = changed
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed.entry)
        .unwrap();
    // The dedicated Unit nominal carrier has an empty-body contract. Ordinary
    // scalar cleanup admits calls before return and carries the same goal.
    let result = semantic_vocabulary::ValueId::new(9005).unwrap();
    changed_entry.result = TerminalMachineResult::Scalar(terminal_psi::ValueDeclaration {
        qualifications: Default::default(),
        id: semantic_vocabulary::ValueId::new(9006).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    let Terminator::ReturnUnitNominalAffine { edge, cleanups } =
        &changed_entry.blocks[0].terminator
    else {
        panic!("nominal cleanup")
    };
    changed_entry.blocks[0].terminator = Terminator::Return {
        edge: *edge,
        value: result,
        cleanup_actions: cleanups
            .iter()
            .cloned()
            .map(TerminalAffineCleanupAction::InvokeNominal)
            .collect(),
    };
    changed_entry.blocks[0]
        .operations
        .push(terminal_psi::Operation {
            static_reach_binding: None,
            id: semantic_vocabulary::OperationId::new(8999).unwrap(),
            result: OperationResult::Scalar(terminal_psi::ValueDeclaration {
                qualifications: Default::default(),
                id: result,
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanConstant { value: false },
        });
    changed_entry.blocks[0]
        .operations
        .push(terminal_psi::Operation {
            static_reach_binding: None,
            id: semantic_vocabulary::OperationId::new(9000).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                erased_arguments: Vec::new(),
                callee: mutator.id,
                arguments: Vec::new(),
                structural_arguments: vec![terminal_psi::StructuralArgument {
                    place: parameter.place,
                    path: Vec::new(),
                    access: terminal_psi::StructuralAccess::MutableBorrow,
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        });
    changed.machines.push(mutator);
    terminal_verifier::validate_module(&changed).expect("well-typed write before owned cleanup");
    let questions = terminal_verifier::reconstruct_terminal_obligations(&changed).unwrap();
    let question = questions
        .obligations()
        .iter()
        .find(|question| question.obligation.id == *obligation)
        .unwrap();
    assert!(!question.requirements.contains(caller_requirement));
    assert!(!question.semantic_axioms.contains(caller_requirement));
    assert!(matches!(
        terminal_verifier::verify_module(&changed, &lowered.proof_bundle, &AdmissionProfile::default()),
        Err(terminal_verifier::VerificationError::RejectedEvidence { obligation: rejected, .. })
            if rejected == *obligation
    ));
}

#[test]
fn finite_contextual_nominal_cleanup_preserves_caller_superset_and_canonical_artifacts() {
    let tokens = Lexer::new(FINITE_CONTEXTUAL_SOURCE)
        .tokenize()
        .expect("tokenize finite contextual cleanup");
    let syntax = parse_syntax_trees(&tokens).expect("parse finite contextual cleanup");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("resolve finite contextual cleanup");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type finite contextual cleanup source");
    let checked = lower_typed_trees(typed).expect("check finite contextual cleanup source");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
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
        semantic_vocabulary::Proposition::Equal(
            semantic_vocabulary::ScalarTerm::boolean(true),
            semantic_vocabulary::ScalarTerm::boolean_field(parameter.place, ready),
        ),
        semantic_vocabulary::Proposition::Equal(
            semantic_vocabulary::ScalarTerm::boolean(true),
            semantic_vocabulary::ScalarTerm::boolean_field(parameter.place, audited),
        ),
        semantic_vocabulary::Proposition::Equal(
            semantic_vocabulary::ScalarTerm::boolean(true),
            semantic_vocabulary::ScalarTerm::boolean_field(parameter.place, armed),
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
            semantic_vocabulary::Proposition::Equal(
                semantic_vocabulary::ScalarTerm::boolean(true),
                semantic_vocabulary::ScalarTerm::boolean_field(receiver, ready),
            ),
            semantic_vocabulary::Proposition::Equal(
                semantic_vocabulary::ScalarTerm::boolean(true),
                semantic_vocabulary::ScalarTerm::boolean_field(receiver, armed),
            ),
        ],
        "the cleanup target retains only its canonical requirement subset",
    );
    assert_eq!(lowered.proof_bundle.evidence.len(), 2);
    let questions = terminal_verifier::reconstruct_terminal_obligations(&lowered.semantic_module)
        .expect("reconstruct live cleanup premises");
    for (obligation_index, evidence) in lowered.proof_bundle.evidence.iter().enumerate() {
        assert_eq!(
            evidence.obligation,
            cleanup.requirement_obligations[obligation_index]
        );
        let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
            panic!("contextual cleanup evidence is certificate-derived")
        };
        let requirement_position = [0, 2][obligation_index];
        assert_eq!(
            certificate.proof.conclusion,
            caller_requires[requirement_position]
        );
        let question = questions
            .obligations()
            .iter()
            .find(|question| question.obligation.id == evidence.obligation)
            .expect("each cleanup requirement has its own question");
        assert!(question.requirements.is_empty());
        let ProofRule::SemanticAxiom { index } = certificate.proof.rule else {
            panic!("cleanup proves the live observation rather than a permanent assumption")
        };
        assert_eq!(
            question.semantic_axioms.get(index),
            Some(&certificate.proof.conclusion)
        );
    }

    terminal_verifier::verify_module(
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
    let proof_bytes = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("finite contextual proof bundle encodes");
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
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("resolve caller-only contextual cleanup");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type caller-only contextual cleanup source");
    let checked = lower_typed_trees(typed).expect("check caller-only contextual cleanup source");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
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
    let semantic_vocabulary::Proposition::Equal(
        semantic_vocabulary::ScalarTerm::Boolean(true),
        semantic_vocabulary::ScalarTerm::BooleanField { root, .. },
    ) = caller_requirement
    else {
        panic!("caller-only contextual fact retains its Boolean-field shape")
    };
    assert_eq!(*root, parameter.place);

    terminal_verifier::verify_module(
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
    let proof_bytes = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
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
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
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

    terminal_verifier::verify_module(
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
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
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

    terminal_verifier::verify_module(
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
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("resolve contextual two-root cleanup");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type contextual two-root cleanup source");
    let checked = lower_typed_trees(typed).expect("check contextual two-root cleanup source");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
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
    terminal_verifier::verify_module(
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
    let proof_bytes = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof bundle encodes");
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
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
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
                arguments,
                erased_arguments: _,
                callee,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } = &operation.kind
            else {
                panic!("cleanup helper operation remains an ordinary Unit call")
            };
            assert!(arguments.is_empty());
            assert!(structural_arguments.is_empty());
            assert!(claim_transfers.is_empty());
            assert!(requirement_obligations.is_empty());
            assert!(crash_continuations.is_empty());
            *callee
        });
    assert_ne!(helper_callees[0], helper_callees[1]);
    assert_ne!(helper_callees[0], helper_callees[2]);
    assert_ne!(helper_callees[1], helper_callees[2]);

    terminal_verifier::verify_module(
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
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
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

    terminal_verifier::verify_module(
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
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
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

    terminal_verifier::verify_module(
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
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("resolve contextual executable cleanup");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type contextual executable cleanup");
    let checked = lower_typed_trees(typed).expect("check contextual executable cleanup");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
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

    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("contextual executable cleanup verifies");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
    let proof_bytes = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof bundle encodes");
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
    let measured = interpret_terminal_artifact_measured(
        &bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &structural_arguments,
            ..Default::default()
        },
        &mut handler,
    )
    .expect("contextual executable cleanup interprets from canonical artifact sections");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), 7);
    assert!(measured.effects().is_empty());
}
