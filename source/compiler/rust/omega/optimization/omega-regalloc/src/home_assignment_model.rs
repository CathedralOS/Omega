use omega_register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use omega_terminal_selected_instructions::TerminalVirtualRegisterId;
use psi_core::MachineId;

use crate::home_assignment_identity::encode_terminal_register_home_content;
use crate::{
    TerminalAllocationLegalityIdentity, TerminalLiveRangeIdentity, terminal_register_home_identity,
};

const REGISTER_HOME_MAGIC: &[u8; 8] = b"OMGRAH\0\0";
const REGISTER_HOME_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalRegisterHomeIdentity(pub(crate) [u8; 32]);

impl TerminalRegisterHomeIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Bounded, deterministic physical homes for one transition-free legality
/// plan. The artifact grants no spill, frame, instruction-emission, or
/// publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRegisterHomePlan {
    pub legality: TerminalAllocationLegalityIdentity,
    pub ranges: TerminalLiveRangeIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub functions: Vec<TerminalFunctionRegisterHomes>,
}

impl TerminalRegisterHomePlan {
    /// Canonical artifact representation. Decoding this representation does
    /// not grant allocation or emission authority; callers must still use the
    /// independent register-home validator with the retained input artifacts.
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_terminal_register_home_content(self);
        let identity = terminal_register_home_identity(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(REGISTER_HOME_MAGIC);
        encoded.extend_from_slice(&REGISTER_HOME_VERSION.to_le_bytes());
        encoded.extend_from_slice(&identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, TerminalRegisterHomeDecodeError> {
        let mut cursor = RegisterHomeCursor::new(encoded);
        if cursor.take(REGISTER_HOME_MAGIC.len())? != REGISTER_HOME_MAGIC {
            return Err(TerminalRegisterHomeDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != REGISTER_HOME_VERSION {
            return Err(TerminalRegisterHomeDecodeError::UnsupportedVersion(version));
        }
        let identity = TerminalRegisterHomeIdentity::from_bytes(cursor.array()?);
        let legality = TerminalAllocationLegalityIdentity::from_bytes(cursor.array()?);
        let ranges = TerminalLiveRangeIdentity::from_bytes(cursor.array()?);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let function_count = cursor.length()?;
        let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
        for _ in 0..function_count {
            let raw_machine = u64::from_le_bytes(cursor.array()?);
            let machine = MachineId::new(raw_machine).ok_or(
                TerminalRegisterHomeDecodeError::InvalidMachineId(raw_machine),
            )?;
            let assignment_count = cursor.length()?;
            let mut assignments = Vec::with_capacity(assignment_count.min(cursor.remaining()));
            for _ in 0..assignment_count {
                assignments.push(TerminalVirtualRegisterHome {
                    virtual_register: TerminalVirtualRegisterId(u32::from_le_bytes(
                        cursor.array()?,
                    )),
                    class: RegisterClassId(u16::from_le_bytes(cursor.array()?)),
                    view: RegisterViewId(u16::from_le_bytes(cursor.array()?)),
                });
            }
            functions.push(TerminalFunctionRegisterHomes {
                machine,
                assignments,
            });
        }
        if cursor.remaining() != 0 {
            return Err(TerminalRegisterHomeDecodeError::TrailingBytes);
        }
        let plan = Self {
            legality,
            ranges,
            register_environment,
            functions,
        };
        if terminal_register_home_identity(&plan) != identity {
            return Err(TerminalRegisterHomeDecodeError::IdentityMismatch);
        }
        Ok(plan)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFunctionRegisterHomes {
    pub machine: MachineId,
    pub assignments: Vec<TerminalVirtualRegisterHome>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalVirtualRegisterHome {
    pub virtual_register: TerminalVirtualRegisterId,
    pub class: RegisterClassId,
    pub view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalRegisterHomeValidationReceipt {
    pub(crate) identity: TerminalRegisterHomeIdentity,
    pub(crate) legality: TerminalAllocationLegalityIdentity,
    pub(crate) ranges: TerminalLiveRangeIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) function_count: usize,
    pub(crate) assignment_count: usize,
}

impl TerminalRegisterHomeValidationReceipt {
    pub const fn identity(self) -> TerminalRegisterHomeIdentity {
        self.identity
    }
    pub const fn legality(self) -> TerminalAllocationLegalityIdentity {
        self.legality
    }
    pub const fn ranges(self) -> TerminalLiveRangeIdentity {
        self.ranges
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn assignment_count(self) -> usize {
        self.assignment_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTerminalRegisterHomes {
    pub(crate) plan: TerminalRegisterHomePlan,
    pub(crate) receipt: TerminalRegisterHomeValidationReceipt,
}

impl ValidatedTerminalRegisterHomes {
    pub const fn plan(&self) -> &TerminalRegisterHomePlan {
        &self.plan
    }
    pub const fn receipt(&self) -> TerminalRegisterHomeValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalRegisterHomeError {
    RootMismatch,
    FunctionMismatch {
        function: usize,
    },
    VirtualRegisterMismatch {
        function: usize,
        register: u32,
    },
    UnresolvedEntryTransitions {
        function: usize,
        register: u32,
        count: usize,
    },
    NoLivePoints {
        function: usize,
        register: u32,
    },
    IntervalOverflow {
        function: usize,
        register: u32,
    },
    NoCommonCandidate {
        function: usize,
        register: u32,
    },
    UnknownOrIncompatibleView {
        function: usize,
        register: u32,
        view: u16,
    },
    NoCompatibleHome {
        function: usize,
        register: u32,
    },
    NonCanonicalAssignments {
        function: usize,
    },
}

impl std::fmt::Display for TerminalRegisterHomeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Terminal register-home assignment failed: {self:?}"
        )
    }
}

impl std::error::Error for TerminalRegisterHomeError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalRegisterHomeDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidMachineId(u64),
    LengthOverflow,
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for TerminalRegisterHomeDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid Terminal register-home encoding: {self:?}"
        )
    }
}

impl std::error::Error for TerminalRegisterHomeDecodeError {}

struct RegisterHomeCursor<'encoded> {
    encoded: &'encoded [u8],
    offset: usize,
}

impl<'encoded> RegisterHomeCursor<'encoded> {
    const fn new(encoded: &'encoded [u8]) -> Self {
        Self { encoded, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'encoded [u8], TerminalRegisterHomeDecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(TerminalRegisterHomeDecodeError::Truncated)?;
        let value = self
            .encoded
            .get(self.offset..end)
            .ok_or(TerminalRegisterHomeDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], TerminalRegisterHomeDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| TerminalRegisterHomeDecodeError::Truncated)
    }

    fn length(&mut self) -> Result<usize, TerminalRegisterHomeDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| TerminalRegisterHomeDecodeError::LengthOverflow)
    }

    fn remaining(&self) -> usize {
        self.encoded.len() - self.offset
    }
}
