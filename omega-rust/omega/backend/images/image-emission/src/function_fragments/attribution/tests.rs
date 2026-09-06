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
fn disjoint_same_site_spans_are_not_widened_over_another_operation() {
    let (mut fragment, source) = fixture();
    let instructions = &mut fragment.blocks[0].instructions;
    instructions[1].provenance.operations = vec![OperationId::new(2).unwrap()];
    instructions[2].provenance.operations = vec![OperationId::new(1).unwrap()];
    assert!(produce(&fragment, &source).is_err());
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
