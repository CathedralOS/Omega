use omega_register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use omega_selected_instructions::VirtualRegisterId;
use psi_core::MachineId;

use crate::allocation::home_assignment::identity::encode_terminal_register_home_content;
use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, LiveRangeIdentity,
    register_home_identity,
};

const REGISTER_HOME_MAGIC: &[u8; 8] = b"OMGRAH\0\0";
const REGISTER_HOME_VERSION: u32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegisterHomeIdentity(pub(crate) [u8; 32]);

impl RegisterHomeIdentity {
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
pub struct RegisterHomePlan {
    pub legality: AllocationLegalityIdentity,
    pub ranges: LiveRangeIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub functions: Vec<FunctionRegisterHomes>,
    pub structural_unit_functions: Vec<FunctionRegisterHomes>,
}

impl RegisterHomePlan {
    /// Canonical artifact representation. Decoding this representation does
    /// not grant allocation or emission authority; callers must still use the
    /// independent register-home validator with the retained input artifacts.
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_terminal_register_home_content(self);
        let identity = register_home_identity(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(REGISTER_HOME_MAGIC);
        encoded.extend_from_slice(&REGISTER_HOME_VERSION.to_le_bytes());
        encoded.extend_from_slice(&identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, RegisterHomeDecodeError> {
        let mut cursor = RegisterHomeCursor::new(encoded);
        if cursor.take(REGISTER_HOME_MAGIC.len())? != REGISTER_HOME_MAGIC {
            return Err(RegisterHomeDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != REGISTER_HOME_VERSION {
            return Err(RegisterHomeDecodeError::UnsupportedVersion(version));
        }
        let identity = RegisterHomeIdentity::from_bytes(cursor.array()?);
        let legality = AllocationLegalityIdentity::from_bytes(cursor.array()?);
        let ranges = LiveRangeIdentity::from_bytes(cursor.array()?);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let allocator_availability = AllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
        let function_count = cursor.length()?;
        let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
        for _ in 0..function_count {
            let raw_machine = u64::from_le_bytes(cursor.array()?);
            let machine = MachineId::new(raw_machine)
                .ok_or(RegisterHomeDecodeError::InvalidMachineId(raw_machine))?;
            let assignment_count = cursor.length()?;
            let mut assignments = Vec::with_capacity(assignment_count.min(cursor.remaining()));
            for _ in 0..assignment_count {
                assignments.push(VirtualRegisterHome {
                    virtual_register: VirtualRegisterId(u32::from_le_bytes(cursor.array()?)),
                    class: RegisterClassId(u16::from_le_bytes(cursor.array()?)),
                    view: RegisterViewId(u16::from_le_bytes(cursor.array()?)),
                });
            }
            functions.push(FunctionRegisterHomes {
                machine,
                assignments,
            });
        }
        let structural_unit_function_count = cursor.length()?;
        let mut structural_unit_functions =
            Vec::with_capacity(structural_unit_function_count.min(cursor.remaining()));
        for _ in 0..structural_unit_function_count {
            let raw_machine = u64::from_le_bytes(cursor.array()?);
            let machine = MachineId::new(raw_machine)
                .ok_or(RegisterHomeDecodeError::InvalidMachineId(raw_machine))?;
            let assignment_count = cursor.length()?;
            let mut assignments = Vec::with_capacity(assignment_count.min(cursor.remaining()));
            for _ in 0..assignment_count {
                assignments.push(VirtualRegisterHome {
                    virtual_register: VirtualRegisterId(u32::from_le_bytes(cursor.array()?)),
                    class: RegisterClassId(u16::from_le_bytes(cursor.array()?)),
                    view: RegisterViewId(u16::from_le_bytes(cursor.array()?)),
                });
            }
            structural_unit_functions.push(FunctionRegisterHomes {
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
            structural_unit_functions,
        };
        if register_home_identity(&plan) != identity {
            return Err(RegisterHomeDecodeError::IdentityMismatch);
        }
        Ok(plan)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionRegisterHomes {
    pub machine: MachineId,
    pub assignments: Vec<VirtualRegisterHome>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualRegisterHome {
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterHomeValidationReceipt {
    pub(crate) identity: RegisterHomeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) function_count: usize,
    pub(crate) structural_unit_function_count: usize,
    pub(crate) assignment_count: usize,
    pub(crate) tied_pair_count: usize,
    pub(crate) tied_component_count: usize,
    pub(crate) early_clobber_count: usize,
}

impl RegisterHomeValidationReceipt {
    pub const fn identity(self) -> RegisterHomeIdentity {
        self.identity
    }
    pub const fn legality(self) -> AllocationLegalityIdentity {
        self.legality
    }
    pub const fn ranges(self) -> LiveRangeIdentity {
        self.ranges
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn allocator_availability(self) -> AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn structural_unit_function_count(self) -> usize {
        self.structural_unit_function_count
    }
    pub const fn assignment_count(self) -> usize {
        self.assignment_count
    }
    pub const fn tied_pair_count(self) -> usize {
        self.tied_pair_count
    }
    pub const fn tied_component_count(self) -> usize {
        self.tied_component_count
    }
    pub const fn early_clobber_count(self) -> usize {
        self.early_clobber_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRegisterHomes {
    pub(crate) plan: RegisterHomePlan,
    pub(crate) receipt: RegisterHomeValidationReceipt,
}

impl ValidatedRegisterHomes {
    pub const fn plan(&self) -> &RegisterHomePlan {
        &self.plan
    }
    pub const fn receipt(&self) -> RegisterHomeValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterHomeError {
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
    UnsupportedTiedTopology {
        function: usize,
        instruction: u32,
    },
    TiedRegistersInterfere {
        function: usize,
        lower: u32,
        higher: u32,
    },
    NoCommonTiedComponent {
        function: usize,
        leader: u32,
        member_count: usize,
    },
    NonCanonicalAssignments {
        function: usize,
    },
}

impl std::fmt::Display for RegisterHomeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Terminal register-home assignment failed: {self:?}"
        )
    }
}

impl std::error::Error for RegisterHomeError {}

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

struct RegisterHomeCursor<'encoded> {
    encoded: &'encoded [u8],
    offset: usize,
}

impl<'encoded> RegisterHomeCursor<'encoded> {
    const fn new(encoded: &'encoded [u8]) -> Self {
        Self { encoded, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'encoded [u8], RegisterHomeDecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(RegisterHomeDecodeError::Truncated)?;
        let value = self
            .encoded
            .get(self.offset..end)
            .ok_or(RegisterHomeDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], RegisterHomeDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| RegisterHomeDecodeError::Truncated)
    }

    fn length(&mut self) -> Result<usize, RegisterHomeDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| RegisterHomeDecodeError::LengthOverflow)
    }

    fn remaining(&self) -> usize {
        self.encoded.len() - self.offset
    }
}
