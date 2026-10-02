//! Scalar ordering assumptions never replace the complete PCC contract question.
use super::*;
use semantic_vocabulary::IntegerValue;
use terminal_verifier::VerificationError;

fn integer() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}

fn term(raw: u64) -> ScalarTerm {
    ScalarTerm::value(value_id(raw), ScalarType::Integer(integer()))
}

fn constant(value: u128) -> ScalarTerm {
    ScalarTerm::Integer {
        scalar_type: integer(),
        value: IntegerValue::Unsigned(value),
    }
}

fn bounded_call() -> (TerminalModule, ProofBundle) {
    let mut module = call_module();
    for machine in &mut module.machines {
        for parameter in &mut machine.parameters {
            parameter.scalar_type = ScalarType::Integer(integer());
        }
        if let TerminalMachineResult::Scalar(result) = &mut machine.result {
            result.scalar_type = ScalarType::Integer(integer());
        }
        for operation in machine
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
        {
            if let OperationResult::Scalar(result) = &mut operation.result {
                result.scalar_type = ScalarType::Integer(integer());
            }
            if matches!(operation.kind, OperationKind::BooleanConstant { .. }) {
                operation.kind = OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(2),
                };
            }
        }
    }
    module.machines[1].contract.requires = vec![Proposition::Equal(term(4), constant(2))];
    module.machines[1].contract.ensures[0].proposition = Proposition::Equal(term(5), term(4));
    let proof = ProofBundle {
        evidence: vec![
            semantic_axiom_evidence(
                obligation_id(1),
                Proposition::Equal(term(1), constant(2)),
                0,
                1,
            ),
            semantic_axiom_evidence(obligation_id(2), Proposition::Equal(term(5), term(4)), 0, 2),
        ],
        ..ProofBundle::default()
    };
    (module, proof)
}

#[test]
fn scalar_ordering_contracts_require_complete_call_and_guarantee_evidence() {
    let (module, proof) = bounded_call();
    verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("exact result equality establishes the declared bound");
    let mut missing = proof.clone();
    missing.evidence.remove(0);
    assert!(matches!(
        verify_module(&module, &missing, &AdmissionProfile::default()),
        Err(VerificationError::MissingEvidence(obligation)) if obligation == obligation_id(1)
    ));

    let mut false_guarantee = module;
    false_guarantee.machines[1].blocks[0]
        .operations
        .push(Operation {
            id: operation_id(3),
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Scalar(ValueDeclaration {
                id: value_id(6),
                scalar_type: ScalarType::Integer(integer()),
                qualifications: Default::default(),
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        });
    let Terminator::Return { value, .. } = &mut false_guarantee.machines[1].blocks[0].terminator
    else {
        unreachable!();
    };
    *value = value_id(6);
    validate_module(&false_guarantee).expect("a declaration is not logical guarantee evidence");
    assert!(
        verify_module(&false_guarantee, &proof, &AdmissionProfile::default()).is_err(),
        "the original certificate cannot make zero equal the input two"
    );
}
