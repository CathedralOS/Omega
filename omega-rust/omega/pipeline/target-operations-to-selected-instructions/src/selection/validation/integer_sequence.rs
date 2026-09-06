//! Independent source-step, operand, evidence, and definition projection checks.

use super::blocks::instruction_projection;
use crate::selection::constraints::row;
use crate::selection::shared::*;
use legalized_operations::{
    LegalizedExactIntegerOperator, LegalizedExactIntegerSequence, LegalizedIntegerStep,
};

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn validate(
    function: usize,
    sequence: &LegalizedExactIntegerSequence,
    result: ValueId,
    inputs: &[(ValueId, VirtualRegisterId)],
    first_instruction: u32,
    first_register: usize,
    registers: &[VirtualRegister],
    instructions: &[SelectedInstruction],
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::FunctionProjectionMismatch { function };
    sequence
        .validate_shape(
            &inputs.iter().map(|(value, _)| *value).collect::<Vec<_>>(),
            result,
        )
        .map_err(|_| invalid())?;
    if instructions.len() != sequence.steps.len()
        || registers.len() != first_register + sequence.steps.len()
    {
        return Err(invalid());
    }
    let u64_type = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
    );
    let mut available = inputs.to_vec();
    for (position, (step, proposed)) in sequence.steps.iter().zip(instructions).enumerate() {
        let id = SelectedInstructionId(
            first_instruction
                .checked_add(u32::try_from(position).map_err(|_| invalid())?)
                .ok_or_else(invalid)?,
        );
        let destination =
            VirtualRegisterId(u32::try_from(first_register + position).map_err(|_| invalid())?);
        let (source_value, site, kind, key, operands, provenance) = match step {
            LegalizedIntegerStep::Immediate(value) => (
                value.source_value,
                value.definition_site,
                SelectedInstructionKind::MaterializeI64 { value: value.value },
                keys.materialize_i64,
                vec![destination],
                SelectedInstructionProvenance {
                    operations: vec![value.constant_operation],
                    values: vec![value.source_value],
                    fuel: value.fuel.clone(),
                    ..Default::default()
                },
            ),
            LegalizedIntegerStep::ExactBinary(value) => {
                let operand = |source| {
                    available
                        .iter()
                        .find(|(value, _)| *value == source)
                        .map(|(_, register)| *register)
                        .ok_or_else(invalid)
                };
                let (kind, key) = match value.operator {
                    LegalizedExactIntegerOperator::Add => (
                        SelectedInstructionKind::ExactAddI64 {
                            obligation: value.obligation,
                            accepted_fact: value.accepted_fact,
                        },
                        keys.add_i64,
                    ),
                    LegalizedExactIntegerOperator::Subtract => (
                        SelectedInstructionKind::ExactSubtractI64 {
                            obligation: value.obligation,
                            accepted_fact: value.accepted_fact,
                        },
                        keys.subtract_i64,
                    ),
                };
                (
                    value.source_value,
                    value.definition_site,
                    kind,
                    key,
                    vec![operand(value.left)?, operand(value.right)?, destination],
                    SelectedInstructionProvenance {
                        operations: vec![value.operation],
                        values: vec![value.left, value.right, value.source_value],
                        obligations: vec![value.obligation],
                        fuel: value.fuel.clone(),
                        ..Default::default()
                    },
                )
            }
        };
        let constraints = row(catalog, key)?;
        if constraints.operands.len() != operands.len() {
            return Err(invalid());
        }
        let definition = &registers[first_register + position];
        if definition.id != destination
            || definition.scalar_type != u64_type
            || definition.origin
                != (VirtualRegisterOrigin::InstructionResult {
                    instruction: id,
                    source_value,
                })
            || definition.definition_site != site
            || definition.entry_fixed_view.is_some()
            || definition.class != constraints.operands.last().ok_or_else(invalid)?.class
        {
            return Err(invalid());
        }
        for (operand, constraint) in operands.iter().zip(&constraints.operands) {
            let register = registers.get(operand.0 as usize).ok_or_else(invalid)?;
            if register.class != constraint.class || register.scalar_type != u64_type {
                return Err(invalid());
            }
        }
        instruction_projection::validate(
            function,
            proposed,
            id,
            kind,
            key,
            &operands,
            &provenance,
            catalog,
        )?;
        available.push((source_value, destination));
    }
    available
        .iter()
        .find(|(value, _)| *value == result)
        .map(|(_, register)| *register)
        .ok_or_else(invalid)
}
