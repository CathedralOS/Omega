//! Scalar instructions share one selection path regardless of the caller's result.

use crate::selection::constraints::{fixed_input_constraint, instruction, row};
use crate::selection::shared::*;
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstructionKind, LegalizedScalarReturnValue,
};

#[cfg(test)]
mod tests;

pub(super) fn build(
    function: usize,
    source: &LegalizedScalarFunction,
    native_target: target::NativeTarget,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedFunction, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::UnsupportedSourceShape { function };
    let [block] = source.blocks.as_slice() else {
        return Err(invalid());
    };
    if source.entry_block != block.id {
        return Err(invalid());
    }
    let environment = register_environment::validate_target_register_environment(
        native_target,
        physical.model().clone(),
        catalog.catalog().clone(),
    )
    .map_err(|_| invalid())?;
    let materialize = row(catalog, constraints.keys.materialize_i64)?;
    let [operand] = materialize.operands.as_slice() else {
        return Err(invalid());
    };
    let class = operand.class;
    let mut builder = Builder {
        class,
        constraints,
        catalog,
        registers: Vec::new(),
        instructions: Vec::new(),
        definitions: Vec::new(),
    };
    // Entry ABI precoloring ends at a copy. The semantic parameter may remain
    // live across calls without being pinned to a caller-clobbered register.
    for (index, parameter) in source.parameters.iter().enumerate() {
        let [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size: 8,
            },
        ] = parameter.placement.locations.as_slice()
        else {
            return Err(invalid());
        };
        let fixed = fixed_input_constraint(
            source.machine,
            parameter.value,
            index,
            *register,
            &constraints.fixed_inputs,
        )
        .ok_or_else(invalid)?;
        if environment.fixed_register_view(*register) != Some(fixed.fixed_view) {
            return Err(invalid());
        }
        let id = VirtualRegisterId(builder.registers.len().try_into().map_err(|_| invalid())?);
        builder.registers.push(VirtualRegister {
            id,
            scalar_type: ScalarType::Integer(parameter.scalar_type),
            class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: parameter.value,
                parameter_index: index,
            },
            definition_site: parameter.definition_site,
            entry_fixed_view: Some(fixed.fixed_view),
        });
        builder.definitions.push((
            parameter.value,
            id,
            parameter.definition_site,
            ScalarType::Integer(parameter.scalar_type),
        ));
    }
    for index in 0..source.parameters.len() {
        let (value, input, site, scalar_type) = builder.definitions[index];
        let output = builder.copy(input, value, site, scalar_type)?;
        builder.definitions[index].1 = output;
    }
    for operation in &block.instructions {
        let scalar_type = ScalarType::Integer(operation.scalar_type);
        let output = match &operation.kind {
            LegalizedScalarInstructionKind::Constant(value) => {
                let output =
                    builder.register(operation.result, operation.definition_site, scalar_type)?;
                builder.emit(
                    SelectedInstructionKind::MaterializeI64 { value: *value },
                    constraints.keys.materialize_i64,
                    &[output],
                    SelectedInstructionProvenance {
                        operations: vec![operation.operation],
                        values: vec![operation.result],
                        fuel: operation.fuel.clone(),
                        ..Default::default()
                    },
                )?;
                output
            }
            LegalizedScalarInstructionKind::Call(call) => {
                let key = constraints
                    .keys
                    .call_i64
                    .get(call.arguments.len())
                    .copied()
                    .ok_or_else(invalid)?;
                crate::selection::scalar_call_abi::validate(
                    function,
                    call,
                    key,
                    row(catalog, key)?,
                    &environment,
                )?;
                let mut operands = Vec::new();
                for argument in &call.arguments {
                    let (_, input, site, argument_type) =
                        builder.resolve(argument.source).ok_or_else(invalid)?;
                    operands.push(builder.copy(input, argument.source, site, argument_type)?);
                }
                let short_result =
                    builder.register(operation.result, operation.definition_site, scalar_type)?;
                operands.push(short_result);
                builder.emit(
                    SelectedInstructionKind::CallI64 {
                        callee: call.callee,
                    },
                    key,
                    &operands,
                    SelectedInstructionProvenance {
                        operations: vec![operation.operation],
                        values: call
                            .arguments
                            .iter()
                            .map(|argument| argument.source)
                            .chain(std::iter::once(operation.result))
                            .collect(),
                        obligations: call.requirement_obligations.clone(),
                        fuel: operation.fuel.clone(),
                        ..Default::default()
                    },
                )?;
                builder.copy(
                    short_result,
                    operation.result,
                    operation.definition_site,
                    scalar_type,
                )?
            }
        };
        builder.definitions.push((
            operation.result,
            output,
            operation.definition_site,
            scalar_type,
        ));
    }
    let returned = &block.terminator;
    let (kind, key, operands, values) = match returned.value {
        LegalizedScalarReturnValue::Unit => (
            SelectedInstructionKind::ReturnUnit,
            constraints.keys.return_unit,
            Vec::new(),
            Vec::new(),
        ),
        LegalizedScalarReturnValue::Value { value, scalar_type } => {
            let (_, input, site, value_type) = builder.resolve(value).ok_or_else(invalid)?;
            if value_type != ScalarType::Integer(scalar_type) {
                return Err(invalid());
            }
            let result = source.call_plan.result.as_ref().ok_or_else(invalid)?;
            let [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ] = result.locations.as_slice()
            else {
                return Err(invalid());
            };
            let [operand] = row(catalog, constraints.keys.return_i64)?
                .operands
                .as_slice()
            else {
                return Err(invalid());
            };
            if operand.fixed_view.is_none()
                || operand.fixed_view != environment.fixed_register_view(*register)
            {
                return Err(invalid());
            }
            let output = builder.copy(input, value, site, value_type)?;
            (
                SelectedInstructionKind::ReturnI64,
                constraints.keys.return_i64,
                vec![output],
                vec![value],
            )
        }
    };
    let terminator = SelectedTerminator::Return {
        instruction: instruction(
            SelectedInstructionId(
                builder
                    .instructions
                    .len()
                    .try_into()
                    .map_err(|_| invalid())?,
            ),
            kind,
            key,
            &operands,
            SelectedInstructionProvenance {
                values,
                edges: vec![returned.edge],
                fuel: returned.fuel.clone(),
                ..Default::default()
            },
            catalog,
        )?,
        psi_return_edge: returned.edge,
    };
    Ok(SelectedFunction {
        machine: source.machine,
        attachment: source.attachment,
        provenance: source.provenance.clone(),
        entry_block: SelectedBlockId(0),
        virtual_registers: builder.registers,
        blocks: vec![SelectedBlock {
            id: SelectedBlockId(0),
            source_block: block.id,
            instructions: builder.instructions,
            terminator,
        }],
    })
}

