//! Evidence replay passes for object construction. Every retained
//! machine-code record is independently re-decoded from final bytes here;
//! no producer-supplied offset, peak, or shape crosses into the artifact
//! unverified. Installation-record validation reuses the same passes.

pub(crate) mod boundary;
pub(crate) mod instruction_loads;
pub(crate) mod scalar;
pub(crate) mod structural;
pub(crate) mod unit;
pub(crate) mod x86_fma;

fn x86_register_info_writes_stack_pointer(info: &iced_x86::InstructionInfo) -> bool {
    info.used_registers().iter().any(|register| {
        matches!(
            register.register(),
            iced_x86::Register::RSP
                | iced_x86::Register::ESP
                | iced_x86::Register::SP
                | iced_x86::Register::SPL
        ) && matches!(
            register.access(),
            iced_x86::OpAccess::Write
                | iced_x86::OpAccess::CondWrite
                | iced_x86::OpAccess::ReadWrite
                | iced_x86::OpAccess::ReadCondWrite
        )
    })
}
