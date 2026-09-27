use super::{FINITE_CONTEXTUAL_SOURCE, SCALAR_SOURCE};
use crate::nominal_affine_source::{
    AdmissionProfile, EvidenceRoute, IntegerSign, IntegerType, ProofRule, ScalarType,
    StructuralFieldType, StructuralTypeShape, Terminator, decode_module, decode_proof_bundle,
    encode_module, encode_proof_section,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;

#[test]
fn finite_contextual_nominal_cleanup_preserves_caller_superset_and_canonical_artifacts() {
    let checked = crate::front_end::checked_program(FINITE_CONTEXTUAL_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
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
fn wide_mixed_primitive_record_crosses_source_lowering_codec_and_verifier() {
    let checked = crate::front_end::checked_program(SCALAR_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
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
