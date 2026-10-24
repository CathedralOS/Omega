//! Canonical home encoding. Successful decode is not allocation admission.

use super::identity::encode_terminal_register_home_content;
use crate::codec_bytes::Envelope;
use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FunctionRegisterHomes,
    RegisterHomeIdentity, RegisterHomePlan, VirtualRegisterHome, register_home_identity,
};
use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{LiveRangeIdentity, VirtualRegisterId};
use semantic_vocabulary::MachineId;

const REGISTER_HOME_ENVELOPE: Envelope = Envelope::new(b"OMGRAH\0\0", 7);

impl RegisterHomePlan {
    /// Canonical artifact representation. Decoding this representation does
    /// not grant allocation or emission authority; callers must still use the
    /// independent register-home validator with the retained input artifacts.
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_terminal_register_home_content(self);
        let identity = register_home_identity(self);
        REGISTER_HOME_ENVELOPE.frame(identity.bytes(), &content)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, RegisterHomeDecodeError> {
        let (mut cursor, identity) = REGISTER_HOME_ENVELOPE.open(
            encoded,
            || RegisterHomeDecodeError::WrongMagic,
            RegisterHomeDecodeError::UnsupportedVersion,
        )?;
        let identity = RegisterHomeIdentity::from_bytes(identity);
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

optimization_core::truncated_from_adapter!(RegisterHomeDecodeError);

optimization_core::take_error_from_adapter!(RegisterHomeDecodeError);
