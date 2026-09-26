//! The frame's unwind matrix: one row per supported (architecture,
//! object-format) pair declaring the preservation convention the pair
//! selects, the physical view the frame's stack pointer lives in, and the
//! mechanism an unwind of the frame recovers its continuation address
//! through.
//!
//! x86-64 `call` pushes the continuation into the caller's activation
//! record, so an x86-64 frame keeps it in caller-stack custody: the unwind
//! roster restores only preservation slots, and the release returns the
//! pushed address to the caller's stack. AArch64 `bl` writes the link
//! register and never moves SP, so a leaf frame keeps the link live while
//! a frame whose body executes calls — or any frame under the
//! saved-return-address policy — gives it an exact committed slot at the
//! highest coordinate that the roster restores first.
//!
//! Layout computation and independent replay both resolve the row here.
//! An undeclared pair answers `None` rather than inheriting another row's
//! custody, and each row pins the preservation convention its pair
//! selects, so a requirements plan arriving under a convention another
//! pair owns fails closed instead of reaching for a wildcard arm.

use target::{Architecture, NativeTarget, ObjectFormat};

use crate::machine_emission::frame_layout::{
    FrameAbiPreservationConvention, ReturnAddressFrameCustody,
};

/// The mechanism a declared pair's unwind uses to recover the frame's
/// continuation address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrameContinuationCustody {
    /// `call` pushed the continuation into the caller's activation record:
    /// it occupies `return_address_size_bytes` at the post-prologue stack
    /// pointer plus the committed extent, and the committed-extent release
    /// returns it to the caller's stack. The unwind roster carries no
    /// link-register row.
    CallerActivationStack { return_address_size_bytes: u16 },
    /// `bl` left the continuation in link-register `view`. A leaf frame
    /// keeps it live there; a frame whose body executes calls — or any
    /// frame under `CanonicalSavedReturnAddressFrameV1` — gives it a
    /// `saved_size_bytes` committed slot, size-aligned, at the highest
    /// frame coordinate, and the unwind roster restores it before every
    /// preservation slot.
    LinkRegister {
        view: &'static str,
        saved_size_bytes: u16,
    },
}

impl FrameContinuationCustody {
    /// Whether a recorded return-address custody is an instance of this
    /// declared mechanism: caller-stack custody for a pushed-return row,
    /// live or saved link-register custody for a link-register row.
    pub(crate) fn admits(self, custody: ReturnAddressFrameCustody) -> bool {
        matches!(
            (self, custody),
            (
                FrameContinuationCustody::CallerActivationStack { .. },
                ReturnAddressFrameCustody::CallerActivationStack { .. },
            ) | (
                FrameContinuationCustody::LinkRegister { .. },
                ReturnAddressFrameCustody::LiveLinkRegister { .. }
                    | ReturnAddressFrameCustody::SavedLinkRegister { .. },
            )
        )
    }
}

/// The unwind policy one declared (architecture, object-format) pair owns
/// for an ordinary frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FrameUnwindPolicy {
    /// The preservation convention this pair selects. A requirements plan
    /// carrying any other convention for the pair has drifted off the
    /// declared row, and producer and replay both fail closed on it.
    pub abi: FrameAbiPreservationConvention,
    /// The physical-register-model view the frame's stack pointer lives in.
    pub stack_pointer_view: &'static str,
    /// The mechanism the frame's unwind recovers its continuation address
    /// through.
    pub continuation: FrameContinuationCustody,
}

