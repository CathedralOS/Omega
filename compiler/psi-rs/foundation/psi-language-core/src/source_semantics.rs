/// First-class usage multiplicity. `[copy]` maps to `Unrestricted`, ordinary
/// data defaults to `Affine`, and `[linear]` maps to `Linear`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Multiplicity {
    /// Freely duplicable and discardable (`[copy]`).
    Unrestricted,
    /// Use at most once; silent discard is legal (ordinary data).
    #[default]
    Affine,
    /// Use exactly once; discard is an error (`[linear]`).
    Linear,
}

/// Whether one authored binding occurrence contributes runtime representation.
///
/// Relevance is deliberately independent of the binding's type and
/// multiplicity. An erased binding remains part of semantic identity and the
/// proof calculus even though later lowering omits its runtime representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum BindingRelevance {
    /// An ordinary binding with runtime representation.
    #[default]
    Relevant,
    /// A proof-side binding authored with `[erased]`.
    Erased,
}

impl BindingRelevance {
    pub const fn is_erased(self) -> bool {
        matches!(self, Self::Erased)
    }
}

/// How a data declaration obtains its representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataSupplyMode {
    #[default]
    CheckedShape,
    BoundaryOpaque,
}

/// Whether a live value may cross a suspension point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CarrySuspension {
    #[default]
    Forbidden,
    Allowed,
}

/// CPU affinity of a live value relative to the CPU recorded at mint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CarryCpu {
    #[default]
    Origin,
    Any,
}

/// Host-thread affinity of a live value relative to the thread recorded at mint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CarryHostThread {
    #[default]
    Origin,
    Any,
}

/// Whether a live value may move to a different storage address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CarryAddress {
    #[default]
    Stable,
    Movable,
}

/// Normalized four-axis carry policy. The default is deliberately strict so
/// missing evidence fails closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CarryPolicy {
    pub suspension: CarrySuspension,
    pub cpu: CarryCpu,
    pub host_thread: CarryHostThread,
    pub address: CarryAddress,
}

impl CarryPolicy {
    pub const STRICT: Self = Self {
        suspension: CarrySuspension::Forbidden,
        cpu: CarryCpu::Origin,
        host_thread: CarryHostThread::Origin,
        address: CarryAddress::Stable,
    };

    pub const PERMISSIVE: Self = Self {
        suspension: CarrySuspension::Allowed,
        cpu: CarryCpu::Any,
        host_thread: CarryHostThread::Any,
        address: CarryAddress::Movable,
    };

    /// True when `self` permits every transition promised by `required`.
    pub const fn permits(self, required: Self) -> bool {
        (matches!(required.suspension, CarrySuspension::Forbidden)
            || matches!(self.suspension, CarrySuspension::Allowed))
            && (matches!(required.cpu, CarryCpu::Origin) || matches!(self.cpu, CarryCpu::Any))
            && (matches!(required.host_thread, CarryHostThread::Origin)
                || matches!(self.host_thread, CarryHostThread::Any))
            && (matches!(required.address, CarryAddress::Stable)
                || matches!(self.address, CarryAddress::Movable))
    }

    /// Structural composition for aggregate fields: each axis takes the most
    /// restrictive demand contributed by either live field.
    pub const fn intersect(self, other: Self) -> Self {
        Self {
            suspension: if matches!(self.suspension, CarrySuspension::Allowed)
                && matches!(other.suspension, CarrySuspension::Allowed)
            {
                CarrySuspension::Allowed
            } else {
                CarrySuspension::Forbidden
            },
            cpu: if matches!(self.cpu, CarryCpu::Any) && matches!(other.cpu, CarryCpu::Any) {
                CarryCpu::Any
            } else {
                CarryCpu::Origin
            },
            host_thread: if matches!(self.host_thread, CarryHostThread::Any)
                && matches!(other.host_thread, CarryHostThread::Any)
            {
                CarryHostThread::Any
            } else {
                CarryHostThread::Origin
            },
            address: if matches!(self.address, CarryAddress::Movable)
                && matches!(other.address, CarryAddress::Movable)
            {
                CarryAddress::Movable
            } else {
                CarryAddress::Stable
            },
        }
    }
}

