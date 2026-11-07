//! Check that descriptor-table instructions use the original tables, selectors
//! and result values. Final byte and register-form checks run before this audit.
use abstract_operations::{AbstractOperation, AbstractResult};
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionKind, SelectedMemoryAccess,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerValue, OperationId, ServiceId, ValueId};
use target_operations::{TargetFunction, TargetUnitOperation};
use terminal_psi::{RegisterOperation, StructuralArgument};

#[derive(Debug, PartialEq, Eq)]
struct DescriptorOperation<'a> {
    operation: OperationId,
    service: ServiceId,
    hardware: RegisterOperation,
    result: Option<AbstractResult>,
    entries: Option<u16>,
    selector: Option<ValueId>,
    sources: Vec<&'a StructuralArgument>,
}

fn source_operation(row: &AbstractOperation) -> Option<DescriptorOperation<'_>> {
    let (operation, service, hardware, result, entries, selector, sources) = match row {
        AbstractOperation::ReadSegmentSelector {
            psi_operation,
            service,
            register,
            result,
        } => (
            *psi_operation,
            *service,
            RegisterOperation::SegmentRead(*register),
            Some(*result),
            None,
            None,
            vec![],
        ),
        AbstractOperation::ReadTaskRegister {
            psi_operation,
            service,
            result,
        } => (
            *psi_operation,
            *service,
            RegisterOperation::TaskRegisterRead,
            Some(*result),
            None,
            None,
            vec![],
        ),
        AbstractOperation::ReloadSegmentDescriptors {
            psi_operation,
            service,
            result,
            entries,
            kind,
            gdt,
            selector,
            scratch,
        } => (
            *psi_operation,
            *service,
            RegisterOperation::SegmentReload(*kind),
            Some(*result),
            Some(*entries),
            Some(*selector),
            vec![gdt, scratch],
        ),
        AbstractOperation::ActivateTaskRegister {
            psi_operation,
            service,
            result,
            entries,
            gdt,
            task_state,
            selector,
            scratch,
        } => (
            *psi_operation,
            *service,
            RegisterOperation::TaskRegisterActivate,
            Some(*result),
            Some(*entries),
            Some(*selector),
            vec![gdt, task_state, scratch],
        ),
        AbstractOperation::GlobalDescriptorTable {
            psi_operation,
            service,
            operation,
            source,
        } => (
            *psi_operation,
            *service,
            RegisterOperation::GlobalDescriptorTable(*operation),
            None,
            None,
            None,
            vec![source],
        ),
        _ => return None,
    };
    Some(DescriptorOperation {
        operation,
        service,
        hardware,
        result,
        entries,
        selector,
        sources,
    })
}

fn target_operation(
    row: &TargetUnitOperation,
) -> Result<Option<DescriptorOperation<'_>>, &'static str> {
    let (operation, service, hardware, home, entries, selector, sources) = match row {
        TargetUnitOperation::ReadSegmentSelector {
            psi_operation,
            service,
            register,
            result_home,
        } => (
            *psi_operation,
            *service,
            RegisterOperation::SegmentRead(*register),
            Some(result_home),
            None,
            None,
            vec![],
        ),
        TargetUnitOperation::ReadTaskRegister {
            psi_operation,
            service,
            result_home,
        } => (
            *psi_operation,
            *service,
            RegisterOperation::TaskRegisterRead,
            Some(result_home),
            None,
            None,
            vec![],
        ),
        TargetUnitOperation::ReloadSegmentDescriptors {
            psi_operation,
            service,
            result_home,
            entries,
            kind,
            gdt,
            selector,
            scratch,
        } => (
            *psi_operation,
            *service,
            RegisterOperation::SegmentReload(*kind),
            Some(result_home),
            Some(*entries),
            Some(selector.source_value()),
            vec![gdt, scratch],
        ),
        TargetUnitOperation::ActivateTaskRegister {
            psi_operation,
            service,
            result_home,
            entries,
            gdt,
            task_state,
            selector,
            scratch,
        } => (
            *psi_operation,
            *service,
            RegisterOperation::TaskRegisterActivate,
            Some(result_home),
            Some(*entries),
            Some(selector.source_value()),
            vec![gdt, task_state, scratch],
        ),
        TargetUnitOperation::GlobalDescriptorTable {
            psi_operation,
            service,
            operation,
            source,
        } => (
            *psi_operation,
            *service,
            RegisterOperation::GlobalDescriptorTable(*operation),
            None,
            None,
            None,
            vec![source],
        ),
        _ => return Ok(None),
    };
    if home.is_some_and(|home| home.defining_operation != operation) {
        return Err("descriptor result home names a different defining operation");
    }
    Ok(Some(DescriptorOperation {
        operation,
        service,
        hardware,
        result: home.map(|home| AbstractResult {
            value: home.source_value,
            scalar_type: home.scalar_type,
        }),
        entries,
        selector,
        sources,
    }))
}

