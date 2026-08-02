use omega_interpreter::{TerminalScalarValue, interpret_terminal};
use psi_core::{
    BlockId, ContractId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType, IntegerValue,
    MachineId, ObligationId, OperationId, Proposition, ScalarTerm, ScalarType, ValueId,
};
use psi_proof_kernel::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemVersion,
};
use psi_terminal::{
    Block, ContractClause, MachineContract, Operation, OperationKind, SemanticVersion,
    TerminalMachine, TerminalModule, Terminator, ValueDeclaration,
};
use psi_terminal_verifier::{ObligationEvidence, ProofBundle, verify_module};

#[test]
fn verified_integer_control_contract_slice_executes_directly() {
    let integer = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let scalar_type = ScalarType::Integer(integer);
    let constant = ValueId::new(1).expect("constant");
    let forwarded = ValueId::new(2).expect("forwarded");
    let result = ValueId::new(3).expect("result");
    let obligation = ObligationId::new(1).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let seven = || ScalarTerm::integer(integer, IntegerValue::Signed(7)).expect("seven");
    let goal = Proposition::Equal(term(result), seven());

    let machine = TerminalMachine {
        id: MachineId::new(1).expect("machine"),
        parameters: Vec::new(),
        result: ValueDeclaration {
            id: result,
            scalar_type,
        },
        entry: BlockId::new(1).expect("entry"),
        blocks: vec![
            Block {
                id: BlockId::new(1).expect("entry"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(1).expect("operation"),
                    result: ValueDeclaration {
                        id: constant,
                        scalar_type,
                    },
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Signed(7),
                    },
                }],
                terminator: Terminator::Jump {
                    edge: EdgeId::new(1).expect("jump"),
                    target: BlockId::new(2).expect("exit"),
                    arguments: vec![constant],
                },
            },
            Block {
                id: BlockId::new(2).expect("exit"),
                parameters: vec![ValueDeclaration {
                    id: forwarded,
                    scalar_type,
                }],
                operations: Vec::new(),
                terminator: Terminator::Return {
                    edge: EdgeId::new(2).expect("return"),
                    value: forwarded,
                },
            },
        ],
        contract: MachineContract {
            id: ContractId::new(1).expect("contract"),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
        },
    };
    let module = TerminalModule {
        semantic_version: SemanticVersion::CURRENT,
        entry: machine.id,
        machines: vec![machine],
    };
    let constant_fact = Proposition::Equal(term(constant), seven());
    let forwarding_fact = Proposition::Equal(term(forwarded), term(constant));
    let return_fact = Proposition::Equal(term(result), term(forwarded));
    let forwarded_is_seven = Proposition::Equal(term(forwarded), seven());
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: return_fact,
                rule: ProofRule::SemanticAxiom { index: 2 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: forwarded_is_seven,
                rule: ProofRule::EqualityTransitivity {
                    left_equals_middle: Box::new(ProofNode {
                        conclusion: forwarding_fact,
                        rule: ProofRule::SemanticAxiom { index: 1 },
                    }),
                    middle_equals_right: Box::new(ProofNode {
                        conclusion: constant_fact,
                        rule: ProofRule::SemanticAxiom { index: 0 },
                    }),
                },
            }),
        },
    };
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).expect("certificate"),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof,
            }),
        }],
    };
    let verified = verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("module verifies without source or producer state");

    assert_eq!(
        interpret_terminal(&verified, &[]).expect("verified module executes"),
        TerminalScalarValue::Integer {
            scalar_type: integer,
            value: IntegerValue::Signed(7),
        }
    );
}
