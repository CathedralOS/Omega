//! Ordered scalar definitions; source operands name existing virtual values.

use crate::selection::constraints::{instruction, row};
use crate::selection::shared::*;
use legalized_operations::{
    LegalizedExactIntegerOperator, LegalizedExactIntegerSequence, LegalizedIntegerStep,
};

#[cfg(test)]
mod tests;

#[allow(clippy::too_many_arguments)]
pub(super) fn build(
    function: usize,
    sequence: &LegalizedExactIntegerSequence,
    result: ValueId,
    inputs: &[(ValueId, VirtualRegisterId)],
    first_instruction: u32,
    registers: &mut Vec<VirtualRegister>,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(Vec<SelectedInstruction>, VirtualRegisterId), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::UnsupportedSourceShape { function };
    sequence
        .validate_shape(
            &inputs.iter().map(|(value, _)| *value).collect::<Vec<_>>(),
            result,
        )
        .map_err(|_| invalid())?;
    let scalar_type = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
    );
    let mut values = inputs.to_vec();
    let mut instructions = Vec::new();
    for (position, step) in sequence.steps.iter().enumerate() {
        let id = SelectedInstructionId(
            first_instruction
                .checked_add(u32::try_from(position).map_err(|_| invalid())?)
                .ok_or_else(invalid)?,
        );
        let destination = VirtualRegisterId(u32::try_from(registers.len()).map_err(|_| invalid())?);
        let (source_value, definition_site, kind, key, operands, provenance) = match step {
            LegalizedIntegerStep::Immediate(immediate) => (
                immediate.source_value,
                immediate.definition_site,
                SelectedInstructionKind::MaterializeI64 {
                    value: immediate.value,
                },
                keys.materialize_i64,
                vec![destination],
                SelectedInstructionProvenance {
                    operations: vec![immediate.constant_operation],
                    values: vec![immediate.source_value],
                    fuel: immediate.fuel.clone(),
                    ..Default::default()
                },
            ),
            LegalizedIntegerStep::ExactBinary(binary) => {
                let lookup = |value| {
                    values
                        .iter()
                        .find(|(source, _)| *source == value)
                        .map(|(_, register)| *register)
                        .ok_or_else(invalid)
                };
                let (kind, key) = match binary.operator {
                    LegalizedExactIntegerOperator::Add => (
                        SelectedInstructionKind::ExactAddI64 {
                            obligation: binary.obligation,
                            accepted_fact: binary.accepted_fact,
                        },
                        keys.add_i64,
                    ),
                    LegalizedExactIntegerOperator::Subtract => (
                        SelectedInstructionKind::ExactSubtractI64 {
                            obligation: binary.obligation,
                            accepted_fact: binary.accepted_fact,
                        },
                        keys.subtract_i64,
                    ),
                };
                (
                    binary.source_value,
                    binary.definition_site,
                    kind,
                    key,
                    vec![lookup(binary.left)?, lookup(binary.right)?, destination],
                    SelectedInstructionProvenance {
                        operations: vec![binary.operation],
                        values: vec![binary.left, binary.right, binary.source_value],
                        obligations: vec![binary.obligation],
                        fuel: binary.fuel.clone(),
                        ..Default::default()
                    },
                )
            }
        };
        let constraints = row(catalog, key)?;
        let output = constraints.operands.last().ok_or_else(invalid)?;
        registers.push(VirtualRegister {
            id: destination,
            scalar_type,
            class: output.class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: id,
                source_value,
            },
            definition_site,
            entry_fixed_view: None,
        });
        instructions.push(instruction(id, kind, key, &operands, provenance, catalog)?);
        values.push((source_value, destination));
    }
    let result = values
        .iter()
        .find(|(source, _)| *source == result)
        .map(|(_, register)| *register)
        .ok_or_else(invalid)?;
    Ok((instructions, result))
}
