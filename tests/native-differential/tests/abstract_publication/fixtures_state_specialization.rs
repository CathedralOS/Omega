//! State-specialization fixture: a dispatch block parameter bound by two
//! incoming unconditional edges, one supplying a proven constant.

use super::{
    module_with_blocks, verified, Block, BlockId, EdgeId, MachineId, Operation, OperationId,
    OperationKind, OperationResult, ProofBundle, ScalarType, SuccessorEdge, TerminalMachineResult,
    Terminator, ValueDeclaration, ValueId, VerifiedPsiOptimizationUnit,
};

fn boolean(id: u64) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(id).unwrap(),
        scalar_type: ScalarType::Boolean,
    }
}

fn successor(edge: u64, target: BlockId, arguments: Vec<ValueId>) -> SuccessorEdge {
    SuccessorEdge {
        erased_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        edge: EdgeId::new(edge).unwrap(),
        target,
        arguments,
        trivial_affine_discards: Vec::new(),
    }
}

fn jump(edge: u64, target: BlockId, arguments: Vec<ValueId>) -> Terminator {
    Terminator::Jump {
        erased_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        edge: EdgeId::new(edge).unwrap(),
        target,
        arguments,
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

fn return_unit(edge: u64) -> Terminator {
    Terminator::ReturnUnit {
        edge: EdgeId::new(edge).unwrap(),
        trivial_affine_discards: Vec::new(),
    }
}

/// `entry -[seed]-> {pred_true, pred_false} -> dispatch(state) -> {yes, no}`:
/// `pred_true` supplies the proven literal `flag`; `pred_false` supplies the
/// machine parameter `seed`, which no sparse constant lattice can prove.
/// Exactly the constant-supplied edge specializes.
pub(super) fn dispatch_specialization_verified() -> VerifiedPsiOptimizationUnit {
    let (entry, pred_true, pred_false, dispatch, yes, no) = (
        BlockId::new(1_902).unwrap(),
        BlockId::new(1_903).unwrap(),
        BlockId::new(1_904).unwrap(),
        BlockId::new(1_905).unwrap(),
        BlockId::new(1_906).unwrap(),
        BlockId::new(1_907).unwrap(),
    );
    let seed = ValueId::new(1_908).unwrap();
    let flag = ValueId::new(1_909).unwrap();
    let state = ValueId::new(1_910).unwrap();
    let machine = MachineId::new(1_901).unwrap();
    let mut module = module_with_blocks(
        machine,
        entry,
        TerminalMachineResult::Unit,
        vec![
            Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: entry,
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(1_911).unwrap(),
                    result: OperationResult::Scalar(boolean(1_909)),
                    kind: OperationKind::BooleanConstant { value: true },
                }],
                terminator: Terminator::Conditional {
                    condition: seed,
                    when_true: successor(1_912, pred_true, Vec::new()),
                    when_false: successor(1_913, pred_false, Vec::new()),
                },
            },
            Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: pred_true,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: jump(1_914, dispatch, vec![flag]),
            },
            Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: pred_false,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: jump(1_915, dispatch, vec![seed]),
            },
            Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: dispatch,
                parameters: vec![boolean(1_910)],
                operations: Vec::new(),
                terminator: Terminator::Conditional {
                    condition: state,
                    when_true: successor(1_916, yes, Vec::new()),
                    when_false: successor(1_917, no, Vec::new()),
                },
            },
            Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: yes,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: return_unit(1_918),
            },
            Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: no,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: return_unit(1_919),
            },
        ],
    );
    module.machines[0].parameters = vec![boolean(1_908)];
    verified(module, ProofBundle::default())
}
