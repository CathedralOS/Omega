//! The frame plan's call-site stack contract: the stack-pointer alignment an
//! ABI requires while one of the body's calls executes, and the residue the
//! architecture's call sequence leaves pushed at callee entry.
//!
//! The alignment is the ABI's own declaration — the selected preservation
//! convention's `stack_alignment`, already sealed by the validated register
//! environment the frame plan binds — never a constant the frame allocator
//! happens to know. The entry residue is the call instruction's stack
//! contribution: x86-64 `call` pushes the eight-byte return address, so the
//! callee enters with the stack pointer that far past the boundary its
//! caller aligned; AArch64 `bl` writes the link register and never moves SP,
//! so the callee inherits the caller's alignment directly. A frame whose
//! body executes calls commits an extent carrying exactly that residue
//! modulo the declared alignment, returning the post-prologue stack pointer
//! to the boundary before any call site runs.
//!
//! Layout computation and independent replay both resolve the contract here.
//! An undeclared (architecture, convention) pair answers `None` rather than
//! borrowing another row's constants, so a new supported target extends the
//! matrix deliberately instead of silently keeping a shared literal.

use target::Architecture;

use crate::machine_emission::frame_layout::{
    FrameAbiPreservationConvention, ValidatedTargetRegisterEnvironment,
};

/// The call-boundary stack contract one (architecture, ABI convention) pair
/// declares for a frame whose body executes calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CallSiteStackContract {
    /// Stack-pointer alignment the ABI requires at a call boundary, in bytes:
    /// the selected preservation convention's declared `stack_alignment`.
    pub stack_alignment_bytes: u16,
    /// Bytes the architecture's call sequence leaves pushed on the stack at
    /// callee entry. A committed frame extent keeps this residue modulo
    /// `stack_alignment_bytes` so the post-prologue stack pointer meets the
    /// declared boundary again.
    pub entry_residue_bytes: u64,
}

/// Resolve the call-site stack contract a declared (architecture, convention)
/// pair owns.
///
/// x86-64 `call` pushes the eight-byte return address, so both x86-64
/// conventions enter the callee with residue 8. AArch64 `bl` writes the link
/// register and never touches SP, so AAPCS64 pairs carry residue 0. Other
/// pairs are undeclared and answer `None`.
pub(super) fn call_site_stack_contract(
    environment: &ValidatedTargetRegisterEnvironment,
    abi: FrameAbiPreservationConvention,
) -> Option<CallSiteStackContract> {
    let stack_alignment_bytes = target_operations_to_selected_instructions::register_environment::selected_abi_preservation(environment)
        .ok()?
        .convention
        .stack_alignment;
    let entry_residue_bytes = match (environment.target().architecture, abi) {
        (
            Architecture::X86_64,
            FrameAbiPreservationConvention::SystemVAMD64
            | FrameAbiPreservationConvention::MicrosoftX64,
        ) => 8,
        (
            Architecture::Aarch64,
            FrameAbiPreservationConvention::Aapcs64 | FrameAbiPreservationConvention::DarwinAapcs64,
        ) => 0,
        _ => return None,
    };
    Some(CallSiteStackContract {
        stack_alignment_bytes,
        entry_residue_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::call_site_stack_contract;
    use crate::machine_emission::frame_layout::FrameAbiPreservationConvention;
    use target::NativeTarget;

    #[test]
    fn call_site_contract_binds_the_declared_convention_alignment() {
        for (target, abi, residue) in [
            (
                NativeTarget::linux_x64(),
                FrameAbiPreservationConvention::SystemVAMD64,
                8,
            ),
            (
                NativeTarget::windows_x64(),
                FrameAbiPreservationConvention::MicrosoftX64,
                8,
            ),
            (
                NativeTarget::uefi_x64(),
                FrameAbiPreservationConvention::MicrosoftX64,
                8,
            ),
            (
                NativeTarget::linux_arm64(),
                FrameAbiPreservationConvention::Aapcs64,
                0,
            ),
            (
                NativeTarget::macos_arm64(),
                FrameAbiPreservationConvention::DarwinAapcs64,
                0,
            ),
        ] {
            let environment =
                target_operations_to_selected_instructions::register_environment::baseline_target_register_environment(target).unwrap();
            let contract = call_site_stack_contract(&environment, abi).unwrap();
            assert_eq!(
                contract.stack_alignment_bytes,
                target_operations_to_selected_instructions::register_environment::selected_abi_preservation(&environment)
                    .unwrap()
                    .convention
                    .stack_alignment,
                "{target:?}"
            );
            assert_eq!(contract.entry_residue_bytes, residue, "{target:?}");
        }
    }

    #[test]
    fn call_site_contract_fails_closed_on_undeclared_pairs() {
        let environment =
            target_operations_to_selected_instructions::register_environment::baseline_target_register_environment(NativeTarget::linux_x64())
                .unwrap();
        for abi in [
            FrameAbiPreservationConvention::Aapcs64,
            FrameAbiPreservationConvention::DarwinAapcs64,
        ] {
            assert_eq!(call_site_stack_contract(&environment, abi), None, "{abi:?}");
        }
    }
}
