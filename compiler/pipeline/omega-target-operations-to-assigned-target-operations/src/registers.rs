use omega_assigned_target_operations::{AssignedRegisterName, X86_64AssignedRegister};
use omega_target::Architecture;

pub(crate) fn scratch_register_name(architecture: Architecture, slot: u16) -> AssignedRegisterName {
    match architecture {
        Architecture::Aarch64 => {
            let register = 19u8.saturating_add((slot % 9) as u8);
            AssignedRegisterName::Aarch64X(register)
        }
        Architecture::X86_64 => AssignedRegisterName::X86_64(match slot % 6 {
            0 => X86_64AssignedRegister::R10,
            1 => X86_64AssignedRegister::R11,
            2 => X86_64AssignedRegister::R12,
            3 => X86_64AssignedRegister::R13,
            4 => X86_64AssignedRegister::R14,
            _ => X86_64AssignedRegister::R15,
        }),
    }
}
