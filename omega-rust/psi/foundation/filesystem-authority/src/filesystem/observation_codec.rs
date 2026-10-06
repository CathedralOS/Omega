//! Byte codecs for retained filesystem-observation evidence.
//!
//! Package review persists compiled-package evidence across invocations; the
//! retained filesystem leaf rows need a recoverable wire form. Each decoder
//! enforces the owning type's invariants — handle identities stay nonzero and
//! tags stay inside the known domain — so a malformed record fails decode
//! rather than materializing a row the evaluator never wrote.

use super::grants::{
    FilesystemGrantAccess, FilesystemGrantRefusal, FilesystemGrantRefusalReason,
    FilesystemObservationProvider,
};
use super::logical_handles::{
    FilesystemLogicalHandleIdentity, FilesystemLogicalHandleInput,
    FilesystemLogicalHandleInputResolution, FilesystemLogicalHandleKind,
    FilesystemLogicalHandleOutput, FilesystemLogicalHandleOutputSource,
};
use super::FilesystemOperationResult;

/// Sequential byte cursor over one retained record's payload. Out-of-range
/// reads surface as `None`; callers map them to a truncated-record error.
pub struct ObservationCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ObservationCursor<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub fn exhausted(&self) -> bool {
        self.offset == self.bytes.len()
    }

    pub fn u8(&mut self) -> Option<u8> {
        let byte = *self.bytes.get(self.offset)?;
        self.offset += 1;
        Some(byte)
    }

    pub fn u16(&mut self) -> Option<u16> {
        let bytes = self.take(2)?;
        Some(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub fn u32(&mut self) -> Option<u32> {
        let bytes = self.take(4)?;
        Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn u64(&mut self) -> Option<u64> {
        let bytes = self.take(8)?;
        Some(u64::from_le_bytes(bytes.try_into().ok()?))
    }

    pub fn i32(&mut self) -> Option<i32> {
        Some(self.u32()? as i32)
    }

    pub fn i64(&mut self) -> Option<i64> {
        Some(self.u64()? as i64)
    }

    pub fn bytes(&mut self, length: usize) -> Option<&'a [u8]> {
        self.take(length)
    }

    /// Length-prefixed byte field.
    pub fn framed(&mut self) -> Option<&'a [u8]> {
        let length = self.u64()? as usize;
        self.take(length)
    }

    fn take(&mut self, length: usize) -> Option<&'a [u8]> {
        let end = self.offset.checked_add(length)?;
        let slice = self.bytes.get(self.offset..end)?;
        self.offset = end;
        Some(slice)
    }
}

