//! Encode the Windows exports from settled instructions and labels.
use crate::adapters::encode_operation;
use physical_planning::windows_internet::{
    Condition, PlannedAuxiliaryExport, Step, WindowsInternetProgram,
};
use target::Architecture;
fn encode(
    program: PlannedAuxiliaryExport,
) -> Result<machine_code::EncodedAuxiliaryExport, &'static str> {
    let (steps, effects) = program.into_parts();
    let model = register_model::validate_physical_register_model(
        isa_x86_64::x86_64_physical_register_model(),
    )
    .map_err(|_| "invalid helper register model")?;
    let mut bytes = Vec::new();
    let mut labels = [None; 4];
    let mut fixups = Vec::new();
    for step in steps {
        match step {
            Step::Instruction(operation) => {
                encode_operation(Architecture::X86_64, &model, &operation, &mut bytes)
                    .map_err(|_| "helper instruction encoding failed")?
            }
            Step::Label(label) => {
                let slot = labels.get_mut(label).ok_or("helper label outside roster")?;
                if slot.replace(bytes.len()).is_some() {
                    return Err("duplicate helper label");
                }
            }
            Step::Branch { condition, target } => {
                bytes.push(match condition {
                    Some(Condition::UnsignedGreaterOrEqual) => 0x73,
                    Some(Condition::NotEqual) => 0x75,
                    None => 0xeb,
                });
                fixups.push((bytes.len(), target));
                bytes.push(0);
            }
        }
    }
    for (field, label) in fixups {
        let target = labels
            .get(label)
            .copied()
            .flatten()
            .ok_or("missing helper label")?;
        let displacement = i8::try_from(target as i64 - (field + 1) as i64)
            .map_err(|_| "helper branch exceeds its selected encoding")?;
        bytes[field] = displacement as u8;
    }
    Ok(machine_code::EncodedAuxiliaryExport { bytes, effects })
}
pub fn encode_windows_internet_provider(
    program: WindowsInternetProgram,
) -> Result<machine_code::WindowsInternetProviderCode, &'static str> {
    let (make_address, local_address) = program.into_exports();
    Ok(machine_code::WindowsInternetProviderCode {
        make_address: encode(make_address)?,
        local_address: encode(local_address)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn internet_export_effects_match_independent_instruction_decoding() {
        let encoded = encode_windows_internet_provider(
            physical_planning::windows_internet::plan_windows_internet_provider(),
        )
        .unwrap();
        for body in [&encoded.make_address, &encoded.local_address] {
            let decoded = isa_x86_64::decode_x86_64_generated_entry(&body.bytes, None).unwrap();
            assert_eq!(body.effects, decoded.effects);
        }
    }
}
