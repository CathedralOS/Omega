//! Raw proposed fragment controls for publication projection, not sealed proofs.

use super::{attribution, control};
use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractParameter, AbstractResult, AbstractSuccessor,
};
use machine_code::{
    FunctionFragment, FunctionFragmentBlockSpan, FunctionFragmentConditionalBranchEvidence,
    FunctionFragmentConditionalBranchPredicate, FunctionFragmentControlProvenance,
    FunctionFragmentInstructionSpan, FunctionFragmentSuccessorProvenance,
    ScalarControlFlowEvidence,
};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, SelectedBlockId, SelectedInstructionId, SelectedInstructionProvenance,
};
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId, ScalarType,
    ValueId,
};
use target_operations::TerminalPsiProvenance;

fn source() -> AbstractFunction {
    let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let result = ValueId::new(9).unwrap();
    let successor = |edge, block| AbstractSuccessor {
        psi_edge: EdgeId::new(edge).unwrap(),
        target: BlockId::new(block).unwrap(),
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let constant = |operation, value, literal| AbstractOperation::IntegerConstant {
        psi_operation: OperationId::new(operation).unwrap(),
        result: ValueId::new(value).unwrap(),
        scalar_type: integer,
        value: IntegerValue::Unsigned(literal),
    };
    let returned = |edge, value| AbstractOperation::Return {
        psi_edge: EdgeId::new(edge).unwrap(),
        result,
        value: ValueId::new(value).unwrap(),
        scalar_type: integer,
        cleanup_actions: Vec::new(),
    };
    AbstractFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        entry: BlockId::new(1).unwrap(),
        parameters: [1, 2]
            .into_iter()
            .map(|value| AbstractParameter {
                value: ValueId::new(value).unwrap(),
                scalar_type: integer,
            })
            .collect(),
        structural_parameters: Vec::new(),
        result: AbstractFunctionResult::Scalar(AbstractResult {
            value: result,
            scalar_type: integer,
        }),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: [(1, 0), (2, 2), (3, 4)]
            .into_iter()
            .map(|(block, operation_offset)| AbstractBlockEntry {
                block: BlockId::new(block).unwrap(),
                operation_offset,
                parameters: Vec::new(),
            })
            .collect(),
        operations: vec![
            AbstractOperation::IntegerLessThan {
                psi_operation: OperationId::new(10).unwrap(),
                result: ValueId::new(3).unwrap(),
                left: ValueId::new(1).unwrap(),
                right: ValueId::new(2).unwrap(),
            },
            AbstractOperation::Conditional {
                condition: ValueId::new(3).unwrap(),
                when_true: successor(1, 2),
                when_false: successor(2, 3),
            },
            constant(11, 4, 7),
            returned(3, 4),
            constant(12, 5, 0),
            returned(4, 5),
        ],
    }
}