pub fn push_framed(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

pub fn encode_observation_provider(provider: FilesystemObservationProvider) -> u8 {
    match provider {
        FilesystemObservationProvider::Virtual => 0,
        FilesystemObservationProvider::RealUnscoped => 1,
        FilesystemObservationProvider::RealScoped => 2,
    }
}

pub fn recover_observation_provider(tag: u8) -> Option<FilesystemObservationProvider> {
    match tag {
        0 => Some(FilesystemObservationProvider::Virtual),
        1 => Some(FilesystemObservationProvider::RealUnscoped),
        2 => Some(FilesystemObservationProvider::RealScoped),
        _ => None,
    }
}

pub fn encode_operation_result(result: FilesystemOperationResult, out: &mut Vec<u8>) {
    match result {
        FilesystemOperationResult::Scalar(value) => {
            out.push(0);
            out.extend_from_slice(&value.to_le_bytes());
        }
        FilesystemOperationResult::LogicalHandle(identity) => {
            out.push(1);
            out.extend_from_slice(&identity.get().to_le_bytes());
        }
    }
}

pub fn recover_operation_result(
    cursor: &mut ObservationCursor<'_>,
) -> Option<FilesystemOperationResult> {
    match cursor.u8()? {
        0 => Some(FilesystemOperationResult::Scalar(cursor.i64()?)),
        1 => Some(FilesystemOperationResult::LogicalHandle(
            FilesystemLogicalHandleIdentity::new(cursor.u64()?)?,
        )),
        _ => None,
    }
}

pub fn encode_logical_handle_kind(kind: FilesystemLogicalHandleKind) -> u8 {
    match kind {
        FilesystemLogicalHandleKind::Descriptor => 0,
        FilesystemLogicalHandleKind::Native => 1,
        FilesystemLogicalHandleKind::Find => 2,
    }
}

pub fn recover_logical_handle_kind(tag: u8) -> Option<FilesystemLogicalHandleKind> {
    match tag {
        0 => Some(FilesystemLogicalHandleKind::Descriptor),
        1 => Some(FilesystemLogicalHandleKind::Native),
        2 => Some(FilesystemLogicalHandleKind::Find),
        _ => None,
    }
}

pub fn encode_logical_handle_input(input: FilesystemLogicalHandleInput, out: &mut Vec<u8>) {
    out.push(input.operand_ordinal());
    out.push(encode_logical_handle_kind(input.kind()));
    match input.resolution() {
        FilesystemLogicalHandleInputResolution::Resolved(identity) => {
            out.push(0);
            out.extend_from_slice(&identity.get().to_le_bytes());
        }
        FilesystemLogicalHandleInputResolution::Null => out.push(1),
        FilesystemLogicalHandleInputResolution::Unknown => out.push(2),
    }
}

pub fn recover_logical_handle_input(
    cursor: &mut ObservationCursor<'_>,
) -> Option<FilesystemLogicalHandleInput> {
    let operand_ordinal = cursor.u8()?;
    let kind = recover_logical_handle_kind(cursor.u8()?)?;
    let resolution = match cursor.u8()? {
        0 => FilesystemLogicalHandleInputResolution::Resolved(
            FilesystemLogicalHandleIdentity::new(cursor.u64()?)?,
        ),
        1 => FilesystemLogicalHandleInputResolution::Null,
        2 => FilesystemLogicalHandleInputResolution::Unknown,
        _ => return None,
    };
    Some(FilesystemLogicalHandleInput {
        operand_ordinal,
        kind,
        resolution,
    })
}

pub fn encode_logical_handle_output(output: FilesystemLogicalHandleOutput, out: &mut Vec<u8>) {
    out.push(encode_logical_handle_kind(output.kind()));
    out.extend_from_slice(&output.identity().get().to_le_bytes());
    match output.source() {
        FilesystemLogicalHandleOutputSource::Created => out.push(0),
        FilesystemLogicalHandleOutputSource::Duplicated(identity) => {
            out.push(1);
            out.extend_from_slice(&identity.get().to_le_bytes());
        }
        FilesystemLogicalHandleOutputSource::Borrowed(identity) => {
            out.push(2);
            out.extend_from_slice(&identity.get().to_le_bytes());
        }
    }
}

pub fn recover_logical_handle_output(
    cursor: &mut ObservationCursor<'_>,
) -> Option<FilesystemLogicalHandleOutput> {
    let kind = recover_logical_handle_kind(cursor.u8()?)?;
    let identity = FilesystemLogicalHandleIdentity::new(cursor.u64()?)?;
    let source = match cursor.u8()? {
        0 => FilesystemLogicalHandleOutputSource::Created,
        1 => FilesystemLogicalHandleOutputSource::Duplicated(FilesystemLogicalHandleIdentity::new(
            cursor.u64()?,
        )?),
        2 => FilesystemLogicalHandleOutputSource::Borrowed(FilesystemLogicalHandleIdentity::new(
            cursor.u64()?,
        )?),
        _ => return None,
    };
    Some(FilesystemLogicalHandleOutput {
        kind,
        identity,
        source,
    })
}

pub fn encode_grant_access(access: FilesystemGrantAccess) -> u8 {
    match access {
        FilesystemGrantAccess::Read => 0,
        FilesystemGrantAccess::Write => 1,
    }
}

pub fn recover_grant_access(tag: u8) -> Option<FilesystemGrantAccess> {
    match tag {
        0 => Some(FilesystemGrantAccess::Read),
        1 => Some(FilesystemGrantAccess::Write),
        _ => None,
    }
}

pub fn encode_grant_refusal(refusal: &FilesystemGrantRefusal, out: &mut Vec<u8>) {
    out.push(refusal.operand_ordinal());
    out.push(encode_grant_access(refusal.access()));
    out.push(match refusal.reason() {
        FilesystemGrantRefusalReason::Unresolvable => 0,
        FilesystemGrantRefusalReason::OutsideGrantedRoots => 1,
        FilesystemGrantRefusalReason::UnrepresentableRootedPath => 2,
        FilesystemGrantRefusalReason::ObservationEvidenceLimitExceeded => 3,
    });
}

pub fn recover_grant_refusal(cursor: &mut ObservationCursor<'_>) -> Option<FilesystemGrantRefusal> {
    let operand_ordinal = cursor.u8()?;
    let access = recover_grant_access(cursor.u8()?)?;
    let reason = match cursor.u8()? {
        0 => FilesystemGrantRefusalReason::Unresolvable,
        1 => FilesystemGrantRefusalReason::OutsideGrantedRoots,
        2 => FilesystemGrantRefusalReason::UnrepresentableRootedPath,
        3 => FilesystemGrantRefusalReason::ObservationEvidenceLimitExceeded,
        _ => return None,
    };
    Some(FilesystemGrantRefusal {
        operand_ordinal,
        access,
        reason,
    })
}