/// Resolve one declared (architecture, object-format) pair to its frame
/// unwind policy.
///
/// Declared rows are x86-64 with ELF or COFF and AArch64 with ELF or
/// Mach-O — the same four pairs the ABI preservation matrix admits.
/// (x86-64, Mach-O) and (Aarch64, COFF) are undeclared and answer `None`,
/// so a new supported target extends this matrix deliberately instead of
/// widening a wildcard arm.
pub(crate) const fn frame_unwind_policy(target: NativeTarget) -> Option<FrameUnwindPolicy> {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf) => Some(FrameUnwindPolicy {
            abi: FrameAbiPreservationConvention::SystemVAMD64,
            stack_pointer_view: "rsp",
            continuation: FrameContinuationCustody::CallerActivationStack {
                return_address_size_bytes: 8,
            },
        }),
        (Architecture::X86_64, ObjectFormat::Coff) => Some(FrameUnwindPolicy {
            abi: FrameAbiPreservationConvention::MicrosoftX64,
            stack_pointer_view: "rsp",
            continuation: FrameContinuationCustody::CallerActivationStack {
                return_address_size_bytes: 8,
            },
        }),
        (Architecture::Aarch64, ObjectFormat::Elf) => Some(FrameUnwindPolicy {
            abi: FrameAbiPreservationConvention::Aapcs64,
            stack_pointer_view: "sp",
            continuation: FrameContinuationCustody::LinkRegister {
                view: "x30",
                saved_size_bytes: 8,
            },
        }),
        (Architecture::Aarch64, ObjectFormat::MachO) => Some(FrameUnwindPolicy {
            abi: FrameAbiPreservationConvention::DarwinAapcs64,
            stack_pointer_view: "sp",
            continuation: FrameContinuationCustody::LinkRegister {
                view: "x30",
                saved_size_bytes: 8,
            },
        }),
        (Architecture::X86_64, ObjectFormat::MachO)
        | (Architecture::Aarch64, ObjectFormat::Coff) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameContinuationCustody, FrameUnwindPolicy, frame_unwind_policy};
    use crate::machine_emission::frame_layout::{
        FrameAbiPreservationConvention, ReturnAddressFrameCustody,
    };
    use target::{NativeTarget, ObjectFormat};

    #[test]
    fn unwind_matrix_declares_one_row_per_supported_pair() {
        let caller_stack = FrameContinuationCustody::CallerActivationStack {
            return_address_size_bytes: 8,
        };
        let link = FrameContinuationCustody::LinkRegister {
            view: "x30",
            saved_size_bytes: 8,
        };
        for (target, expected) in [
            (
                NativeTarget::linux_x64(),
                FrameUnwindPolicy {
                    abi: FrameAbiPreservationConvention::SystemVAMD64,
                    stack_pointer_view: "rsp",
                    continuation: caller_stack,
                },
            ),
            (
                NativeTarget::windows_x64(),
                FrameUnwindPolicy {
                    abi: FrameAbiPreservationConvention::MicrosoftX64,
                    stack_pointer_view: "rsp",
                    continuation: caller_stack,
                },
            ),
            (
                NativeTarget::uefi_x64(),
                FrameUnwindPolicy {
                    abi: FrameAbiPreservationConvention::MicrosoftX64,
                    stack_pointer_view: "rsp",
                    continuation: caller_stack,
                },
            ),
            (
                NativeTarget::linux_arm64(),
                FrameUnwindPolicy {
                    abi: FrameAbiPreservationConvention::Aapcs64,
                    stack_pointer_view: "sp",
                    continuation: link,
                },
            ),
            (
                NativeTarget::macos_arm64(),
                FrameUnwindPolicy {
                    abi: FrameAbiPreservationConvention::DarwinAapcs64,
                    stack_pointer_view: "sp",
                    continuation: link,
                },
            ),
        ] {
            assert_eq!(frame_unwind_policy(target), Some(expected), "{target:?}");
        }
    }

    #[test]
    fn unwind_matrix_fails_closed_on_undeclared_pairs() {
        for target in [
            NativeTarget {
                object_format: ObjectFormat::MachO,
                ..NativeTarget::linux_x64()
            },
            NativeTarget {
                object_format: ObjectFormat::Coff,
                ..NativeTarget::linux_arm64()
            },
        ] {
            assert_eq!(frame_unwind_policy(target), None, "{target:?}");
        }
    }

    #[test]
    fn continuation_admits_only_its_own_custody_family() {
        let caller_stack = FrameContinuationCustody::CallerActivationStack {
            return_address_size_bytes: 8,
        };
        let link = FrameContinuationCustody::LinkRegister {
            view: "x30",
            saved_size_bytes: 8,
        };
        let view = target_operations_to_selected_instructions::register_model::RegisterViewId(7);
        assert!(
            caller_stack.admits(ReturnAddressFrameCustody::CallerActivationStack {
                post_prologue_offset_bytes: 0,
                size_bytes: 8,
            })
        );
        assert!(!caller_stack.admits(ReturnAddressFrameCustody::LiveLinkRegister { view }));
        assert!(
            !caller_stack.admits(ReturnAddressFrameCustody::SavedLinkRegister {
                view,
                frame_offset_bytes: 0,
                size_bytes: 8,
            })
        );
        assert!(link.admits(ReturnAddressFrameCustody::LiveLinkRegister { view }));
        assert!(link.admits(ReturnAddressFrameCustody::SavedLinkRegister {
            view,
            frame_offset_bytes: 0,
            size_bytes: 8,
        }));
        assert!(
            !link.admits(ReturnAddressFrameCustody::CallerActivationStack {
                post_prologue_offset_bytes: 0,
                size_bytes: 8,
            })
        );
    }
}
