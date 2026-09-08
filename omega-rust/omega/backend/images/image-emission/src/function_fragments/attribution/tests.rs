//! These fixtures test semantic interval replay only, not instruction encoding
//! or admission. The integrated object tests retain real staged machine custody.

use super::*;
use abstract_operations::{AbstractFunctionResult, AbstractOperation};
use machine_code::{FunctionFragmentBlockSpan, FunctionFragmentInstructionSpan};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, SelectedBlockId, SelectedInstructionId,
};
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId, ScalarType,
    ValueId,
};

fn fixture() -> (FunctionFragment, AbstractFunction) {
    let first = OperationId::new(1).unwrap();
    let second = OperationId::new(2).unwrap();
    let edge = EdgeId::new(3).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let source = AbstractFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        entry: BlockId::new(1).unwrap(),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: Vec::new(),
        operations: vec![
            AbstractOperation::IntegerConstant {
                psi_operation: first,
                result: ValueId::new(1).unwrap(),
                scalar_type,
                value: IntegerValue::Unsigned(1),
            },
            AbstractOperation::IntegerConstant {
                psi_operation: second,
                result: ValueId::new(2).unwrap(),
                scalar_type,
                value: IntegerValue::Unsigned(2),
            },
            AbstractOperation::ReturnUnit {
                psi_edge: edge,
                cleanup_actions: Vec::new(),
            },
        ],
    };
    let mut instructions = Vec::new();
    for (index, (offset, length, operation)) in [
        (0, 2, Some(first)),
        (2, 3, Some(first)),
        (5, 2, Some(second)),
        (7, 1, None),
    ]
    .into_iter()
    .enumerate()
    {
        instructions.push(FunctionFragmentInstructionSpan {
            instruction: SelectedInstructionId(index as u32),
            alternative: MachineAlternativeKey {
                family: MachineAlternativeFamily::MaterializeI64,
                variant: 0,
            },
            offset,
            bytes: vec![0x90; length],
            branch: None,
            internal_machine_fixup: None,
            provenance: selected_instructions::SelectedInstructionProvenance {
                operations: operation.into_iter().collect(),
                ..Default::default()
            },
            control: if operation.is_some() {
                Control::None
            } else {
                Control::Return {
                    psi_return_edge: edge,
                }
            },
        });
    }
    let fragment = FunctionFragment {
        machine: source.machine,
        attachment: None,
        provenance: Default::default(),
        byte_count: 8,
        bytes: vec![0x90; 8],
        blocks: vec![FunctionFragmentBlockSpan {
            block: SelectedBlockId(0),
            offset: 0,
            byte_count: 8,
            instructions,
        }],
    };
    (fragment, source)
}

