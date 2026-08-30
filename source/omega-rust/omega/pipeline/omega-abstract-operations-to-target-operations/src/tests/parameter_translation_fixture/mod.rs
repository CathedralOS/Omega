//! Parameter-plan fixtures grouped by direct, unary, and comparison semantics.

use super::*;

mod comparison;
mod direct;
mod unary;

pub(super) use comparison::*;
pub(super) use direct::*;
pub(super) use unary::*;

pub(super) fn integer_type(sign: IntegerSign, bits: u16) -> IntegerType {
    IntegerType::new(sign, bits).expect("test integer type")
}

pub(super) fn parameter_return_plan(
    parameter_types: &[ScalarType],
    returned_parameter: usize,
) -> AbstractOperationPlan {
    let machine = MachineId::new(3_001).unwrap();
    let entry = BlockId::new(3_002).unwrap();
    let result_value = ValueId::new(3_003).unwrap();
    let return_edge = EdgeId::new(3_004).unwrap();
    let parameters = parameter_types
        .iter()
        .enumerate()
        .map(|(index, scalar_type)| AbstractParameter {
            value: ValueId::new(3_100 + index as u64).unwrap(),
            scalar_type: *scalar_type,
        })
        .collect::<Vec<_>>();
    let scalar_type = parameter_types[returned_parameter];
    AbstractOperationPlan {
        psi: identity(),
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry,
            parameters: parameters.clone(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: result_value,
                scalar_type,
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block: entry,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![AbstractOperation::Return {
                psi_edge: return_edge,
                result: result_value,
                value: parameters[returned_parameter].value,
                scalar_type,
                cleanup_actions: Vec::new(),
            }],
        }],
    }
}
