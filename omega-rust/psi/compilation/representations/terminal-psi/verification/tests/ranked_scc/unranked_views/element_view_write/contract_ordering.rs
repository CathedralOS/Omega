//! Borrowed windows consume exact scalar call contracts, never raw callee IDs.
use super::*;
use semantic_vocabulary::{BlockId, ContractId, EdgeId, IntegerValue, MachineId};
use terminal_psi::{ContractClause, MachineContract, TerminalMachineResult, Terminator};

fn scalar(value: u64) -> ValueDeclaration {
    ValueDeclaration {
        id: id(value, ValueId::new),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        qualifications: Default::default(),
    }
}

fn term(value: u64) -> ScalarTerm {
    ScalarTerm::value(id(value, ValueId::new), scalar(value).scalar_type)
}

fn one() -> ScalarTerm {
    ScalarTerm::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(1),
    }
}

fn contract_window() -> TerminalModule {
    let mut module = mutable_child_fixture();
    let machine = &mut module.machines[0];
    machine.contract.requires = vec![Proposition::LessOrEqual(one(), term(1))];
    machine.blocks[0].operations.extend([
        Operation {
            id: id(100, OperationId::new),
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Scalar(scalar(100)),
            kind: OperationKind::Call {
                callee: id(900, MachineId::new),
                arguments: vec![id(1, ValueId::new)],
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                requirement_obligations: vec![id(900, ObligationId::new)],
                crash_continuations: Vec::new(),
            },
        },
        Operation {
            id: id(101, OperationId::new),
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Scalar(scalar(101)),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        },
    ]);
    for block in &mut machine.blocks {
        for operation in &mut block.operations {
            match &mut operation.kind {
                OperationKind::ElementViewSubslice { start, .. } => *start = id(100, ValueId::new),
                OperationKind::ElementViewWrite { index, .. } => *index = id(101, ValueId::new),
                _ => {}
            }
        }
    }
    machine.blocks[1].operations.swap(1, 2);
    let mut callee = machine.clone();
    callee.id = id(900, MachineId::new);
    callee.parameters = vec![scalar(900)];
    callee.structural_parameters.clear();
    callee.structural_places.clear();
    callee.result = TerminalMachineResult::Scalar(scalar(901));
    callee.entry = id(900, BlockId::new);
    callee.blocks.truncate(1);
    callee.blocks[0].id = callee.entry;
    callee.blocks[0].operations.clear();
    callee.blocks[0].terminator = Terminator::Return {
        edge: id(900, EdgeId::new),
        value: id(900, ValueId::new),
        cleanup_actions: Vec::new(),
    };
    callee.contract = MachineContract {
        id: id(900, ContractId::new),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        requires: vec![Proposition::LessOrEqual(one(), term(900))],
        ensures: vec![ContractClause {
            obligation: id(901, ObligationId::new),
            proposition: Proposition::Equal(term(901), term(900)),
        }],
        outcome_specific_ensures: Vec::new(),
        crash_routes: Vec::new(),
    };
    module.machines.push(callee);
    module
}

#[test]
fn contract_ordering_replays_entry_and_normal_result_bounds_for_live_windows() {
    let mut module = contract_window();
    validate_module(&module).expect("index zero is below the exact returned window start");
    module.machines.reverse();
    validate_module(&module).expect("callee visitation order grants no extra authority");
}

#[test]
fn contract_ordering_rejects_missing_conditional_and_unmapped_window_bounds() {
    let original = contract_window();
    validate_module(&original).unwrap();
    for mutation in 0..4 {
        let mut changed = original.clone();
        match mutation {
            0 => changed.machines[0].contract.requires.clear(),
            1 => changed.machines[1].contract.ensures.clear(),
            2 => {
                let clause = &mut changed.machines[1].contract.ensures[0];
                clause.proposition = Proposition::Implication {
                    premise: Box::new(Proposition::Truth),
                    conclusion: Box::new(clause.proposition.clone()),
                };
            }
            3 => {
                changed.machines[1].contract.ensures[0].proposition =
                    Proposition::Equal(term(901), term(1))
            }
            _ => unreachable!(),
        }
        assert!(
            validate_module(&changed).is_err(),
            "unsupported window authority {mutation}"
        );
    }
}