#[test]
fn case_edges_share_the_authored_case_ordinal_without_fabricated_operations() {
    let (_, mut source) = fixture();
    source.operations[2] = AbstractOperation::StructuralCase {
        source: semantic_vocabulary::PlaceId::new(1).unwrap(),
        cases: [10, 11]
            .into_iter()
            .map(
                |identity| abstract_operations::AbstractStructuralCaseSuccessor {
                    psi_edge: EdgeId::new(identity).unwrap(),
                    target: BlockId::new(identity).unwrap(),
                    case: semantic_vocabulary::StructuralCaseId::new(identity).unwrap(),
                    payloads: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            )
            .collect(),
    };
    for identity in [10, 11] {
        let edge = EdgeId::new(identity).unwrap();
        assert_eq!(ordinal(&source, SemanticCodeSite::Edge(edge)).unwrap(), 2);
        assert!(
            ordinal(
                &source,
                SemanticCodeSite::Operation(OperationId::new(identity).unwrap())
            )
            .is_err()
        );
    }
    assert!(ordinal(&source, SemanticCodeSite::Edge(EdgeId::new(99).unwrap())).is_err());
    source.operations.push(source.operations[2].clone());
    assert!(ordinal(&source, SemanticCodeSite::Edge(EdgeId::new(10).unwrap())).is_err());
}

#[test]
fn process_exit_nominal_edge_is_zero_width_and_never_a_return_span() {
    let (mut fragment, source) = fixture();
    let terminal = fragment.blocks[0].instructions.last_mut().unwrap();
    let Control::Return { psi_return_edge } = &terminal.control else {
        panic!("return fixture");
    };
    let psi_return_edge = *psi_return_edge;
    terminal.control = Control::HostedExitProcess {
        nominal_return_edge: psi_return_edge,
    };
    let end = usize::try_from(terminal.offset).unwrap() + terminal.bytes.len();
    let rows = produce(&fragment, &source).unwrap();
    validate(&fragment, &source, &rows).unwrap();
    let position = rows
        .iter()
        .position(|row| row.site == SemanticCodeSite::Edge(psi_return_edge))
        .unwrap();
    assert_eq!(rows[position].code_offset, end);
    assert_eq!(rows[position].byte_count, 0);
    let mut changed = rows.clone();
    changed[position].byte_count = 1;
    assert!(validate(&fragment, &source, &changed).is_err());
    let mut changed = rows;
    changed[position].code_offset -= 1;
    assert!(validate(&fragment, &source, &changed).is_err());
}

#[test]
fn boolean_constant_attribution_requires_its_exact_operation_ordinal() {
    let (fragment, mut source) = fixture();
    source.operations[0] = AbstractOperation::BooleanConstant {
        psi_operation: OperationId::new(1).unwrap(),
        result: ValueId::new(1).unwrap(),
        value: true,
    };
    let rows = produce(&fragment, &source).unwrap();
    assert_eq!(rows[0].operation_ordinal, 0);
    validate(&fragment, &source, &rows).unwrap();
    let mut substituted = rows.clone();
    substituted[0].operation_ordinal = 1;
    assert!(validate(&fragment, &source, &substituted).is_err());
    source.operations.swap(0, 1);
    assert!(validate(&fragment, &source, &rows).is_err());
}

#[test]
fn contiguous_spans_coalesce_without_absorbing_neighboring_operations() {
    let (fragment, source) = fixture();
    let rows = produce(&fragment, &source).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!((rows[0].code_offset, rows[0].byte_count), (0, 5));
    validate(&fragment, &source, &rows).unwrap();

    let mut extended = rows.clone();
    extended[0].byte_count = 7;
    assert!(validate(&fragment, &source, &extended).is_err());
    let mut shifted = rows.clone();
    shifted[1].code_offset -= 1;
    shifted[1].byte_count += 1;
    assert!(validate(&fragment, &source, &shifted).is_err());
}

#[test]
fn ieee_literal_attribution_requires_complete_exact_operation_membership() {
    for value in [
        semantic_vocabulary::IeeeFloatValue::Binary32(0x7fc0_0041),
        semantic_vocabulary::IeeeFloatValue::Binary64(0x8000_0000_0000_0000),
    ] {
        let (fragment, mut source) = fixture();
        source.operations[0] = AbstractOperation::IeeeFloatConstant {
            psi_operation: OperationId::new(1).unwrap(),
            result: ValueId::new(1).unwrap(),
            value,
        };
        let rows = produce(&fragment, &source).unwrap();
        validate(&fragment, &source, &rows).unwrap();
        assert_eq!(
            (
                rows[0].operation_ordinal,
                rows[0].code_offset,
                rows[0].byte_count
            ),
            (0, 0, 5)
        );
        let mut omitted = rows.clone();
        omitted.remove(0);
        assert!(validate(&fragment, &source, &omitted).is_err());
        source.operations.swap(0, 1);
        assert!(validate(&fragment, &source, &rows).is_err());
    }
}

#[test]
fn omitted_operation_and_return_attribution_reject() {
    let (fragment, source) = fixture();
    let rows = produce(&fragment, &source).unwrap();
    for removed in 0..rows.len() {
        let mut omitted = rows.clone();
        omitted.remove(removed);
        assert!(validate(&fragment, &source, &omitted).is_err());
    }
}

#[test]
fn bridge_continuation_does_not_attribute_the_semantic_edge_twice() {
    let (mut fragment, mut source) = fixture();
    let edge = EdgeId::new(4).unwrap();
    source.operations.push(AbstractOperation::Jump {
        structural_bindings: Vec::new(),
        psi_edge: edge,
        target: BlockId::new(2).unwrap(),
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    });
    let successor = machine_code::FunctionFragmentSuccessorProvenance {
        role: SelectedSuccessorRole::Semantic,
        psi_edge: edge,
        block: SelectedBlockId(1),
        source_target: BlockId::new(2).unwrap(),
        bindings: Vec::new(),
        fuel: Vec::new(),
    };
    let mut jump = fragment.blocks[0].instructions[3].clone();
    jump.instruction = SelectedInstructionId(4);
    jump.offset = 8;
    jump.control = Control::Jump {
        successor: successor.clone(),
    };
    fragment.blocks[0].instructions.push(jump.clone());
    jump.instruction = SelectedInstructionId(5);
    jump.offset = 9;
    jump.control = Control::Jump {
        successor: machine_code::FunctionFragmentSuccessorProvenance {
            role: SelectedSuccessorRole::EdgeTransferContinuation,
            block: SelectedBlockId(2),
            ..successor
        },
    };
    fragment.blocks[0].instructions.push(jump);
    fragment.blocks[0].byte_count = 10;
    fragment.byte_count = 10;
    fragment.bytes.resize(10, 0x90);
    let rows = produce(&fragment, &source).unwrap();
    validate(&fragment, &source, &rows).unwrap();
    let row = rows
        .iter()
        .find(|row| row.site == SemanticCodeSite::Edge(edge))
        .unwrap();
    assert_eq!((row.code_offset, row.byte_count), (8, 1));
    let mut forged = rows.clone();
    let row = forged
        .iter_mut()
        .find(|row| row.site == SemanticCodeSite::Edge(edge))
        .unwrap();
    row.byte_count = 2;
    assert!(validate(&fragment, &source, &forged).is_err());
    let row = forged
        .iter_mut()
        .find(|row| row.site == SemanticCodeSite::Edge(edge))
        .unwrap();
    row.code_offset = 9;
    row.byte_count = 1;
    assert!(validate(&fragment, &source, &forged).is_err());
}

#[test]
fn disjoint_same_site_spans_are_not_widened_over_another_operation() {
    let (mut fragment, source) = fixture();
    let instructions = &mut fragment.blocks[0].instructions;
    instructions[1].provenance.operations = vec![OperationId::new(2).unwrap()];
    instructions[2].provenance.operations = vec![OperationId::new(1).unwrap()];
    let rows = produce(&fragment, &source).unwrap();
    validate(&fragment, &source, &rows).unwrap();
    assert_eq!(rows.len(), 4);
    assert_eq!((rows[0].code_offset, rows[0].byte_count), (0, 2));
    assert_eq!((rows[1].code_offset, rows[1].byte_count), (5, 2));
    let forged = vec![
        SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(OperationId::new(1).unwrap()),
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: 7,
        },
        SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(OperationId::new(2).unwrap()),
            operation_ordinal: 1,
            code_offset: 2,
            byte_count: 3,
        },
        SemanticCodeAttribution {
            site: SemanticCodeSite::Edge(EdgeId::new(3).unwrap()),
            operation_ordinal: 2,
            code_offset: 7,
            byte_count: 1,
        },
    ];
    assert!(validate(&fragment, &source, &forged).is_err());
}

