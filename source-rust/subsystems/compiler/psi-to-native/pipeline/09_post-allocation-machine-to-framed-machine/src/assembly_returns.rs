//! Check the final instruction alternatives and physical homes against the assembly body.
use operation_operands::values::{AssemblyRegisterMaterialization, AssemblyRegisterVersion};
use selected_instructions::{
    MachineEncodedControlEffect, MachineEncodedEffects, SelectedInstruction,
    SelectedInstructionKind,
};
use semantic_vocabulary::IntegerValue;

pub(super) fn validate(frame: &crate::PhysicalFrameProgram) -> Result<(), &'static str> {
    let program = &frame.instructions;
    for obligation in program.contracts.function_abis().iter() {
        let Some(view) = obligation.assembly_return else {
            continue;
        };
        let permit = program
            .physical()
            .model()
            .view_named(view.name())
            .ok_or("checked assembly result register is missing from the physical model")?;
        let protocol = frame
            .protocols
            .functions()
            .iter()
            .find(|row| row.machine == obligation.machine)
            .ok_or("checked assembly return has no physical frame recipe")?;
        use physical_recipes::frame_protocol::FrameRecipe;
        let bytes = match protocol.recipe() {
            FrameRecipe::X86(recipe) => recipe.frame_size_bytes(),
            FrameRecipe::Aarch64(recipe) => recipe.frame_size_bytes(),
        };
        if bytes != 0 {
            return Err("this checked assembly return cannot introduce a physical stack frame");
        }
        let function = program
            .machine
            .function(obligation.machine)
            .ok_or("missing checked assembly body in physical program")?;
        let [block] = function.blocks.as_slice() else {
            return Err(
                "checked assembly local transfers must realize one contiguous instruction sequence",
            );
        };
        let (terminal, writes) = block
            .instructions
            .split_last()
            .ok_or("checked assembly return has no physical instructions")?;
        if !matches!(
            terminal.alternative.encoded.control,
            MachineEncodedControlEffect::ReturnFromActivationStackV1
                | MachineEncodedControlEffect::ReturnIndirectRegisterV1 { .. }
        ) || !matches!(terminal.operands.as_slice(), [result] if result.read_units == permit.write_units)
        {
            return Err("physical return does not consume the checked assembly result register");
        }
        let selected = program
            .function(obligation.machine)
            .and_then(|function| function.block(block.block))
            .ok_or("missing selected definitions for checked assembly body")?;
        let final_version = obligation
            .assembly_register_versions
            .last()
            .ok_or("checked assembly return has no reaching definition")?;
        if obligation
            .assembly_register_versions
            .iter()
            .any(|version| version.view != view)
        {
            return Err("checked assembly physical body requires a single register view");
        }
        let mut selected = selected.instructions.iter();
        // Several labels can observe the same uninterrupted SSA version.
        let mut expected = obligation
            .assembly_register_versions
            .chunk_by(|left, right| {
                left.operation == right.operation
                    && left.value == right.value
                    && left.materialization == right.materialization
            })
            .map(|edges| &edges[0])
            .peekable();
        let mut holds_result = false;
        let plain_write = MachineEncodedEffects::fallthrough_v1(Vec::new(), vec![0]);
        for instruction in writes {
            // The chosen alternative must preserve flags. A materialization of zero
            // implemented by XOR is a different footprint from an immediate move.
            if instruction.alternative.encoded != plain_write
                || instruction.unit_defs != permit.write_units
                || !instruction.unit_clobbers.is_empty()
                || !instruction.unit_uses.is_empty()
            {
                return Err(
                    "physical assembly write changes its required register or flag footprint",
                );
            }
            let definition = selected
                .find(|source| source.id == instruction.instruction)
                .ok_or("physical assembly write has no ordered selected definition")?;
            let SelectedInstructionKind::MaterializeI64 { value } = definition.kind else {
                return Err("physical assembly write is not an immediate register definition");
            };
            // Both eax and w0 clear the upper half. A scalar signed -1 materialized
            // into the full return register would produce different upper bits.
            let zero_extended = match value {
                IntegerValue::Signed(number) => u32::try_from(number).is_ok(),
                IntegerValue::Unsigned(number) => u32::try_from(number).is_ok(),
            };
            if !zero_extended {
                return Err("physical assembly write does not zero-extend the 32-bit register");
            }
            holds_result = establishes(definition, value, final_version);
            let _ = expected.next_if(|row| establishes(definition, value, row));
        }
        if expected.next().is_some() || !holds_result {
            return Err(
                "allocated assembly instructions do not establish each register version in edge order through ret",
            );
        }
    }
    Ok(())
}

fn establishes(
    instruction: &SelectedInstruction,
    literal: IntegerValue,
    version: &AssemblyRegisterVersion,
) -> bool {
    version.materialization == AssemblyRegisterMaterialization::IntegerImmediate(literal)
        && instruction
            .provenance
            .operations
            .contains(&version.operation)
        && instruction.provenance.values.contains(&version.value)
}
