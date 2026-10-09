//! Durable allocator availability data; not validation or allocation authority.

mod identity;
pub use identity::allocator_availability_identity;

use register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterViewId,
    TargetRegisterEnvironmentIdentity,
};

use self::identity::encode_terminal_allocator_availability_content;

const ALLOCATOR_AVAILABILITY_MAGIC: &[u8; 8] = b"OMGAVA\0\0";
const ALLOCATOR_AVAILABILITY_VERSION: u32 = 1;

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
        let mut encoded = Vec::with_capacity(44 + content.len());
        crate::codec_bytes::encode_header(
            &mut encoded,
            ALLOCATOR_AVAILABILITY_MAGIC,
            ALLOCATOR_AVAILABILITY_VERSION,
        );
        encoded.extend_from_slice(&identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, AllocatorAvailabilityDecodeError> {
        let mut cursor = crate::cursor::Cursor::new(encoded);
        crate::codec_bytes::decode_header(
            &mut cursor,
            ALLOCATOR_AVAILABILITY_MAGIC,
            ALLOCATOR_AVAILABILITY_VERSION,
            || AllocatorAvailabilityDecodeError::WrongMagic,
            AllocatorAvailabilityDecodeError::UnsupportedVersion,
        )?;
        let identity = AllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
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

impl From<crate::cursor::Truncated> for AllocatorAvailabilityDecodeError {
    fn from(_: crate::cursor::Truncated) -> Self {
        Self::Truncated
    }
}

impl From<crate::cursor::TakeError> for AllocatorAvailabilityDecodeError {
    fn from(error: crate::cursor::TakeError) -> Self {
        match error {
            crate::cursor::TakeError::Truncated => Self::Truncated,
            crate::cursor::TakeError::LengthOverflow => Self::LengthOverflow,
        }
    }
}