#[test]
fn compiler_spill_gaps_remain_unattributed_and_every_site_interval_is_required() {
    let (mut fragment, source) = fixture();
    let instructions = &mut fragment.blocks[0].instructions;
    instructions[1].provenance = Default::default();
    instructions[2].provenance.operations = vec![OperationId::new(1).unwrap()];
    let rows = produce(&fragment, &source).unwrap();
    validate(&fragment, &source, &rows).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!((rows[0].code_offset, rows[0].byte_count), (0, 2));
    assert_eq!((rows[1].code_offset, rows[1].byte_count), (5, 2));
    for missing in [0, 1] {
        let mut omitted = rows.clone();
        omitted.remove(missing);
        assert!(validate(&fragment, &source, &omitted).is_err());
    }
    let mut widened = rows.clone();
    widened[0].byte_count = 7;
    widened.remove(1);
    assert!(validate(&fragment, &source, &widened).is_err());
    let mut duplicate = rows.clone();
    duplicate.insert(1, rows[0]);
    assert!(validate(&fragment, &source, &duplicate).is_err());
    let mut reordered = rows.clone();
    reordered.swap(0, 1);
    assert!(validate(&fragment, &source, &reordered).is_err());
    let mut shifted = rows.clone();
    shifted[1].code_offset += 1;
    shifted[1].byte_count -= 1;
    assert!(validate(&fragment, &source, &shifted).is_err());
}

#[test]
fn contiguous_membership_cannot_be_forged_as_nonmaximal_intervals() {
    let (fragment, source) = fixture();
    let mut rows = produce(&fragment, &source).unwrap();
    let mut second = rows[0];
    rows[0].byte_count = 2;
    second.code_offset = 2;
    second.byte_count = 3;
    rows.insert(1, second);
    assert!(validate(&fragment, &source, &rows).is_err());
}
