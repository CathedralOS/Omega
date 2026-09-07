//! Replay selected register transport against the current scalar graph.
//! This checks the proposed stream in place; it does not call selection.

use super::integrity::{validate_block_constraints, validate_def_use};
use crate::selection::constraints::{fixed_input_constraint, row};
use crate::selection::shared::*;
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstructionKind, LegalizedScalarReturnValue,
};

pub(in crate::selection) fn validate(
    function: usize,
    source: &LegalizedScalarFunction,
    selected: &SelectedFunction,
    native_target: target::NativeTarget,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::FunctionProjectionMismatch { function };
    let ([source_block], [block]) = (source.blocks.as_slice(), selected.blocks.as_slice()) else {
        return Err(invalid());
    };
    if source.entry_block != source_block.id
        || selected.machine != source.machine
        || selected.attachment != source.attachment
        || selected.provenance != source.provenance
        || selected.entry_block != SelectedBlockId(0)
        || block.id != SelectedBlockId(0)
        || block.source_block != source_block.id
    {
        return Err(invalid());
    }
    let environment = register_environment::validate_target_register_environment(
        native_target,
        physical.model().clone(),
        catalog.catalog().clone(),
    )
    .map_err(|_| invalid())?;
    let [operand] = row(catalog, constraints.keys.materialize_i64)?
        .operands
        .as_slice()
    else {
        return Err(invalid());
    };
    let mut replay = Replay {
        function,
        selected,
        block,
        class: operand.class,
        instruction_cursor: 0,
        register_cursor: 0,
        definitions: Vec::new(),
        constraints,
    };
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
        let id = replay.check_register(
            parameter.definition_site,
            ScalarType::Integer(parameter.scalar_type),
            VirtualRegisterOrigin::EntryParameter {
                source_value: parameter.value,
                parameter_index: index,
            },
            Some(fixed.fixed_view),
        )?;
        replay.definitions.push((
            parameter.value,
            id,
            parameter.definition_site,
            ScalarType::Integer(parameter.scalar_type),
        ));
    }
    for index in 0..source.parameters.len() {
        let (value, input, site, scalar_type) = replay.definitions[index];
        let output = replay.check_copy(input, value, site, scalar_type)?;
        replay.definitions[index].1 = output;
    }
    for operation in &source_block.instructions {
        let scalar_type = ScalarType::Integer(operation.scalar_type);
        let output = match &operation.kind {
            LegalizedScalarInstructionKind::Constant(value) => {
                let register = replay.result_register(
                    operation.result,
                    operation.definition_site,
                    scalar_type,
                )?;
                replay.check_instruction(
                    SelectedInstructionKind::MaterializeI64 { value: *value },
                    constraints.keys.materialize_i64,
                    &[register],
                    &SelectedInstructionProvenance {
                        operations: vec![operation.operation],
                        values: vec![operation.result],
                        fuel: operation.fuel.clone(),
                        ..Default::default()
                    },
                )?;
                register
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
                        replay.resolve(argument.source).ok_or_else(invalid)?;
                    operands.push(replay.check_copy(
                        input,
                        argument.source,
                        site,
                        argument_type,
                    )?);
                }
                let short_result = replay.result_register(
                    operation.result,
                    operation.definition_site,
                    scalar_type,
                )?;
                operands.push(short_result);
                replay.check_instruction(
                    SelectedInstructionKind::CallI64 {
                        callee: call.callee,
                    },
                    key,
                    &operands,
                    &SelectedInstructionProvenance {
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
                replay.check_copy(
                    short_result,
                    operation.result,
                    operation.definition_site,
                    scalar_type,
                )?
            }
        };
        replay.definitions.push((
            operation.result,
            output,
            operation.definition_site,
            scalar_type,
        ));
    }
    let returned = &source_block.terminator;
    let (kind, key, operands, values) = match returned.value {
        LegalizedScalarReturnValue::Unit => (
            SelectedInstructionKind::ReturnUnit,
            constraints.keys.return_unit,
            Vec::new(),
            Vec::new(),
        ),
        LegalizedScalarReturnValue::Value { value, scalar_type } => {
            let (_, input, site, value_type) = replay.resolve(value).ok_or_else(invalid)?;
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
            let output = replay.check_copy(input, value, site, value_type)?;
            (
                SelectedInstructionKind::ReturnI64,
                constraints.keys.return_i64,
                vec![output],
                vec![value],
            )
        }
    };
    if replay.instruction_cursor != block.instructions.len()
        || replay.register_cursor != selected.virtual_registers.len()
    {
        return Err(invalid());
    }
    let SelectedTerminator::Return {
        instruction,
        psi_return_edge,
    } = &block.terminator
    else {
        return Err(invalid());
    };
    if *psi_return_edge != returned.edge
        || instruction.id.0 as usize != replay.instruction_cursor
        || instruction.kind != kind
        || instruction.constraint != key
        || instruction
            .operands
            .iter()
            .map(|operand| operand.virtual_register)
            .ne(operands)
        || instruction.provenance
            != (SelectedInstructionProvenance {
                values,
                edges: vec![returned.edge],
                fuel: returned.fuel.clone(),
                ..Default::default()
            })
    {
        return Err(invalid());
    }
    validate_block_constraints(function, block, selected, catalog)?;
    validate_def_use(function, selected, catalog)
}