impl std::fmt::Display for CarryPolicy {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let suspension = match self.suspension {
            CarrySuspension::Forbidden => "forbidden",
            CarrySuspension::Allowed => "allowed",
        };
        let cpu = match self.cpu {
            CarryCpu::Origin => "same",
            CarryCpu::Any => "any",
        };
        let thread = match self.host_thread {
            CarryHostThread::Origin => "same",
            CarryHostThread::Any => "any",
        };
        let address = match self.address {
            CarryAddress::Stable => "stable",
            CarryAddress::Movable => "movable",
        };
        write!(
            formatter,
            "carry(suspension: {suspension}, cpu: {cpu}, thread: {thread}, address: {address})"
        )
    }
}

/// One compiler-owned positive permission that relaxes a resource claim's
/// born-strict carry policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CarryPermission {
    AcrossSuspend,
    AnyCpu,
    AnyThread,
    MovableAddress,
}

impl CarryPermission {
    pub const ALL: [Self; 4] = [
        Self::AcrossSuspend,
        Self::AnyCpu,
        Self::AnyThread,
        Self::MovableAddress,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::AcrossSuspend => "Carry::AcrossSuspend",
            Self::AnyCpu => "Carry::AnyCpu",
            Self::AnyThread => "Carry::AnyThread",
            Self::MovableAddress => "Carry::MovableAddress",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Carry::AcrossSuspend" => Some(Self::AcrossSuspend),
            "Carry::AnyCpu" => Some(Self::AnyCpu),
            "Carry::AnyThread" => Some(Self::AnyThread),
            "Carry::MovableAddress" => Some(Self::MovableAddress),
            _ => None,
        }
    }

    /// Apply this positive permission while leaving the independent axes
    /// untouched.
    pub const fn relax(self, mut policy: CarryPolicy) -> CarryPolicy {
        match self {
            Self::AcrossSuspend => policy.suspension = CarrySuspension::Allowed,
            Self::AnyCpu => policy.cpu = CarryCpu::Any,
            Self::AnyThread => policy.host_thread = CarryHostThread::Any,
            Self::MovableAddress => policy.address = CarryAddress::Movable,
        }
        policy
    }
}

impl std::fmt::Display for CarryPermission {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.name().fmt(formatter)
    }
}

/// Exact call-site acknowledgement of the statically known operational
/// envelope. This is diagnostic/audit metadata, not contract identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CallOperationalAcknowledgement {
    pub origin: CallOperationalAcknowledgementOrigin,
    pub acknowledges_suspend: bool,
    pub acknowledges_block: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CallOperationalAcknowledgementOrigin {
    #[default]
    Source,
    CompilerSynthesized,
}

/// Whether a declared domain carries an explicit predicate body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DomainPredicateBody {
    #[default]
    Bodyless,
    Present,
}

/// Source-level access carried by a reference type.
///
/// `WriteOnly` is exclusive like `Mutable`, but grants no observation of the
/// referent. Keeping this as one closed axis prevents a reference from being
/// represented as independently mutable and write-only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReferenceAccess {
    #[default]
    Shared,
    Mutable,
    WriteOnly,
}

impl ReferenceAccess {
    pub const fn is_exclusive(self) -> bool {
        matches!(self, Self::Mutable | Self::WriteOnly)
    }

    pub const fn is_readable(self) -> bool {
        !matches!(self, Self::WriteOnly)
    }
}

/// One closed compiler-owned classification attached explicitly to a domain.
///
/// This is not an ordinary trait conformance: each case grants fixed language
/// semantics that packages cannot extend by declaring another same-shaped
/// trait. The initial vocabulary contains only progress profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainClassification {
    ProgressProfile,
}

impl DomainClassification {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProgressProfile => "progress_profile",
        }
    }
}

impl DomainPredicateBody {
    pub const fn is_present(self) -> bool {
        matches!(self, Self::Present)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bodyless => "bodyless",
            Self::Present => "present",
        }
    }
}
