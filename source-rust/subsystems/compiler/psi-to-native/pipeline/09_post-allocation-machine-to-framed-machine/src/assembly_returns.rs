//! Check the final instruction alternatives and physical homes against the assembly body.
use selected_instructions::{MachineEncodedControlEffect, MachineEncodedEffects};

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
        if writes.is_empty() {
            return Err("checked assembly return does not initialize its physical result register");
        }
        for instruction in writes {
            // This source body only establishes its result through register-immediate writes.
            // Check the chosen alternative as well: a zeroing XOR would alter flags.
            let plain_write = MachineEncodedEffects::fallthrough_v1(Vec::new(), vec![0]);
            if instruction.alternative.encoded != plain_write
                || instruction.unit_defs != permit.write_units
                || !instruction.unit_clobbers.is_empty()
                || !instruction.unit_uses.is_empty()
            {
                return Err(
                    "physical instruction changes the checked assembly register, flag, or control footprint",
                );
            }
        }
    }
    Ok(())
}