struct Builder<'a> {
    class: RegisterClassId,
    constraints: &'a SelectedSelectionConstraints,
    catalog: &'a ValidatedRegisterConstraintCatalog,
    registers: Vec<VirtualRegister>,
    instructions: Vec<SelectedInstruction>,
    definitions: Vec<(ValueId, VirtualRegisterId, ValueDefinitionSite, ScalarType)>,
}

impl Builder<'_> {
    fn resolve(
        &self,
        value: ValueId,
    ) -> Option<(ValueId, VirtualRegisterId, ValueDefinitionSite, ScalarType)> {
        self.definitions
            .iter()
            .find(|(source, ..)| *source == value)
            .copied()
    }

    fn register(
        &mut self,
        value: ValueId,
        site: ValueDefinitionSite,
        scalar_type: ScalarType,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let id = VirtualRegisterId(
            self.registers
                .len()
                .try_into()
                .map_err(|_| SelectedInstructionError::SourceCustodyMismatch)?,
        );
        self.registers.push(VirtualRegister {
            id,
            scalar_type,
            class: self.class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(
                    self.instructions
                        .len()
                        .try_into()
                        .map_err(|_| SelectedInstructionError::SourceCustodyMismatch)?,
                ),
                source_value: value,
            },
            definition_site: site,
            entry_fixed_view: None,
        });
        Ok(id)
    }

    fn emit(
        &mut self,
        kind: SelectedInstructionKind,
        key: RegisterConstraintKey,
        operands: &[VirtualRegisterId],
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        let id = SelectedInstructionId(
            self.instructions
                .len()
                .try_into()
                .map_err(|_| SelectedInstructionError::SourceCustodyMismatch)?,
        );
        self.instructions.push(instruction(
            id,
            kind,
            key,
            operands,
            provenance,
            self.catalog,
        )?);
        Ok(())
    }

    fn copy(
        &mut self,
        input: VirtualRegisterId,
        value: ValueId,
        site: ValueDefinitionSite,
        scalar_type: ScalarType,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let output = self.register(value, site, scalar_type)?;
        self.emit(
            SelectedInstructionKind::CopyI64,
            self.constraints.keys.copy_i64,
            &[input, output],
            SelectedInstructionProvenance {
                values: vec![value],
                ..Default::default()
            },
        )?;
        Ok(output)
    }
}
