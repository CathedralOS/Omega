//! Durable allocator availability data; not validation or allocation authority.

mod identity;
pub use identity::allocator_availability_identity;

use register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterViewId,
    TargetRegisterEnvironmentIdentity,
};

use self::identity::encode_terminal_allocator_availability_content;
use crate::codec_bytes::Envelope;

const ALLOCATOR_AVAILABILITY_ENVELOPE: Envelope = Envelope::new(b"OMGAVA\0\0", 1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AllocatorAvailabilityIdentity(pub(crate) [u8; 32]);

impl AllocatorAvailabilityIdentity {
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
pub enum AllocatorAvailabilityPolicy {
    AllEnvironmentAllocatableViewsV1,
    ExplicitUnconstrainedViewAllowlistV1 { views: Vec<RegisterViewId> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocatorAvailabilityPlan {
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub physical: PhysicalRegisterModelIdentity,
    pub policy: AllocatorAvailabilityPolicy,
    /// One class-ID-sorted row for every physical register class. Each view
    /// list is sorted and contains only environment-allocatable class members.
    pub classes: Vec<RegisterClassAvailability>,
}

impl AllocatorAvailabilityPlan {
    /// Canonical transport only. The decoded plan remains untrusted until its
    /// exact environment and derivation are independently validated.
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_terminal_allocator_availability_content(self);
        let identity = crate::allocator_availability_identity(self);
        ALLOCATOR_AVAILABILITY_ENVELOPE.frame(identity.bytes(), &content)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, AllocatorAvailabilityDecodeError> {
        let (mut cursor, identity) = ALLOCATOR_AVAILABILITY_ENVELOPE.open(
            encoded,
            || AllocatorAvailabilityDecodeError::WrongMagic,
            AllocatorAvailabilityDecodeError::UnsupportedVersion,
        )?;
        let identity = AllocatorAvailabilityIdentity::from_bytes(identity);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let physical = PhysicalRegisterModelIdentity::from_bytes(cursor.array()?);
        let policy = match cursor.byte()? {
            0 => AllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1,
            1 => {
                let count = cursor.length()?;
                let mut views = Vec::with_capacity(count.min(cursor.remaining() / 2));
                for _ in 0..count {
                    views.push(RegisterViewId(cursor.u16()?));
                }
                AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 { views }
            }
            tag => return Err(AllocatorAvailabilityDecodeError::UnknownPolicy(tag)),
        };
        let class_count = cursor.length()?;
        let mut classes = Vec::with_capacity(class_count.min(cursor.remaining() / 2));
        for _ in 0..class_count {
            let class = RegisterClassId(cursor.u16()?);
            let view_count = cursor.length()?;
            let mut unconstrained_views =
                Vec::with_capacity(view_count.min(cursor.remaining() / 2));
            for _ in 0..view_count {
                unconstrained_views.push(RegisterViewId(cursor.u16()?));
            }
            classes.push(RegisterClassAvailability {
                class,
                unconstrained_views,
            });
        }
        if cursor.remaining() != 0 {
            return Err(AllocatorAvailabilityDecodeError::TrailingBytes);
        }
        let plan = Self {
            register_environment,
            physical,
            policy,
            classes,
        };
        if crate::allocator_availability_identity(&plan) != identity {
            return Err(AllocatorAvailabilityDecodeError::IdentityMismatch);
        }
        Ok(plan)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterClassAvailability {
    pub class: RegisterClassId,
    pub unconstrained_views: Vec<RegisterViewId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocatorAvailabilityDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownPolicy(u8),
    LengthOverflow,
    TrailingBytes,
    IdentityMismatch,
}

impl std::fmt::Display for AllocatorAvailabilityDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid terminal allocator availability: {self:?}"
        )
    }
}

impl std::error::Error for AllocatorAvailabilityDecodeError {}

optimization_core::truncated_from_adapter!(AllocatorAvailabilityDecodeError);

optimization_core::take_error_from_adapter!(AllocatorAvailabilityDecodeError);