struct Replay<'a> {
    function: usize,
    selected: &'a SelectedFunction,
    block: &'a SelectedBlock,
    class: RegisterClassId,
    instruction_cursor: usize,
    register_cursor: usize,
    definitions: Vec<(ValueId, VirtualRegisterId, ValueDefinitionSite, ScalarType)>,
    constraints: &'a SelectedSelectionConstraints,
}

impl Replay<'_> {
    fn invalid(&self) -> SelectedInstructionError {
        SelectedInstructionError::FunctionProjectionMismatch {
            function: self.function,
        }
    }

    fn resolve(
        &self,
        value: ValueId,
    ) -> Option<(ValueId, VirtualRegisterId, ValueDefinitionSite, ScalarType)> {
        self.definitions
            .iter()
            .find(|(source, ..)| *source == value)
            .copied()
    }

    fn check_register(
        &mut self,
        site: ValueDefinitionSite,
        scalar_type: ScalarType,
        origin: VirtualRegisterOrigin,
        fixed: Option<RegisterViewId>,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let register = self
            .selected
            .virtual_registers
            .get(self.register_cursor)
            .ok_or_else(|| self.invalid())?;
        if register.id.0 as usize != self.register_cursor
            || register.scalar_type != scalar_type
            || register.class != self.class
            || register.origin != origin
            || register.definition_site != site
            || register.entry_fixed_view != fixed
        {
            return Err(self.invalid());
        }
        self.register_cursor += 1;
        Ok(register.id)
    }

    fn result_register(
        &mut self,
        value: ValueId,
        site: ValueDefinitionSite,
        scalar_type: ScalarType,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let instruction = SelectedInstructionId(
            self.instruction_cursor
                .try_into()
                .map_err(|_| self.invalid())?,
        );
        self.check_register(
            site,
            scalar_type,
            VirtualRegisterOrigin::InstructionResult {
                instruction,
                source_value: value,
            },
            None,
        )
    }

    fn check_instruction(
        &mut self,
        kind: SelectedInstructionKind,
        key: RegisterConstraintKey,
        operands: &[VirtualRegisterId],
        provenance: &SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        let instruction = self
            .block
            .instructions
            .get(self.instruction_cursor)
            .ok_or_else(|| self.invalid())?;
        if instruction.id.0 as usize != self.instruction_cursor
            || instruction.kind != kind
            || instruction.constraint != key
            || instruction.provenance != *provenance
            || instruction
                .operands
                .iter()
                .map(|operand| operand.virtual_register)
                .ne(operands.iter().copied())
        {
            return Err(self.invalid());
        }
        self.instruction_cursor += 1;
        Ok(())
    }

    fn check_copy(
        &mut self,
        input: VirtualRegisterId,
        value: ValueId,
        site: ValueDefinitionSite,
        scalar_type: ScalarType,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let output = self.result_register(value, site, scalar_type)?;
        self.check_instruction(
            SelectedInstructionKind::CopyI64,
            self.constraints.keys.copy_i64,
            &[input, output],
            &SelectedInstructionProvenance {
                values: vec![value],
                ..Default::default()
            },
        )?;
        Ok(output)
    }
}
