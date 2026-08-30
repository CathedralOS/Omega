//! Exact replay for `open_at` on an unknown compiler-owned descriptor.

use crate::{
    EvaluationObservations, FilesystemByteOperand, FilesystemOperationAttempt, FilesystemReplay,
    FilesystemScalarOperand, FilesystemScalarOperandValue, FilesystemSourceInputReplayRecord,
};

use super::handle_failures::{
    unknown_descriptor_failure_attempt, unknown_descriptor_failure_has_exact_core_shape,
    unknown_handle_input_failure_replay_from_observations,
    unknown_handle_input_failure_replay_from_record,
};

const OPEN_AT_OPERATION_TAG: u16 = 14;

/// Optional exact Source-input prefix followed by one `open_at` call whose
/// directory descriptor deterministically fails with `EBADF`.
///
/// The relative component and flags are exact authored inputs. The component
/// remains inert bytes: an unknown descriptor provides no rooted path or
/// filesystem authority from which a larger coordinate could be inferred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputUnknownDescriptorOpenAtReplayRecord {
    source_input: Option<FilesystemSourceInputReplayRecord>,
    relative_component: Vec<u8>,
    flags: i32,
}

impl FilesystemInputUnknownDescriptorOpenAtReplayRecord {
    pub fn new(
        source_input: Option<FilesystemSourceInputReplayRecord>,
        relative_component: Vec<u8>,
        flags: i32,
    ) -> Result<Self, String> {
        if !filesystem_relative_component_is_safe(&relative_component) {
            return Err(
                "filesystem replay open_at name is not one nonempty portable relative component"
                    .to_owned(),
            );
        }
        Ok(Self {
            source_input,
            relative_component,
            flags,
        })
    }

    pub const fn source_input(&self) -> Option<&FilesystemSourceInputReplayRecord> {
        self.source_input.as_ref()
    }

    pub fn relative_component(&self) -> &[u8] {
        &self.relative_component
    }

    pub const fn flags(&self) -> i32 {
        self.flags
    }

    fn into_parts(self) -> (Option<FilesystemSourceInputReplayRecord>, Vec<u8>, i32) {
        (self.source_input, self.relative_component, self.flags)
    }
}

impl FilesystemReplay {
    /// Construct the closed optional-Source plus one unknown-directory-
    /// descriptor `open_at` failure from typed compiler-owned evidence.
    pub fn from_input_unknown_descriptor_open_at_record(
        record: FilesystemInputUnknownDescriptorOpenAtReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, relative_component, flags) = record.into_parts();
        unknown_handle_input_failure_replay_from_record(
            source_input,
            unknown_descriptor_open_at_attempt(relative_component, flags),
            unknown_descriptor_open_at_attempt_is_exact,
            "open_at",
        )
    }

    /// Validate observed evidence for an optional Source-input prefix followed
    /// by exactly one `open_at` failure on an unknown directory descriptor.
    pub fn from_input_unknown_descriptor_open_at_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        unknown_handle_input_failure_replay_from_observations(
            observations,
            unknown_descriptor_open_at_attempt_is_exact,
            "open_at",
        )
    }
}

pub(crate) fn unknown_descriptor_open_at_from_exact_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Option<(&[u8], i32)> {
    let [flags] = attempt.scalar_operands.as_slice() else {
        return None;
    };
    let FilesystemScalarOperand {
        operand_ordinal: 2,
        value: FilesystemScalarOperandValue::I32(flags),
    } = flags
    else {
        return None;
    };
    let [relative_component] = attempt.byte_operands.as_slice() else {
        return None;
    };
    (relative_component.operand_ordinal == 1
        && filesystem_relative_component_is_safe(&relative_component.bytes)
        && attempt.mutable_byte_operand_resolutions.is_empty()
        && attempt.mutable_byte_operands.is_empty()
        && unknown_descriptor_failure_has_exact_core_shape(attempt, OPEN_AT_OPERATION_TAG))
    .then_some((relative_component.bytes.as_slice(), *flags))
}

pub(crate) fn unknown_descriptor_open_at_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    unknown_descriptor_open_at_from_exact_attempt(attempt).is_some()
}

pub(crate) fn unknown_descriptor_open_at_attempt(
    relative_component: Vec<u8>,
    flags: i32,
) -> FilesystemOperationAttempt {
    debug_assert!(filesystem_relative_component_is_safe(&relative_component));
    let mut attempt = unknown_descriptor_failure_attempt(
        OPEN_AT_OPERATION_TAG,
        vec![FilesystemScalarOperand {
            operand_ordinal: 2,
            value: FilesystemScalarOperandValue::I32(flags),
        }],
    );
    attempt.byte_operands = vec![FilesystemByteOperand {
        operand_ordinal: 1,
        bytes: relative_component,
    }];
    attempt
}

fn filesystem_relative_component_is_safe(bytes: &[u8]) -> bool {
    !bytes.is_empty()
        && bytes != b"."
        && bytes != b".."
        && !bytes.contains(&b'/')
        && !bytes.contains(&b'\\')
        && !bytes.contains(&0)
}