fn fragment() -> FunctionFragment {
    let bytes = vec![
        0x48, 0x39, 0xf7, 0x72, 0x06, 0xb8, 0, 0, 0, 0, 0xc3, 0xb8, 7, 0, 0, 0, 0xc3,
    ];
    let span = |id, family, offset: usize, length, operation: Option<u64>| {
        FunctionFragmentInstructionSpan {
            instruction: SelectedInstructionId(id),
            alternative: MachineAlternativeKey { family, variant: 0 },
            offset: offset as u64,
            bytes: bytes[offset..offset + length].to_vec(),
            branch: None,
            internal_machine_fixup: None,
            provenance: SelectedInstructionProvenance {
                operations: operation
                    .into_iter()
                    .map(|value| OperationId::new(value).unwrap())
                    .collect(),
                ..Default::default()
            },
            control: FunctionFragmentControlProvenance::None,
        }
    };
    let successor = |edge, block, source_target| FunctionFragmentSuccessorProvenance {
        psi_edge: EdgeId::new(edge).unwrap(),
        block: SelectedBlockId(block),
        source_target: BlockId::new(source_target).unwrap(),
        bindings: Vec::new(),
        fuel: Vec::new(),
    };
    let mut branch = span(
        1,
        MachineAlternativeFamily::ConditionalBranchU64LessThan,
        3,
        2,
        None::<u64>,
    );
    branch.control = FunctionFragmentControlProvenance::ConditionalBranch {
        predicate: FunctionFragmentConditionalBranchPredicate::U64LessThanV1,
        when_taken: successor(1, 1, 2),
        when_fallthrough: successor(2, 2, 3),
    };
    let mut effects = MachineEncodedEffects::fallthrough_v1(Vec::new(), Vec::new());
    effects.control = MachineEncodedControlEffect::ConditionalRelativeBranchV1;
    branch.branch = Some(Box::new(FunctionFragmentConditionalBranchEvidence {
        predicate: FunctionFragmentConditionalBranchPredicate::U64LessThanV1,
        source_block: SelectedBlockId(0),
        when_taken_edge: EdgeId::new(1).unwrap(),
        when_taken_block: SelectedBlockId(1),
        when_taken_offset: 11,
        when_fallthrough_edge: EdgeId::new(2).unwrap(),
        when_fallthrough_block: SelectedBlockId(2),
        when_fallthrough_offset: 5,
        byte_displacement: 6,
        decoded_register_reads: Vec::new(),
        decoded_effects: effects,
    }));
    let mut returned_true = span(3, MachineAlternativeFamily::ReturnI64, 16, 1, None);
    returned_true.control = FunctionFragmentControlProvenance::Return {
        psi_return_edge: EdgeId::new(3).unwrap(),
    };
    let mut returned_false = span(5, MachineAlternativeFamily::ReturnI64, 10, 1, None);
    returned_false.control = FunctionFragmentControlProvenance::Return {
        psi_return_edge: EdgeId::new(4).unwrap(),
    };
    let blocks = vec![
        FunctionFragmentBlockSpan {
            block: SelectedBlockId(0),
            offset: 0,
            byte_count: 5,
            instructions: vec![
                span(0, MachineAlternativeFamily::CompareI64, 0, 3, Some(10)),
                branch,
            ],
        },
        FunctionFragmentBlockSpan {
            block: SelectedBlockId(1),
            offset: 11,
            byte_count: 6,
            instructions: vec![
                span(2, MachineAlternativeFamily::MaterializeI64, 11, 5, Some(11)),
                returned_true,
            ],
        },
        FunctionFragmentBlockSpan {
            block: SelectedBlockId(2),
            offset: 5,
            byte_count: 6,
            instructions: vec![
                span(4, MachineAlternativeFamily::MaterializeI64, 5, 5, Some(12)),
                returned_false,
            ],
        },
    ];
    FunctionFragment {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        provenance: TerminalPsiProvenance {
            operations: [10, 11, 12]
                .into_iter()
                .map(|value| OperationId::new(value).unwrap())
                .collect(),
            edges: [1, 2, 3, 4]
                .into_iter()
                .map(|value| EdgeId::new(value).unwrap())
                .collect(),
        },
        byte_count: bytes.len() as u64,
        bytes,
        blocks,
    }
}

#[test]
fn selected_conditional_preserves_taken_polarity_and_current_operation_ordinals() {
    let fragment = fragment();
    let source = source();
    let ScalarControlFlowEvidence::DirectConditional { branch } =
        control::project(&fragment, &source).unwrap()
    else {
        panic!("selected predicate custody")
    };
    assert_eq!(
        (
            branch.branch_offset,
            branch.branch_byte_count,
            branch.fallthrough_offset,
            branch.taken_offset
        ),
        (3, 2, 5, 11)
    );
    assert_eq!(
        branch.predicate,
        FunctionFragmentConditionalBranchPredicate::U64LessThanV1
    );
    let rows = attribution::project(&fragment, &source).unwrap();
    assert_eq!(
        rows.iter()
            .map(|row| (row.operation_ordinal, row.code_offset, row.byte_count))
            .collect::<Vec<_>>(),
        [
            (0, 0, 3),
            (1, 3, 2),
            (1, 5, 0),
            (2, 11, 5),
            (3, 16, 1),
            (4, 5, 5),
            (5, 10, 1)
        ]
    );
}

#[test]
fn selected_conditional_rejects_changed_successors_and_foreign_sites() {
    let source = source();
    let mut wrong_offset = fragment();
    wrong_offset.blocks[0].instructions[1]
        .branch
        .as_mut()
        .unwrap()
        .when_taken_offset = 5;
    assert!(control::project(&wrong_offset, &source).is_err());
    let mut wrong_predicate = fragment();
    wrong_predicate.blocks[0].instructions[1]
        .branch
        .as_mut()
        .unwrap()
        .predicate = FunctionFragmentConditionalBranchPredicate::NonZeroV1;
    assert!(control::project(&wrong_predicate, &source).is_err());
    let mut foreign_operation = fragment();
    foreign_operation.blocks[1].instructions[0]
        .provenance
        .operations[0] = OperationId::new(99).unwrap();
    assert!(attribution::project(&foreign_operation, &source).is_err());
}
