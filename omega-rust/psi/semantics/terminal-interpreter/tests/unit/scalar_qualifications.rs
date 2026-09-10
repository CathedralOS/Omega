use super::*;
use semantic_vocabulary::{DomainSemanticId, ScalarDomainId, ScalarQualificationSetId};
use terminal_psi::{ScalarDomainDeclaration, ScalarQualificationCoercion, ScalarQualificationSet};

fn value(raw: u64, qualified: bool) -> ValueDeclaration {
    ValueDeclaration {
        id: value_id(raw),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        qualifications: ScalarQualificationSetId::new(u64::from(qualified)),
    }
}

fn module() -> TerminalModule {
    let mut module = unit_module();
    let machine = &mut module.machines[0];
    machine.parameters = vec![value(1, false)];
    machine.result = TerminalMachineResult::Scalar(value(3, true));
    machine.blocks[0].terminator = Terminator::Jump {
        edge: edge_id(1),
        target: block_id(2),
        arguments: vec![value_id(1)],
        structural_arguments: vec![],
        trivial_affine_discards: vec![],
        residual_affine_discards: vec![],
    };
    machine.blocks.push(Block {
        id: block_id(2),
        parameters: vec![value(2, true)],
        structural_parameters: vec![],
        operations: vec![],
        terminator: Terminator::Return {
            edge: edge_id(2),
            value: value_id(2),
            cleanup_actions: vec![],
        },
    });
    module
        .scalar_qualifications
        .domains
        .push(ScalarDomainDeclaration {
            id: ScalarDomainId::new(1).unwrap(),
            semantic_domain: DomainSemanticId::new(1).unwrap(),
            identity: "test::Km<u64>".into(),
            carrier: value(1, false).scalar_type,
        });
    module
        .scalar_qualifications
        .sets
        .push(ScalarQualificationSet {
            id: ScalarQualificationSetId::new(1),
            domains: vec![ScalarDomainId::new(1).unwrap()],
        });
    module
        .scalar_qualifications
        .coercions
        .push(ScalarQualificationCoercion {
            machine: machine.id,
            edge: edge_id(1),
            argument_ordinal: 0,
            source: value_id(1),
            destination: value_id(2),
        });
    module
}

#[test]
fn canonical_scalar_qualification_transports_payload_without_an_operation() {
    let module = module();
    let semantic = encode_module(&module).unwrap();
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    let decoded = decode_module(&semantic).unwrap();
    assert_eq!(decoded, module);
    assert_eq!(
        decoded.machines[0].result.scalar().unwrap().qualifications,
        ScalarQualificationSetId::new(1)
    );
    for raw in [0, 7, u64::MAX] {
        let argument = TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            value: IntegerValue::Unsigned(u128::from(raw)),
        };
        let execution = interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument],
        )
        .unwrap();
        assert_eq!(execution.value(), TerminalExecutionResult::Scalar(argument));
        assert_eq!(execution.usage().total_units(), 2);
    }
}

#[test]
fn canonical_qualified_root_parameters_require_host_membership_evidence() {
    let mut module = module();
    module.machines[0].parameters[0].qualifications = ScalarQualificationSetId::new(1);
    module.scalar_qualifications.coercions.clear();
    let semantic = encode_module(&module).unwrap();
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    let argument = TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(7),
    };
    assert!(matches!(
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument],
        ),
        Err(
            terminal_interpreter::TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::ScalarEntryQualificationUnsupported
            )
        )
    ));
}
