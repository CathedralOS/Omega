//! Input-only joins, shared by production and replay; no placed records are built.
pub(super) mod call;
mod fixup;
pub(super) mod relocation;
use super::{StructuralFragmentPlacementInputs, TextPlacementError, TextPlacementInput};
use omega_machine_code::FunctionFragmentEmissionPlan;
use omega_target::Architecture;

pub(super) fn validate(input: TextPlacementInput<'_>) -> Result<(), TextPlacementError> {
    match input {
        TextPlacementInput::RelocationFree(fragments) => {
            if !fragments.structural_unit_functions.is_empty() {
                return Err(TextPlacementError::SourceShapeMismatch);
            }
            for function in &fragments.functions {
                relocation::prove_none(function)?;
            }
            Ok(())
        }
        TextPlacementInput::InternalCalls(fragments) => {
            if !fragments.structural_unit_functions.is_empty() {
                return Err(TextPlacementError::SourceShapeMismatch);
            }
            Ok(())
        }
        TextPlacementInput::Structural { fragments, facts } => structural(fragments, &facts),
    }
}
fn structural(
    fragments: &FunctionFragmentEmissionPlan,
    inputs: &StructuralFragmentPlacementInputs<'_>,
) -> Result<(), TextPlacementError> {
    if fragments.target.architecture != Architecture::X86_64 || !fragments.functions.is_empty() {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    let selected_plan = &inputs.program.selected;
    let machine_plan = &inputs.program.machine;
    let effects_plan = &inputs.program.effects;
    let encoding = inputs.structural_encoding;
    let layout = &inputs.program.layout;
    let exit = inputs.exit;
    let count = fragments.structural_unit_functions.len();
    if count == 0
        || selected_plan.structural_unit_functions.len() != count
        || machine_plan.structural_unit_functions.len() != count
        || effects_plan.structural_unit_functions.len() != count
        || encoding.len() != count
        || layout.structural_unit_functions.len() != count
        || exit.structural_unit_functions.len() != count
        || !selected_plan.functions.is_empty()
        || !machine_plan.functions.is_empty()
        || !effects_plan.functions.is_empty()
        || !layout.functions.is_empty()
        || !exit.functions.is_empty()
    {
        return Err(TextPlacementError::SourceShapeMismatch);
    }

    // Establish the whole input roster before per-function joins. Duplicate
    // identities must not masquerade as a missing selected/layout counterpart.
    let mut machines = std::collections::BTreeSet::new();
    let mut extent = 0_u64;
    for fragment in &fragments.structural_unit_functions {
        if fragment.byte_count != fragment.bytes.len() as u64 {
            return Err(TextPlacementError::SourceShapeMismatch);
        }
        if !machines.insert(fragment.machine) {
            return Err(TextPlacementError::DuplicateFunction(fragment.machine));
        }
        extent = extent
            .checked_add(fragment.byte_count)
            .ok_or(TextPlacementError::OffsetOverflow)?;
    }
    if !machines.contains(&fragments.entry) {
        return Err(TextPlacementError::MissingSemanticEntry(fragments.entry));
    }

    for fragment in &fragments.structural_unit_functions {
        let selected = lookup::unique_machine(
            &selected_plan.structural_unit_functions,
            fragment.machine,
            |function| function.machine,
        )?;
        let machine = lookup::unique_machine(
            &machine_plan.structural_unit_functions,
            fragment.machine,
            |function| function.machine,
        )?;
        let effects = lookup::unique_machine(
            &effects_plan.structural_unit_functions,
            fragment.machine,
            |function| function.machine,
        )?;
        let encoded =
            lookup::unique_machine(encoding, fragment.machine, |function| function.machine)?;
        let laid_out = lookup::unique_machine(
            &layout.structural_unit_functions,
            fragment.machine,
            |function| function.machine,
        )?;
        let exited = lookup::unique_machine(
            exit.structural_unit_functions.as_slice(),
            fragment.machine,
            |function| function.machine,
        )?;
        if fragment.block.block != selected.entry_block
            || fragment.block.block != machine.block
            || fragment.block.block != effects.block
            || fragment.block.block != encoded.block
            || fragment.block.block != laid_out.block
            || fragment.block.block != exited.returned.block
            || fragment.block.offset != 0
            || fragment.block.byte_count != fragment.byte_count
        {
            return Err(TextPlacementError::SourceShapeMismatch);
        }

        match (
            fragment.block.call.as_ref(),
            selected.call.as_ref(),
            machine.call.as_ref(),
            effects.call.as_ref(),
            encoded.call.as_ref(),
            laid_out.call.as_ref(),
            exited.call.as_ref(),
        ) {
            (None, None, None, None, None, None, None) => {}
            (
                Some(fragment_call),
                Some(selected_call),
                Some(machine_call),
                Some(effect_call),
                Some(encoded_call),
                Some(layout_call),
                Some(exit_call),
            ) => {
                call::validate(
                    fragment.machine,
                    fragment_call,
                    selected_call,
                    machine_call,
                    effect_call,
                    encoded_call,
                    layout_call,
                    exit_call,
                    selected_plan.target,
                    inputs.physical,
                    inputs.constraints,
                )?;
                let start = u64_to_usize(fragment_call.offset)?;
                let end = start
                    .checked_add(fragment_call.bytes.len())
                    .ok_or(TextPlacementError::OffsetOverflow)?;
                if fragment.bytes.get(start..end) != Some(fragment_call.bytes.as_slice()) {
                    return Err(TextPlacementError::SourceShapeMismatch);
                }
            }
            _ => return Err(TextPlacementError::SourceShapeMismatch),
        }
        let returned = &fragment.block.return_instruction;
        if returned.instruction != selected.terminator.instruction.id
            || returned.instruction != machine.return_instruction.instruction
            || returned.instruction != effects.return_instruction.instruction
            || returned.instruction != encoded.return_instruction.instruction
            || returned.instruction != laid_out.return_instruction.instruction
            || returned.instruction != exited.returned.instruction
            || returned.offset != laid_out.return_instruction.offset
            || returned.offset != exited.returned.offset
        {
            return Err(TextPlacementError::SourceShapeMismatch);
        }
        let returned_start = u64_to_usize(returned.offset)?;
        let returned_end = returned_start
            .checked_add(returned.bytes.len())
            .ok_or(TextPlacementError::OffsetOverflow)?;
        if fragment.bytes.get(returned_start..returned_end) != Some(returned.bytes.as_slice()) {
            return Err(TextPlacementError::SourceShapeMismatch);
        }
    }
    Ok(())
}

fn u64_to_usize(value: u64) -> Result<usize, TextPlacementError> {
    usize::try_from(value).map_err(|_| TextPlacementError::OffsetOverflow)
}
mod lookup {
    use psi_core::MachineId;

    use super::TextPlacementError;

    pub(super) fn unique_machine<T>(
        functions: &[T],
        machine: MachineId,
        identify: impl Fn(&T) -> MachineId,
    ) -> Result<&T, TextPlacementError> {
        let mut matches = functions
            .iter()
            .filter(|function| identify(function) == machine);
        let function = matches
            .next()
            .ok_or(TextPlacementError::SourceShapeMismatch)?;
        if matches.next().is_some() {
            return Err(TextPlacementError::DuplicateFunction(machine));
        }
        Ok(function)
    }
}
