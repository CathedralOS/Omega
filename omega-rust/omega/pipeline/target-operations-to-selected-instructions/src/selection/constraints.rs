use super::shared::*;

pub(super) fn instruction(
    id: SelectedInstructionId,
    kind: SelectedInstructionKind,
    key: RegisterConstraintKey,
    registers: &[VirtualRegisterId],
    provenance: SelectedInstructionProvenance,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedInstruction, SelectedInstructionError> {
    let row = row(catalog, key)?;
    if row.operands.len() != registers.len() {
        return Err(SelectedInstructionError::MissingConstraint(key));
    }
    Ok(SelectedInstruction {
        id,
        kind,
        constraint: key,
        operands: row
            .operands
            .iter()
            .zip(registers)
            .map(|(constraint, register)| SelectedOperand {
                operand: constraint.operand,
                virtual_register: *register,
                access: constraint.access,
                class: constraint.class,
                fixed_view: constraint.fixed_view,
                tied_to: constraint.tied_to,
                early_clobber: constraint.early_clobber,
            })
            .collect(),
        implicit_uses: row.implicit_uses.clone(),
        implicit_defs: row.implicit_defs.clone(),
        clobbers: row.clobbers.clone(),
        provenance,
    })
}

pub(super) fn require_key_rows(
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    for key in [
        keys.materialize_i64,
        keys.copy_i64,
        keys.add_i64,
        keys.add_i64_immediate,
        keys.compare_i64_zero,
        keys.compare_i64,
        keys.conditional_branch,
        keys.jump,
        keys.return_i64,
        keys.return_unit,
    ] {
        row(catalog, key)?;
    }
    Ok(())
}

pub(super) fn row(
    catalog: &ValidatedRegisterConstraintCatalog,
    key: RegisterConstraintKey,
) -> Result<&RegisterInstructionConstraint, SelectedInstructionError> {
    catalog
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == key)
        .ok_or(SelectedInstructionError::MissingConstraint(key))
}

pub(super) fn fixed_input_constraint(
    machine: semantic_vocabulary::MachineId,
    source_value: semantic_vocabulary::ValueId,
    parameter_index: usize,
    register: target_operations::MachineRegister,
    inputs: &[SelectedFixedInputConstraint],
) -> Option<&SelectedFixedInputConstraint> {
    let mut matches = inputs.iter().filter(|input| {
        input.machine == machine
            && input.source_value == source_value
            && input.parameter_index == parameter_index
            && input.register == register
    });
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}
