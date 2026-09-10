use super::*;
use semantic_vocabulary::{DomainSemanticId, ScalarDomainId, ScalarQualificationSetId};
use terminal_psi::{ScalarDomainDeclaration, ScalarQualificationSet};

#[test]
fn scalar_qualification_catalog_round_trips_and_commits_semantic_identity() {
    let mut module = unit_fixture();
    module
        .scalar_qualifications
        .domains
        .push(ScalarDomainDeclaration {
            id: ScalarDomainId::new(1).unwrap(),
            semantic_domain: DomainSemanticId::new(7).unwrap(),
            identity: "bool::Tag<7>".into(),
            carrier: ScalarType::Boolean,
        });
    module
        .scalar_qualifications
        .sets
        .push(ScalarQualificationSet {
            id: ScalarQualificationSetId::new(1),
            domains: vec![ScalarDomainId::new(1).unwrap()],
        });
    let bytes = encode_module(&module).unwrap();
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let original = semantic_fingerprint(&module).unwrap();
    module.scalar_qualifications.domains[0].identity = "bool::Tag<8>".into();
    assert_ne!(semantic_fingerprint(&module).unwrap(), original);
    module.scalar_qualifications.domains[0].semantic_domain = DomainSemanticId::new(8).unwrap();
    assert_ne!(encode_module(&module).unwrap(), bytes);
}

#[test]
fn scalar_qualification_catalog_rejects_previous_vocabulary() {
    let mut bytes = encode_module(&unit_fixture()).unwrap();
    bytes[10..12].copy_from_slice(&95_u16.to_le_bytes());
    assert_eq!(
        decode_module(&bytes),
        Err(CodecError::UnsupportedVocabularyMarker(95))
    );
}

#[test]
fn explicit_scalar_erasure_round_trips_and_commits_its_exact_edge() {
    use semantic_vocabulary::{BlockId, EdgeId, ValueId};
    use terminal_psi::{
        Block, ScalarQualificationCoercion, TerminalMachineResult, Terminator, ValueDeclaration,
    };
    let mut module = unit_fixture();
    let set = ScalarQualificationSetId::new(1);
    let source = ValueDeclaration {
        id: ValueId::new(1).unwrap(),
        scalar_type: ScalarType::Boolean,
        qualifications: set,
    };
    let destination = ValueDeclaration {
        id: ValueId::new(2).unwrap(),
        qualifications: ScalarQualificationSetId::ZERO,
        ..source
    };
    let machine = &mut module.machines[0];
    machine.parameters = vec![source];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: ValueId::new(3).unwrap(),
        ..destination
    });
    machine.blocks[0].terminator = Terminator::Jump {
        edge: EdgeId::new(1).unwrap(),
        target: BlockId::new(901).unwrap(),
        arguments: vec![source.id],
        structural_arguments: vec![],
        trivial_affine_discards: vec![],
        residual_affine_discards: vec![],
    };
    machine.blocks.push(Block {
        id: BlockId::new(901).unwrap(),
        parameters: vec![destination],
        structural_parameters: vec![],
        operations: vec![],
        terminator: Terminator::Return {
            edge: EdgeId::new(2).unwrap(),
            value: destination.id,
            cleanup_actions: vec![],
        },
    });
    module
        .scalar_qualifications
        .domains
        .push(ScalarDomainDeclaration {
            id: ScalarDomainId::new(1).unwrap(),
            semantic_domain: DomainSemanticId::new(7).unwrap(),
            identity: "bool::Tag<7>".into(),
            carrier: ScalarType::Boolean,
        });
    module
        .scalar_qualifications
        .sets
        .push(ScalarQualificationSet {
            id: set,
            domains: vec![ScalarDomainId::new(1).unwrap()],
        });
    module
        .scalar_qualifications
        .coercions
        .push(ScalarQualificationCoercion {
            machine: machine.id,
            edge: EdgeId::new(1).unwrap(),
            argument_ordinal: 0,
            source: source.id,
            destination: destination.id,
        });
    terminal_verifier::validate_module_representation(&module).unwrap();
    let bytes = encode_module(&module).unwrap();
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let fingerprint = semantic_fingerprint(&module).unwrap();
    let mut introduction = module.clone();
    introduction.machines[0].parameters[0].qualifications = ScalarQualificationSetId::ZERO;
    introduction.machines[0].blocks[1].parameters[0].qualifications = set;
    introduction.machines[0]
        .result
        .scalar_mut()
        .unwrap()
        .qualifications = set;
    assert_ne!(semantic_fingerprint(&introduction).unwrap(), fingerprint);
    module.scalar_qualifications.coercions[0].destination = source.id;
    assert!(encode_module(&module).is_err());
}
