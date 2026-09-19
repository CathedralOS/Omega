//! The frame protocol's codec matrix: one row per supported
//! (architecture, object-format) pair declaring which ISA codec encodes
//! the pair's prologue and epilogue bytes.
//!
//! Codec selection used to fall out of the continuation-mechanism match:
//! the unwind row named how the pair recovers its continuation address,
//! and the producer and replay each picked whichever encoder arm that
//! mechanism happened to sit in. The mechanism an unwind uses is a
//! different axis than the byte codec an ISA owns, so a new pair declaring
//! a known mechanism would silently inherit that family's codec. The four
//! admitted pairs are declared here once; protocol production and
//! independent replay both resolve through [`frame_protocol_codec`], and an
//! undeclared pair answers `None` instead of a mechanism's borrowed codec.
//!
//! The codec is the ISA's fixed-frame byte shape, not the pair's ABI
//! preservation convention: every admitted x86-64 pair commits with
//! `sub rsp`, saves and restores through `[rsp + offset]`, and releases
//! with one `add rsp`, whether the pair unwinds under System V or
//! Microsoft x64 preservation; every admitted AArch64 pair uses the same
//! `sub sp` / `[sp + offset]` / `add sp` shape under AAPCS64 or Darwin.
//! The convention difference belongs to the unwind matrix's declared
//! `abi` row, which the layout stage already pinned when the roster was
//! admitted.

use target::{Architecture, NativeTarget, ObjectFormat};

/// The ISA frame-protocol codec one declared pair selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FrameProtocolCodec {
    /// The canonical fixed-frame x86-64 codec
    /// (`encode_system_v_amd64_frame_protocol`): `sub rsp` commits with an
    /// exact probe roster, `[rsp + offset]` saves and restores, and one
    /// `add rsp` release returning the pushed return address to the
    /// caller's activation stack.
    X86_64,
    /// The canonical fixed-frame AArch64 codec
    /// (`encode_aapcs64_frame_protocol`): `sub sp` commits with an exact
    /// probe roster, `[sp + offset]` saves and restores including a saved
    /// link register, and one `add sp` release.
    Aarch64,
}

/// Resolve one declared (architecture, object-format) pair to its frame
/// protocol codec.
///
/// Declared rows are the same four pairs the unwind matrix admits: x86-64
/// with ELF or COFF selects the x86-64 codec, and AArch64 with ELF or
/// Mach-O selects the AArch64 codec. (x86-64, Mach-O) and (Aarch64, COFF)
/// are undeclared and answer `None`, so a new supported target extends
/// this matrix deliberately rather than inheriting a codec through a
/// continuation-mechanism match arm.
pub(super) const fn frame_protocol_codec(target: NativeTarget) -> Option<FrameProtocolCodec> {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf) | (Architecture::X86_64, ObjectFormat::Coff) => {
            Some(FrameProtocolCodec::X86_64)
        }
        (Architecture::Aarch64, ObjectFormat::Elf)
        | (Architecture::Aarch64, ObjectFormat::MachO) => Some(FrameProtocolCodec::Aarch64),
        (Architecture::X86_64, ObjectFormat::MachO)
        | (Architecture::Aarch64, ObjectFormat::Coff) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameProtocolCodec, frame_protocol_codec};
    use crate::frame_layout::{FrameContinuationCustody, frame_unwind_policy};
    use target::{Architecture, NativeTarget, ObjectFormat};

    #[test]
    fn codec_matrix_declares_one_row_per_supported_pair() {
        for (target, expected) in [
            (NativeTarget::linux_x64(), FrameProtocolCodec::X86_64),
            (NativeTarget::windows_x64(), FrameProtocolCodec::X86_64),
            (NativeTarget::uefi_x64(), FrameProtocolCodec::X86_64),
            (NativeTarget::linux_arm64(), FrameProtocolCodec::Aarch64),
            (NativeTarget::macos_arm64(), FrameProtocolCodec::Aarch64),
        ] {
            assert_eq!(frame_protocol_codec(target), Some(expected), "{target:?}");
        }
    }

    #[test]
    fn codec_matrix_fails_closed_on_undeclared_pairs() {
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
            assert_eq!(frame_protocol_codec(target), None, "{target:?}");
        }
    }

    #[test]
    fn codec_matrix_agrees_with_the_unwind_matrix_on_every_pair() {
        // The two matrices declare the same supported pair set, and each
        // pair's codec serves the continuation mechanism its unwind row
        // declares: the x86-64 codec's `add rsp` release only implements a
        // pushed-return caller-stack custody, and the AArch64 codec's slot
        // restores exist to recover a saved link register. A pair declared
        // in one matrix but not the other — or declared with a mechanism
        // the codec cannot serve — must fail here, not emit bytes.
        for architecture in [Architecture::X86_64, Architecture::Aarch64] {
            for object_format in [ObjectFormat::Elf, ObjectFormat::MachO, ObjectFormat::Coff] {
                let target = NativeTarget {
                    architecture,
                    object_format,
                    pointer_size: 8,
                    pointer_alignment: 8,
                };
                let codec = frame_protocol_codec(target);
                let unwind = frame_unwind_policy(target);
                assert_eq!(
                    codec.is_some(),
                    unwind.is_some(),
                    "{target:?}: codec and unwind matrices must declare the same pairs"
                );
                let serves = matches!(
                    (codec, unwind.map(|policy| policy.continuation)),
                    (
                        Some(FrameProtocolCodec::X86_64),
                        Some(FrameContinuationCustody::CallerActivationStack { .. }),
                    ) | (
                        Some(FrameProtocolCodec::Aarch64),
                        Some(FrameContinuationCustody::LinkRegister { .. }),
                    ) | (None, None)
                );
                assert!(
                    serves,
                    "{target:?}: the declared codec cannot serve the declared continuation"
                );
            }
        }
    }
}