/// Operation IDs are opaque. Sort these sparse associations once per function;
/// keep duplicate rejection instead of selecting the first matching row.
pub(super) struct DescriptorReplay<'a> {
    operations: Vec<DescriptorOperation<'a>>,
    hardware: Vec<(OperationId, &'a SelectedInstruction)>,
    selected: &'a SelectedFunction,
    target: &'a TargetFunction,
    definitions: Vec<Definition<'a>>,
    accesses: Vec<&'a SelectedMemoryAccess>,
}

#[derive(Clone, Copy)]
enum Definition<'a> {
    Absent,
    Unique(&'a SelectedInstruction),
    Multiple,
}

impl<'a> DescriptorReplay<'a> {
    pub(super) fn new(
        target: &'a TargetFunction,
        selected: &'a SelectedFunction,
    ) -> Result<Self, &'static str> {
        let mut operations = Vec::new();
        let mut installations = Vec::new();
        for row in target.graph.operations() {
            if let TargetUnitOperation::InstallStaticInterrupt { psi_operation, .. } = row {
                installations.push(*psi_operation);
            }
            if let Some(operation) = target_operation(row)? {
                operations.push(operation);
            }
        }
        operations.sort_unstable_by_key(|row| row.operation);
        if operations
            .windows(2)
            .any(|pair| pair[0].operation == pair[1].operation)
        {
            return Err("descriptor operation has duplicate target rows");
        }
        installations.sort_unstable();
        if installations.windows(2).any(|pair| pair[0] == pair[1])
            || operations
                .iter()
                .any(|row| installations.binary_search(&row.operation).is_ok())
        {
            return Err("descriptor hardware owner has duplicate target operations");
        }
        let mut hardware = Vec::new();
        for row in selected.instructions() {
            if matches!(
                row.kind,
                SelectedInstructionKind::RegisterAccess {
                    operation: RegisterOperation::SegmentRead(_)
                        | RegisterOperation::TaskRegisterRead
                        | RegisterOperation::SegmentReload(_)
                        | RegisterOperation::TaskRegisterActivate
                        | RegisterOperation::GlobalDescriptorTable(_)
                }
            ) {
                let [operation] = row.provenance.operations.as_slice() else {
                    return Err("descriptor instruction has no exact source operation");
                };
                let dedicated = operations
                    .binary_search_by_key(operation, |row| row.operation)
                    .is_ok();
                if !dedicated_descriptor_owner(dedicated, &installations, *operation)? {
                    continue;
                }
                hardware.push((*operation, row));
            }
        }
        hardware.sort_unstable_by_key(|row| row.0);
        if hardware.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err("descriptor operation has duplicate hardware instructions");
        }
        if operations.is_empty() {
            return Ok(Self {
                operations,
                hardware,
                selected,
                target,
                definitions: Vec::new(),
                accesses: Vec::new(),
            });
        }
        let mut definitions = vec![Definition::Absent; selected.virtual_registers.len()];
        for row in selected.blocks.iter().flat_map(|block| {
            block.instructions.iter().chain(std::iter::once(
                selected_instructions::terminator_instruction(&block.terminator),
            ))
        }) {
            for operand in &row.operands {
                if matches!(
                    operand.access,
                    RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                ) {
                    let definition = definitions
                        .get_mut(operand.virtual_register.0 as usize)
                        .ok_or("descriptor replay found a definition of an absent register")?;
                    *definition = match definition {
                        Definition::Absent => Definition::Unique(row),
                        _ => Definition::Multiple,
                    };
                }
            }
        }
        let mut accesses: Vec<_> = selected.memory_accesses.iter().collect();
        accesses.sort_unstable_by_key(|access| access.instruction);
        Ok(Self {
            operations,
            hardware,
            selected,
            target,
            definitions,
            accesses,
        })
    }

    pub(super) fn validate(&self, source: &AbstractOperation) -> Result<(), String> {
        let expected = source_operation(source).ok_or("operation is not a descriptor operation")?;
        let index = self
            .operations
            .binary_search_by_key(&expected.operation, |row| row.operation)
            .map_err(|_| "descriptor operation has no retained target row")?;
        if self.operations[index] != expected {
            return Err(
                "descriptor target differs from the source operands, service or result".into(),
            );
        }
        let index = self
            .hardware
            .binary_search_by_key(&expected.operation, |row| row.0)
            .map_err(|_| "descriptor operation has no selected hardware instruction")?;
        let instruction = self.hardware[index].1;
        if instruction.kind
            != (SelectedInstructionKind::RegisterAccess {
                operation: expected.hardware,
            })
        {
            return Err("descriptor hardware instruction differs from the source operation".into());
        }
        self.validate_operands(&expected, instruction)
    }

    fn validate_operands(
        &self,
        expected: &DescriptorOperation<'_>,
        instruction: &SelectedInstruction,
    ) -> Result<(), String> {
        let (source_operands, selector_operand, extent_operand, result_operand, roles): (
            &[u16],
            Option<u16>,
            Option<u16>,
            Option<u16>,
            &[&[SelectedMemoryAccessRole]],
        ) = match expected.hardware {
            RegisterOperation::SegmentRead(_) | RegisterOperation::TaskRegisterRead => {
                (&[], None, None, Some(0), &[])
            }
            RegisterOperation::SegmentReload(_) => (
                &[0, 2],
                Some(1),
                Some(3),
                Some(4),
                &[
                    &[
                        SelectedMemoryAccessRole::ReadPlace,
                        SelectedMemoryAccessRole::WritePlace,
                    ],
                    &[SelectedMemoryAccessRole::WritePlace],
                ],
            ),
            RegisterOperation::TaskRegisterActivate => (
                &[0, 1, 3],
                Some(2),
                Some(4),
                Some(5),
                &[
                    &[
                        SelectedMemoryAccessRole::ReadPlace,
                        SelectedMemoryAccessRole::WritePlace,
                    ],
                    &[SelectedMemoryAccessRole::ReadPlace],
                    &[SelectedMemoryAccessRole::WritePlace],
                ],
            ),
            RegisterOperation::GlobalDescriptorTable(
                terminal_psi::GlobalDescriptorTableOperation::Load,
            ) => (
                &[0],
                None,
                None,
                None,
                &[&[SelectedMemoryAccessRole::ReadPlace]],
            ),
            RegisterOperation::GlobalDescriptorTable(
                terminal_psi::GlobalDescriptorTableOperation::Store,
            ) => (
                &[0],
                None,
                None,
                None,
                &[&[SelectedMemoryAccessRole::WritePlace]],
            ),
            _ => return Err("unexpected descriptor hardware operation".into()),
        };
        let count = source_operands.len()
            + usize::from(selector_operand.is_some())
            + usize::from(extent_operand.is_some())
            + usize::from(result_operand.is_some());
        if instruction.operands.len() != count {
            return Err("descriptor instruction has an unexpected operand count".into());
        }
        let mut memory = Vec::new();
        for (index, (&position, argument)) in
            source_operands.iter().zip(&expected.sources).enumerate()
        {
            let pointer = operand(instruction, position, RegisterOperandAccess::Use)?;
            let offset = super::placed_reference::borrowed_address_matches(
                expected.operation,
                argument,
                self.target,
                self.selected,
                instruction,
                pointer,
            )?;
            let byte_count = if let Some(entries) = expected.entries.filter(|_| index == 0) {
                u32::from(entries) * 8
            } else if expected.hardware == RegisterOperation::TaskRegisterActivate && index == 1 {
                104
            } else {
                10
            };
            for role in roles[index] {
                memory.push(SelectedMemoryAccess {
                    instruction: instruction.id,
                    origin: SelectedMemoryAccessOrigin::Operation(expected.operation),
                    place: argument.place,
                    byte_offset: offset,
                    byte_count,
                    role: *role,
                });
            }
        }
        let start = self
            .accesses
            .partition_point(|row| row.instruction < instruction.id);
        let end = self
            .accesses
            .partition_point(|row| row.instruction <= instruction.id);
        validate_memory(&self.accesses[start..end], &memory)?;
        if let Some(position) = selector_operand {
            let register = operand(instruction, position, RegisterOperandAccess::Use)?;
            if super::placed_reference::source_value(self.selected, register, instruction)
                != expected.selector
            {
                return Err("descriptor selector register names a different source value".into());
            }
        }
        if let Some(position) = extent_operand {
            let register = operand(instruction, position, RegisterOperandAccess::Use)?;
            let definition = self.definition(register)?;
            let extent = u128::from(
                expected
                    .entries
                    .ok_or("descriptor table extent is absent")?,
            ) * 8;
            if definition.kind
                != (SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(extent),
                })
                || definition.provenance.operations.as_slice() != [expected.operation]
                || !super::placed_reference::precedes(self.selected, definition.id, instruction.id)
            {
                return Err(
                    "descriptor extent does not equal the original table byte count".into(),
                );
            }
        }
        if let Some(position) = result_operand {
            let result = expected
                .result
                .ok_or("descriptor source result is absent")?;
            let register = operand(instruction, position, RegisterOperandAccess::Def)?;
            let retained = self
                .selected
                .virtual_register(register)
                .ok_or("descriptor result register is absent")?;
            if self.definition(register)?.id != instruction.id {
                return Err("descriptor result has a different or repeated definition".into());
            }
            validate_result(instruction, retained, result)?;
        }
        Ok(())
    }

    fn definition(
        &self,
        register: VirtualRegisterId,
    ) -> Result<&'a SelectedInstruction, &'static str> {
        match self.definitions.get(register.0 as usize) {
            Some(Definition::Unique(instruction)) => Ok(instruction),
            _ => Err("descriptor operand has no unique register definition"),
        }
    }
}

