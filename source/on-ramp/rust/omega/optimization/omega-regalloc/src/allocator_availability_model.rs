use omega_register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterViewId,
    TargetRegisterEnvironmentIdentity,
};

use crate::allocator_availability_identity::encode_terminal_allocator_availability_content;

const ALLOCATOR_AVAILABILITY_MAGIC: &[u8; 8] = b"OMGAVA\0\0";
const ALLOCATOR_AVAILABILITY_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalAllocatorAvailabilityIdentity(pub(crate) [u8; 32]);

impl TerminalAllocatorAvailabilityIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Exact named policy controlling only unconstrained allocator candidates.
/// Fixed ABI and instruction-operand requirements remain authoritative even
/// when their views are absent from an explicit allowlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalAllocatorAvailabilityPolicy {
    AllEnvironmentAllocatableViewsV1,
    ExplicitUnconstrainedViewAllowlistV1 { views: Vec<RegisterViewId> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAllocatorAvailabilityPlan {
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub physical: PhysicalRegisterModelIdentity,
    pub policy: TerminalAllocatorAvailabilityPolicy,
    /// One class-ID-sorted row for every physical register class. Each view
    /// list is sorted and contains only environment-allocatable class members.
    pub classes: Vec<TerminalRegisterClassAvailability>,
}

impl TerminalAllocatorAvailabilityPlan {
    /// Canonical transport only. The decoded plan remains untrusted until its
    /// exact environment and derivation are independently validated.
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_terminal_allocator_availability_content(self);
        let identity = crate::terminal_allocator_availability_identity(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(ALLOCATOR_AVAILABILITY_MAGIC);
        encoded.extend_from_slice(&ALLOCATOR_AVAILABILITY_VERSION.to_le_bytes());
        encoded.extend_from_slice(&identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, TerminalAllocatorAvailabilityDecodeError> {
        let mut cursor = AllocatorAvailabilityCursor::new(encoded);
        if cursor.take(ALLOCATOR_AVAILABILITY_MAGIC.len())? != ALLOCATOR_AVAILABILITY_MAGIC {
            return Err(TerminalAllocatorAvailabilityDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != ALLOCATOR_AVAILABILITY_VERSION {
            return Err(TerminalAllocatorAvailabilityDecodeError::UnsupportedVersion(version));
        }
        let identity = TerminalAllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let physical = PhysicalRegisterModelIdentity::from_bytes(cursor.array()?);
        let policy = match cursor.byte()? {
            0 => TerminalAllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1,
            1 => {
                let count = cursor.length()?;
                let mut views = Vec::with_capacity(count.min(cursor.remaining() / 2));
                for _ in 0..count {
                    views.push(RegisterViewId(u16::from_le_bytes(cursor.array()?)));
                }
                TerminalAllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 { views }
            }
            tag => return Err(TerminalAllocatorAvailabilityDecodeError::UnknownPolicy(tag)),
        };
        let class_count = cursor.length()?;
        let mut classes = Vec::with_capacity(class_count.min(cursor.remaining() / 2));
        for _ in 0..class_count {
            let class = RegisterClassId(u16::from_le_bytes(cursor.array()?));
            let view_count = cursor.length()?;
            let mut unconstrained_views =
                Vec::with_capacity(view_count.min(cursor.remaining() / 2));
            for _ in 0..view_count {
                unconstrained_views.push(RegisterViewId(u16::from_le_bytes(cursor.array()?)));
            }
            classes.push(TerminalRegisterClassAvailability {
                class,
                unconstrained_views,
            });
        }
        if cursor.remaining() != 0 {
            return Err(TerminalAllocatorAvailabilityDecodeError::TrailingBytes);
        }
        let plan = Self {
            register_environment,
            physical,
            policy,
            classes,
        };
        if crate::terminal_allocator_availability_identity(&plan) != identity {
            return Err(TerminalAllocatorAvailabilityDecodeError::IdentityMismatch);
        }
        Ok(plan)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRegisterClassAvailability {
    pub class: RegisterClassId,
    pub unconstrained_views: Vec<RegisterViewId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalAllocatorAvailabilityValidationReceipt {
    pub(crate) identity: TerminalAllocatorAvailabilityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) physical: PhysicalRegisterModelIdentity,
    pub(crate) class_count: usize,
    pub(crate) unconstrained_view_count: usize,
}

impl TerminalAllocatorAvailabilityValidationReceipt {
    pub const fn identity(self) -> TerminalAllocatorAvailabilityIdentity {
        self.identity
    }

    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }

    pub const fn physical(self) -> PhysicalRegisterModelIdentity {
        self.physical
    }

    pub const fn class_count(self) -> usize {
        self.class_count
    }

    pub const fn unconstrained_view_count(self) -> usize {
        self.unconstrained_view_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTerminalAllocatorAvailability {
    pub(crate) plan: TerminalAllocatorAvailabilityPlan,
    pub(crate) receipt: TerminalAllocatorAvailabilityValidationReceipt,
}

impl ValidatedTerminalAllocatorAvailability {
    pub const fn plan(&self) -> &TerminalAllocatorAvailabilityPlan {
        &self.plan
    }

    pub const fn receipt(&self) -> TerminalAllocatorAvailabilityValidationReceipt {
        self.receipt
    }

    pub(crate) fn unconstrained_views(&self, class: RegisterClassId) -> Option<&[RegisterViewId]> {
        self.plan
            .classes
            .iter()
            .find(|row| row.class == class)
            .map(|row| row.unconstrained_views.as_slice())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalAllocatorAvailabilityError {
    RootMismatch,
    NonCanonicalAllowlist,
    UnknownView { view: u16 },
    ViewNotEnvironmentAllocatable { view: u16 },
    NonCanonicalPlan,
    PlanMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalAllocatorAvailabilityDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownPolicy(u8),
    LengthOverflow,
    TrailingBytes,
    IdentityMismatch,
}

impl std::fmt::Display for TerminalAllocatorAvailabilityDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid terminal allocator availability: {self:?}"
        )
    }
}

impl std::error::Error for TerminalAllocatorAvailabilityDecodeError {}

struct AllocatorAvailabilityCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> AllocatorAvailabilityCursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], TerminalAllocatorAvailabilityDecodeError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(TerminalAllocatorAvailabilityDecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(TerminalAllocatorAvailabilityDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], TerminalAllocatorAvailabilityDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| TerminalAllocatorAvailabilityDecodeError::Truncated)
    }

    fn byte(&mut self) -> Result<u8, TerminalAllocatorAvailabilityDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    fn length(&mut self) -> Result<usize, TerminalAllocatorAvailabilityDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| TerminalAllocatorAvailabilityDecodeError::LengthOverflow)
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }
}

impl std::fmt::Display for TerminalAllocatorAvailabilityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "terminal allocator-availability derivation failed: {self:?}"
        )
    }
}

impl std::error::Error for TerminalAllocatorAvailabilityError {}
