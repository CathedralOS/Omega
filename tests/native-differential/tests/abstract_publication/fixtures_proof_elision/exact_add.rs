//! Exact-add proof-elision fixture shared by live-result and discarded-result cases.

use super::*;

pub(in super::super) fn exact_add_verified_with_result(
    return_result: bool,
) -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_011).unwrap();
    let block = BlockId::new(1_012).unwrap();
    let left = ValueId::new(1_013).unwrap();
    let right = ValueId::new(1_014).unwrap();
    let computed = ValueId::new(1_015).unwrap();
    let result = ValueId::new(1_016).unwrap();
    let obligation = ObligationId::new(1_017).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let declaration = |id| ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type,
    };
    let machine_result = if return_result {
        TerminalMachineResult::Scalar(declaration(result))
    } else {
        TerminalMachineResult::Unit
    };
    let terminator = if return_result {
        Terminator::Return {
            cleanup_actions: Vec::new(),
            edge: EdgeId::new(1_021).unwrap(),
            value: computed,
        }
    } else {
        Terminator::ReturnUnit {
            edge: EdgeId::new(1_021).unwrap(),
            trivial_affine_discards: Vec::new(),
        }
    };
    verified(
        module_with_blocks(
            machine,
            block,
            machine_result,
            vec![Block {
                structural_parameters: Vec::new(),
                id: block,
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        static_reach_binding: None,
                        id: OperationId::new(1_018).unwrap(),
                        result: OperationResult::Scalar(declaration(left)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(7),
                        },
                    },
                    Operation {
                        static_reach_binding: None,
                        id: OperationId::new(1_019).unwrap(),
                        result: OperationResult::Scalar(declaration(right)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(8),
                        },
                    },
                    Operation {
                        static_reach_binding: None,
                        id: OperationId::new(1_020).unwrap(),
                        result: OperationResult::Scalar(declaration(computed)),
                        kind: OperationKind::ExactIntegerAdd {
                            left,
                            right,
                            obligation,
                        },
                    },
                ],
                terminator,
            }],
        ),
        ProofBundle {
            recursive_components: Vec::new(),
            control_cycles: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            }],
        },
    )
}