/// Scope's descriptor instructions belong to its complete installation replay.
/// Every other descriptor instruction needs its own dedicated source operation.
fn dedicated_descriptor_owner(
    dedicated: bool,
    installations: &[OperationId],
    operation: OperationId,
) -> Result<bool, &'static str> {
    if dedicated {
        Ok(true)
    } else if installations.binary_search(&operation).is_ok() {
        Ok(false)
    } else {
        Err("descriptor hardware instruction has no descriptor or installation source operation")
    }
}

fn validate_memory(
    actual: &[&SelectedMemoryAccess],
    expected: &[SelectedMemoryAccess],
) -> Result<(), &'static str> {
    if actual.len() != expected.len()
        || expected
            .iter()
            .any(|wanted| actual.iter().filter(|actual| ***actual == *wanted).count() != 1)
    {
        return Err(
            "descriptor hardware memory footprint differs from its table, task or scratch operands",
        );
    }
    Ok(())
}

fn validate_result(
    instruction: &SelectedInstruction,
    retained: &selected_instructions::VirtualRegister,
    result: AbstractResult,
) -> Result<(), &'static str> {
    if retained.origin
        != (VirtualRegisterOrigin::InstructionResult {
            instruction: instruction.id,
            source_value: result.value,
        })
        || retained.scalar_type != result.scalar_type
    {
        return Err(
            "descriptor output register does not retain the original result value and type",
        );
    }
    Ok(())
}

fn operand(
    instruction: &SelectedInstruction,
    position: u16,
    access: RegisterOperandAccess,
) -> Result<VirtualRegisterId, &'static str> {
    let mut rows = instruction
        .operands
        .iter()
        .filter(|row| row.operand == position);
    match (rows.next(), rows.next()) {
        (Some(row), None) if row.access == access => Ok(row.virtual_register),
        _ => Err("descriptor instruction operand is missing, duplicated or has wrong access"),
    }
}
