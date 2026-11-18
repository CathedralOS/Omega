//! Capture source coordinates and carrier types before encoding drops physical
//! instruction columns. These are provenance records, not executable code.
use machine_code::image_origins::{ImageAddressOrigin, ImageAddressOrigins};
use post_allocation_machine::PhysicalFunction;
use register_model::RegisterOperandAccess;
use selected_instructions::SelectedInstructionKind;

pub fn capture<'input>(
    functions: impl IntoIterator<Item = &'input PhysicalFunction>,
) -> Result<ImageAddressOrigins, &'static str> {
    let mut rows = Vec::new();
    for function in functions {
        for block in &function.blocks {
            for instruction in &block.instructions {
                let SelectedInstructionKind::ImageAddress { reference } = instruction.kind else {
                    continue;
                };
                if !ImageAddressOrigins::indexes(reference) {
                    continue;
                }
                let [operation] = instruction.provenance.operations.as_slice() else {
                    return Err("current image address has no exact source occurrence");
                };
                let output = instruction
                    .operand(0)
                    .filter(|operand| {
                        matches!(
                            operand.access,
                            RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                        )
                    })
                    .ok_or("current image address has no result operand")?;
                let register = function
                    .virtual_registers
                    .get(output.virtual_register.0 as usize)
                    .filter(|register| register.id == output.virtual_register)
                    .ok_or("current image address has an unreadable result register")?;
                rows.push(ImageAddressOrigin {
                    machine: function.machine,
                    block: block.id,
                    instruction: instruction.id,
                    operation: *operation,
                    reference,
                    result: register.scalar_type,
                });
            }
        }
    }
    rows.sort_unstable_by_key(|row| row.key());
    if rows.windows(2).any(|pair| pair[0].key() == pair[1].key()) {
        return Err("current image address repeats its instruction owner");
    }
    Ok(ImageAddressOrigins { rows })
}
