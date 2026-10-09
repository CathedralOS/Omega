//! Canonical home encoding. Successful decode is not allocation admission.

use super::identity::encode_terminal_register_home_content;
use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FunctionRegisterHomes,
    RegisterHomeIdentity, RegisterHomePlan, VirtualRegisterHome, register_home_identity,
};
use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{LiveRangeIdentity, VirtualRegisterId};
use semantic_vocabulary::MachineId;

const REGISTER_HOME_MAGIC: &[u8; 8] = b"OMGRAH\0\0";
const REGISTER_HOME_VERSION: u32 = 7;

impl RegisterHomePlan {
    /// Canonical artifact representation. Decoding this representation does
    /// not grant allocation or emission authority; callers must still use the
    /// independent register-home validator with the retained input artifacts.
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_terminal_register_home_content(self);
        let identity = register_home_identity(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        crate::codec_bytes::encode_header(&mut encoded, REGISTER_HOME_MAGIC, REGISTER_HOME_VERSION);
        encoded.extend_from_slice(&identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, RegisterHomeDecodeError> {
        let mut cursor = crate::cursor::Cursor::new(encoded);
        crate::codec_bytes::decode_header(
            &mut cursor,
            REGISTER_HOME_MAGIC,
            REGISTER_HOME_VERSION,
            || RegisterHomeDecodeError::WrongMagic,
            RegisterHomeDecodeError::UnsupportedVersion,
        )?;
        let identity = RegisterHomeIdentity::from_bytes(cursor.array()?);
        let legality = AllocationLegalityIdentity::from_bytes(cursor.array()?);
        let ranges = LiveRangeIdentity::from_bytes(cursor.array()?);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let allocator_availability = AllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
        let function_count = cursor.length()?;
        let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
        for _ in 0..function_count {
            let raw_machine = cursor.u64()?;
            let machine = MachineId::new(raw_machine)
                .ok_or(RegisterHomeDecodeError::InvalidMachineId(raw_machine))?;
            let assignment_count = cursor.length()?;
            let mut assignments = Vec::with_capacity(assignment_count.min(cursor.remaining()));
            for _ in 0..assignment_count {
                assignments.push(VirtualRegisterHome {
                    virtual_register: VirtualRegisterId(cursor.u32()?),
                    class: RegisterClassId(cursor.u16()?),
                    view: RegisterViewId(cursor.u16()?),
                });
            }
            functions.push(FunctionRegisterHomes {
                machine,
                assignments,
            });
        }
        if cursor.remaining() != 0 {
            return Err(RegisterHomeDecodeError::TrailingBytes);
        }
        let plan = Self {
            legality,
            ranges,
            register_environment,
            allocator_availability,
            functions,
        };
        if register_home_identity(&plan) != identity {
            return Err(RegisterHomeDecodeError::IdentityMismatch);
        }
        Ok(plan)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterHomeDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidMachineId(u64),
    LengthOverflow,
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for RegisterHomeDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid Terminal register-home encoding: {self:?}"
        )
    }
}

impl std::error::Error for RegisterHomeDecodeError {}

impl From<crate::cursor::Truncated> for RegisterHomeDecodeError {
    fn from(_: crate::cursor::Truncated) -> Self {
        Self::Truncated
    }
}

impl From<crate::cursor::TakeError> for RegisterHomeDecodeError {
    fn from(error: crate::cursor::TakeError) -> Self {
        match error {
            crate::cursor::TakeError::Truncated => Self::Truncated,
            crate::cursor::TakeError::LengthOverflow => Self::LengthOverflow,
        }
    }
}
