/*
We encode and independently replay target prologue and epilogue byte protocols (`ValidatedTargetFrameProtocolEncoding`) for validated function frame layouts (`ValidatedTargetFrameLayout`). Using the explicit `(Architecture, ObjectFormat)` codec matrix (`frame_protocol_codec`), we invoke the target-owned x86-64 or AArch64 frame-protocol encoder over each function's committed non-red-zone byte extent, stack-probe schedule, and reversed callee-save restore roster, packing all prologue and epilogue bytes into one contiguous arena referenced by per-function `FrameProtocolByteSpan` records. The selected return instruction itself stays outside the frame protocol so `frame_application` can insert the epilogue immediately before each retained return row.
*/

use std::sync::Arc;

use register_environment::ValidatedTargetRegisterEnvironment;
use target::{Architecture, NativeTarget, ObjectFormat};

pub use machine_code::{
    FrameProtocolByteSpan, FunctionTargetFrameProtocolEncoding,
    TargetFrameProtocolEncodingIdentity, TargetFrameProtocolEncodingPlan,
    TargetFrameProtocolEncodingPolicy, target_frame_protocol_encoding_identity,
};

use crate::count_where;
use crate::frame_layout::{
    TargetFrameLayoutIdentity, ValidatedTargetFrameLayout, committed_extent, frame_unwind_policy,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameProtocolCodec {
    X86_64,
    Aarch64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetFrameProtocolEncodingError {
    RootMismatch,
    UnsupportedPolicy,
    UnsupportedTarget,
    FunctionRosterMismatch,
    ByteArenaOverflow,
    UnsupportedReturnAddressCustody,
    X86(isa_x86_64::X86_64FrameProtocolError),
    Aarch64(isa_aarch64::Aarch64FrameProtocolError),
    NonCanonicalEncoding,
}

diagnostics::failed_display!(
    TargetFrameProtocolEncodingError,
    "target frame protocol encoding failed"
);

impl std::error::Error for TargetFrameProtocolEncodingError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetFrameProtocolEncodingReceipt {
    identity: TargetFrameProtocolEncodingIdentity, // Content digest of the encoded frame-protocol plan.
    frame_layout: TargetFrameLayoutIdentity,       // Upstream validated frame-layout identity.
    target: NativeTarget, // Target platform for which the protocol was encoded.
    policy: TargetFrameProtocolEncodingPolicy, // Frame-protocol encoding policy applied.
    function_count: usize, // Number of functions covered by the plan.
    byte_count: usize,    // Total byte length of the packed prologue/epilogue arena.
    nonempty_prologue_count: usize, // Count of functions with non-empty prologue spans.
    nonempty_epilogue_count: usize, // Count of functions with non-empty epilogue spans.
}

impl TargetFrameProtocolEncodingReceipt {
    pub const fn identity(self) -> TargetFrameProtocolEncodingIdentity {
        self.identity
    }

    pub const fn frame_layout(self) -> TargetFrameLayoutIdentity {
        self.frame_layout
    }

    pub const fn target(self) -> NativeTarget {
        self.target
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTargetFrameProtocolEncoding {
    pub(crate) plan: Arc<TargetFrameProtocolEncodingPlan>, // Admitted frame-protocol encoding plan.
    pub(crate) receipt: TargetFrameProtocolEncodingReceipt, // Sealed summary receipt for downstream custody checks.
}

impl ValidatedTargetFrameProtocolEncoding {
    pub fn plan(&self) -> &TargetFrameProtocolEncodingPlan {
        &self.plan
    }

    pub fn shared_plan(&self) -> Arc<TargetFrameProtocolEncodingPlan> {
        Arc::clone(&self.plan)
    }

    pub const fn receipt(&self) -> TargetFrameProtocolEncodingReceipt {
        self.receipt
    }
}

pub(crate) fn stage_target_frame_protocol_encoding(
    frame: &ValidatedTargetFrameLayout,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: TargetFrameProtocolEncodingPolicy,
) -> Result<ValidatedTargetFrameProtocolEncoding, TargetFrameProtocolEncodingError> {
    let plan = derive(frame, environment, policy)?;
    validate_target_frame_protocol_encoding(frame, environment, plan)
}

pub(crate) fn validate_target_frame_protocol_encoding(
    frame: &ValidatedTargetFrameLayout,
    environment: &ValidatedTargetRegisterEnvironment,
    candidate: TargetFrameProtocolEncodingPlan,
) -> Result<ValidatedTargetFrameProtocolEncoding, TargetFrameProtocolEncodingError> {
    if candidate.frame_layout != frame.receipt().identity()
        || candidate.register_environment != environment.identity()
        || candidate.physical_register_model != environment.physical().identity()
        || candidate.target != environment.target()
    {
        return Err(TargetFrameProtocolEncodingError::RootMismatch);
    }
    validate_bytes(frame, environment, &candidate)?;
    let receipt = seal(&candidate);
    Ok(ValidatedTargetFrameProtocolEncoding {
        plan: Arc::new(candidate),
        receipt,
    })
}

const fn frame_protocol_codec(target: NativeTarget) -> Option<FrameProtocolCodec> {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf)
        | (Architecture::X86_64, ObjectFormat::Coff)
        | (Architecture::X86_64, ObjectFormat::MachO) => Some(FrameProtocolCodec::X86_64),
        (Architecture::Aarch64, ObjectFormat::Elf)
        | (Architecture::Aarch64, ObjectFormat::MachO) => Some(FrameProtocolCodec::Aarch64),
        (Architecture::Aarch64, ObjectFormat::Coff) => None,
    }
}

fn encode_frame_function(
    codec: FrameProtocolCodec,
    environment: &ValidatedTargetRegisterEnvironment,
    function: &machine_code::FunctionTargetFrameLayout,
    committed_bytes: u64,
) -> Result<(Vec<u8>, Vec<u8>), TargetFrameProtocolEncodingError> {
    Ok(match codec {
        FrameProtocolCodec::X86_64 => {
            let slots = function
                .unwind
                .restores
                .iter()
                .rev()
                .map(|restore| isa_x86_64::X86_64FrameSlot {
                    view: restore.view,
                    offset_bytes: restore.frame_offset_bytes,
                    size_bytes: restore.size_bytes,
                })
                .collect::<Vec<_>>();
            isa_x86_64::encode_system_v_amd64_frame_protocol(
                environment.physical(),
                committed_bytes,
                isa_x86_64::X86_64StackProbe {
                    interval_bytes: function.stack_probe.interval_bytes,
                    touches: function.stack_probe.touches,
                },
                &slots,
            )
            .map_err(TargetFrameProtocolEncodingError::X86)?
        }
        FrameProtocolCodec::Aarch64 => {
            let slots = function
                .unwind
                .restores
                .iter()
                .rev()
                .map(|restore| isa_aarch64::Aarch64FrameSlot {
                    view: restore.view,
                    offset_bytes: restore.frame_offset_bytes,
                    size_bytes: restore.size_bytes,
                })
                .collect::<Vec<_>>();
            isa_aarch64::encode_aapcs64_frame_protocol(
                environment.physical(),
                committed_bytes,
                isa_aarch64::Aarch64StackProbe {
                    interval_bytes: function.stack_probe.interval_bytes,
                    touches: function.stack_probe.touches,
                },
                &slots,
            )
            .map_err(TargetFrameProtocolEncodingError::Aarch64)?
        }
    })
}

fn derive(
    frame: &ValidatedTargetFrameLayout,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: TargetFrameProtocolEncodingPolicy,
) -> Result<TargetFrameProtocolEncodingPlan, TargetFrameProtocolEncodingError> {
    if policy != TargetFrameProtocolEncodingPolicy::CanonicalFixedFrameV1 {
        return Err(TargetFrameProtocolEncodingError::UnsupportedPolicy);
    }
    if frame.plan().target != environment.target()
        || frame.plan().register_environment != environment.identity()
        || frame.plan().physical_register_model != environment.physical().identity()
    {
        return Err(TargetFrameProtocolEncodingError::RootMismatch);
    }
    let codec = frame_protocol_codec(environment.target())
        .ok_or(TargetFrameProtocolEncodingError::UnsupportedTarget)?;
    let continuation = frame_unwind_policy(environment.target())
        .ok_or(TargetFrameProtocolEncodingError::UnsupportedTarget)?
        .continuation;
    let mut bytes = Vec::new();
    let mut functions = Vec::with_capacity(frame.plan().functions.len());
    for function in &frame.plan().functions {
        let committed_bytes = committed_extent(function)
            .ok_or(TargetFrameProtocolEncodingError::NonCanonicalEncoding)?;
        if !continuation.admits(function.return_address) {
            return Err(TargetFrameProtocolEncodingError::UnsupportedReturnAddressCustody);
        }
        let (prologue_bytes, epilogue_bytes) =
            encode_frame_function(codec, environment, function, committed_bytes)?;
        let prologue = append(&mut bytes, &prologue_bytes)?;
        let epilogue = append(&mut bytes, &epilogue_bytes)?;
        functions.push(FunctionTargetFrameProtocolEncoding {
            machine: function.machine,
            prologue,
            epilogue,
        });
    }
    Ok(TargetFrameProtocolEncodingPlan {
        frame_layout: frame.receipt().identity(),
        register_environment: environment.identity(),
        physical_register_model: environment.physical().identity(),
        target: environment.target(),
        policy,
        functions,
        bytes,
    })
}

fn append(
    arena: &mut Vec<u8>,
    value: &[u8],
) -> Result<FrameProtocolByteSpan, TargetFrameProtocolEncodingError> {
    let offset = u32::try_from(arena.len())
        .map_err(|_| TargetFrameProtocolEncodingError::ByteArenaOverflow)?;
    let length = u32::try_from(value.len())
        .map_err(|_| TargetFrameProtocolEncodingError::ByteArenaOverflow)?;
    arena
        .len()
        .checked_add(value.len())
        .ok_or(TargetFrameProtocolEncodingError::ByteArenaOverflow)?;
    arena.extend_from_slice(value);
    Ok(FrameProtocolByteSpan { offset, length })
}

fn validate_bytes(
    frame: &ValidatedTargetFrameLayout,
    environment: &ValidatedTargetRegisterEnvironment,
    candidate: &TargetFrameProtocolEncodingPlan,
) -> Result<(), TargetFrameProtocolEncodingError> {
    if candidate.policy != TargetFrameProtocolEncodingPolicy::CanonicalFixedFrameV1 {
        return Err(TargetFrameProtocolEncodingError::UnsupportedPolicy);
    }
    if frame.plan().target != environment.target()
        || frame.plan().register_environment != environment.identity()
        || frame.plan().physical_register_model != environment.physical().identity()
    {
        return Err(TargetFrameProtocolEncodingError::RootMismatch);
    }
    if candidate.functions.len() != frame.plan().functions.len() {
        return Err(TargetFrameProtocolEncodingError::NonCanonicalEncoding);
    }
    let codec = frame_protocol_codec(environment.target())
        .ok_or(TargetFrameProtocolEncodingError::UnsupportedTarget)?;
    let continuation = frame_unwind_policy(environment.target())
        .ok_or(TargetFrameProtocolEncodingError::UnsupportedTarget)?
        .continuation;
    let mut cursor = 0;
    for (row, function) in candidate.functions.iter().zip(&frame.plan().functions) {
        if row.machine != function.machine {
            return Err(TargetFrameProtocolEncodingError::NonCanonicalEncoding);
        }
        let committed_bytes = committed_extent(function)
            .ok_or(TargetFrameProtocolEncodingError::NonCanonicalEncoding)?;
        if !continuation.admits(function.return_address) {
            return Err(TargetFrameProtocolEncodingError::UnsupportedReturnAddressCustody);
        }
        let (prologue, epilogue) =
            encode_frame_function(codec, environment, function, committed_bytes)?;
        cursor = validate_span(&candidate.bytes, cursor, row.prologue, &prologue)?;
        cursor = validate_span(&candidate.bytes, cursor, row.epilogue, &epilogue)?;
    }
    if cursor != candidate.bytes.len() {
        return Err(TargetFrameProtocolEncodingError::NonCanonicalEncoding);
    }
    Ok(())
}

fn validate_span(
    arena: &[u8],
    cursor: usize,
    span: FrameProtocolByteSpan,
    expected: &[u8],
) -> Result<usize, TargetFrameProtocolEncodingError> {
    let end = cursor
        .checked_add(expected.len())
        .ok_or(TargetFrameProtocolEncodingError::ByteArenaOverflow)?;
    if usize::try_from(span.offset).ok() != Some(cursor)
        || usize::try_from(span.length).ok() != Some(expected.len())
        || arena.get(cursor..end) != Some(expected)
    {
        return Err(TargetFrameProtocolEncodingError::NonCanonicalEncoding);
    }
    Ok(end)
}

fn seal(plan: &TargetFrameProtocolEncodingPlan) -> TargetFrameProtocolEncodingReceipt {
    TargetFrameProtocolEncodingReceipt {
        identity: self::target_frame_protocol_encoding_identity(plan),
        frame_layout: plan.frame_layout,
        target: plan.target,
        policy: plan.policy,
        function_count: plan.functions.len(),
        byte_count: plan.bytes.len(),
        nonempty_prologue_count: count_where(&plan.functions, |row| row.prologue.length != 0),
        nonempty_epilogue_count: count_where(&plan.functions, |row| row.epilogue.length != 0),
    }
}

#[cfg(test)]
mod tests {
    use target::{Architecture, NativeTarget, ObjectFormat};

    use super::{FrameProtocolByteSpan, FrameProtocolCodec, frame_protocol_codec, validate_span};
    use crate::frame_layout::{FrameContinuationCustody, frame_unwind_policy};

    #[test]
    fn span_replay_rejects_padding_aliases_truncation_and_substituted_bytes() {
        let arena = [1, 2, 3];
        let span = FrameProtocolByteSpan {
            offset: 1,
            length: 2,
        };
        assert_eq!(validate_span(&arena, 1, span, &[2, 3]), Ok(3));
        assert!(validate_span(&arena, 0, span, &[2, 3]).is_err());
        assert!(
            validate_span(
                &arena,
                1,
                FrameProtocolByteSpan {
                    offset: 0,
                    length: 2
                },
                &[2, 3]
            )
            .is_err()
        );
        assert!(
            validate_span(
                &arena,
                1,
                FrameProtocolByteSpan {
                    offset: 1,
                    length: 1
                },
                &[2, 3]
            )
            .is_err()
        );
        assert!(validate_span(&arena, 1, span, &[3, 2]).is_err());
        assert!(validate_span(&arena[..2], 1, span, &[2, 3]).is_err());
    }

    #[test]
    fn codec_matrix_declares_one_row_per_supported_pair() {
        for (target, expected) in [
            (NativeTarget::linux_x64(), FrameProtocolCodec::X86_64),
            (NativeTarget::macos_x64(), FrameProtocolCodec::X86_64),
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
        let target = NativeTarget {
            object_format: ObjectFormat::Coff,
            ..NativeTarget::linux_arm64()
        };
        assert_eq!(frame_protocol_codec(target), None, "{target:?}");
    }

    #[test]
    fn codec_matrix_agrees_with_the_unwind_matrix_on_every_pair() {
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
