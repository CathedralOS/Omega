/*
We define source-semantics vocabulary that resolution and typing consult:
binding roles, multiplicities, relevance, carry policies, reference access
modes, domain classifications, and the canonical `self` receiver keyword,
rendered place roots, and reserved `result` binder spelling.
*/

pub mod contract_result {
    pub const RESERVED_RESULT: &str = "result";

    pub fn is_reserved_result(name: &str) -> bool {
        name == RESERVED_RESULT
    }

    pub fn signature_reserves_result<'name>(
        mut parameter_names: impl Iterator<Item = &'name str>,
    ) -> bool {
        !parameter_names.any(is_reserved_result)
    }
}

pub mod place_root {
    pub const PLACE_MEMBER_SEPARATOR: char = '.';

    pub fn place_label(root: &str, field: &str) -> String {
        let mut label = String::with_capacity(root.len() + 1 + field.len());
        label.push_str(root);
        label.push(PLACE_MEMBER_SEPARATOR);
        label.push_str(field);
        label
    }

    pub fn place_field<'label>(root: &str, label: &'label str) -> Option<&'label str> {
        label
            .strip_prefix(root)?
            .strip_prefix(PLACE_MEMBER_SEPARATOR)
    }

    pub fn is_rooted_at(root: &str, label: &str) -> bool {
        label == root || place_field(root, label).is_some()
    }

    #[cfg(test)]
    mod tests {
        use super::{is_rooted_at, place_field, place_label};

        #[test]
        fn place_label_renders_the_member_path_below_the_root() {
            assert_eq!(place_label("self", "count"), "self.count");
            assert_eq!(place_label("main", "count"), "main.count");
            assert_eq!(place_label("main", "items[0]"), "main.items[0]");
            assert_eq!(place_label("holder", "a.b"), "holder.a.b");
        }

        #[test]
        fn place_field_takes_apart_only_labels_rooted_at_the_given_name() {
            assert_eq!(place_field("self", "self.count"), Some("count"));
            assert_eq!(place_field("main", "main.a.b"), Some("a.b"));
            assert_eq!(place_field("main", "main.items[0]"), Some("items[0]"));
            assert_eq!(place_field("main", "main"), None);
            assert_eq!(place_field("main", "mainline.a"), None);
            assert_eq!(place_field("main", "other.count"), None);
            assert_eq!(place_field("main", ""), None);
            assert_eq!(place_field("main", "self.count"), None);
        }

        #[test]
        fn rootedness_covers_the_root_and_places_below_it() {
            assert!(is_rooted_at("main", "main"));
            assert!(is_rooted_at("main", "main.count"));
            assert!(!is_rooted_at("main", "mainline"));
            assert!(!is_rooted_at("main", "other.count"));
            assert!(is_rooted_at("self", "self.count"));
            assert!(!is_rooted_at("self", "main.count"));
        }
    }
}

pub mod receiver_binding {
    pub const SELF_RECEIVER: &str = "self";

    pub fn is_self_receiver(name: &str) -> bool {
        name == SELF_RECEIVER
    }

    #[cfg(test)]
    mod tests {
        use super::is_self_receiver;

        #[test]
        fn only_the_bare_keyword_spells_the_receiver() {
            assert!(is_self_receiver("self"));
            assert!(!is_self_receiver("selfish"));
            assert!(!is_self_receiver("main"));
            assert!(!is_self_receiver(""));
        }
    }
}

/// A decomposition reads either ordinary fields or the case-only component
/// of the same value. Selecting cases creates no storage or access authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
pub enum PatternComponent {
    #[default]
    Fields,
    Case,
}

/// The parser's purpose for a local, independent of its diagnostic name and
/// relevance. Pattern markers retain coverage metadata but do not capture an
/// owned runtime value; their per-field bindings are ordinary values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
pub enum LocalBindingRole {
    #[default]
    Value,
    DispatchSubject, // Evaluates subject before arm selection; typed preparation chooses copy or place observation.
    AtomicObservedResult, // Reserved immutable binding established by the associated atomic event.
    RecordPattern,
    ArmPattern,
    /// A prepared private dispatch capture of existing storage.
    DispatchObservation,
}

impl LocalBindingRole {
    pub const fn is_runtime_binding(self) -> bool {
        matches!(
            self,
            Self::Value
                | Self::DispatchSubject
                | Self::DispatchObservation
                | Self::AtomicObservedResult
        )
    }

    pub const fn has_ordinary_initializer(self) -> bool {
        matches!(
            self,
            Self::Value | Self::DispatchSubject | Self::DispatchObservation
        )
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, compiler_storage::Record,
)]
pub enum Multiplicity {
    Unrestricted, // Freely duplicable and discardable (`[copy]`).
    #[default]
    Affine, // Use at most once; silent discard is legal (ordinary data).
    Linear,       // Use exactly once; discard is an error (`[linear]`).
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, compiler_storage::Record,
)]
pub enum BindingRelevance {
    #[default]
    Relevant, // Ordinary binding with runtime representation.
    Erased, // Proof-side binding authored with `[erased]`.
}

impl BindingRelevance {
    pub const fn is_erased(self) -> bool {
        matches!(self, Self::Erased)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
pub enum DataSupplyMode {
    #[default]
    CheckedShape,
    BoundaryOpaque,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, compiler_storage::Record,
)]
pub enum CarrySuspension {
    #[default]
    Forbidden,
    Allowed,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, compiler_storage::Record,
)]
pub enum CarryCpu {
    #[default]
    Origin,
    Any,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, compiler_storage::Record,
)]
pub enum CarryHostThread {
    #[default]
    Origin,
    Any,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, compiler_storage::Record,
)]
pub enum CarryAddress {
    #[default]
    Stable,
    Movable,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, compiler_storage::Record,
)]
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

    pub const fn permits(self, required: Self) -> bool {
        (matches!(required.suspension, CarrySuspension::Forbidden)
            || matches!(self.suspension, CarrySuspension::Allowed))
            && (matches!(required.cpu, CarryCpu::Origin) || matches!(self.cpu, CarryCpu::Any))
            && (matches!(required.host_thread, CarryHostThread::Origin)
                || matches!(self.host_thread, CarryHostThread::Any))
            && (matches!(required.address, CarryAddress::Stable)
                || matches!(self.address, CarryAddress::Movable))
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
pub struct CallOperationalAcknowledgement {
    pub origin: CallOperationalAcknowledgementOrigin,
    pub acknowledges_suspend: bool,
    pub acknowledges_block: bool,
}

impl CallOperationalAcknowledgement {
    /// Acknowledgements cover the actual envelope's independent waiting axes.
    /// Extra markers grant no effect, carry or execution permission.
    pub fn require_envelope(self, suspends: bool, blocks: bool) -> Result<(), &'static str> {
        match (
            suspends && !self.acknowledges_suspend,
            blocks && !self.acknowledges_block,
        ) {
            (true, true) => Err("call that may suspend and block needs both acknowledgements"),
            (true, false) => Err("call that may suspend needs a suspend acknowledgement"),
            (false, true) => Err("call that may block needs a block acknowledgement"),
            (false, false) => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
pub enum CallOperationalAcknowledgementOrigin {
    #[default]
    Source,
    CompilerSynthesized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
pub enum DomainPredicatePresence {
    #[default]
    Absent,
    Present,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
)]
pub enum ReferenceAccess {
    #[default]
    Shared,
    Mutable,
    WriteOnly, // Exclusive like Mutable, but grants no observation of the referent.
}

impl ReferenceAccess {
    pub const fn is_exclusive(self) -> bool {
        matches!(self, Self::Mutable | Self::WriteOnly)
    }

    pub const fn is_readable(self) -> bool {
        !matches!(self, Self::WriteOnly)
    }

    pub const fn borrow_prefix(self) -> &'static str {
        match self {
            Self::Mutable => "&mut ",
            Self::WriteOnly => "&write ",
            Self::Shared => "&",
        }
    }

    pub const fn borrow_modifier(self) -> &'static str {
        match self {
            Self::Mutable => "mut ",
            Self::WriteOnly => "write ",
            Self::Shared => "",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Shared => "shared",
            Self::Mutable => "mutable",
            Self::WriteOnly => "write_only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
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

impl DomainPredicatePresence {
    pub const fn is_present(self) -> bool {
        matches!(self, Self::Present)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "bodyless",
            Self::Present => "present",
        }
    }
}

#[cfg(test)]
mod domain_predicate_presence_tests {
    use super::DomainPredicatePresence;

    #[test]
    fn absence_and_presence_preserve_canonical_encoding_tags() {
        assert_eq!(
            DomainPredicatePresence::default(),
            DomainPredicatePresence::Absent
        );
        assert!(!DomainPredicatePresence::Absent.is_present());
        assert!(DomainPredicatePresence::Present.is_present());
        assert_eq!(DomainPredicatePresence::Absent.as_str(), "bodyless");
        assert_eq!(DomainPredicatePresence::Present.as_str(), "present");
    }
}

#[cfg(test)]
mod call_acknowledgement_tests {
    use super::CallOperationalAcknowledgement;

    #[test]
    fn waiting_axes_are_independent_and_redundant_markers_grant_nothing() {
        let suspend = CallOperationalAcknowledgement {
            acknowledges_suspend: true,
            ..Default::default()
        };
        let block = CallOperationalAcknowledgement {
            acknowledges_block: true,
            ..Default::default()
        };
        assert_eq!(
            suspend.require_envelope(true, true),
            Err("call that may block needs a block acknowledgement")
        );
        assert_eq!(
            block.require_envelope(true, true),
            Err("call that may suspend needs a suspend acknowledgement")
        );
        let both = CallOperationalAcknowledgement {
            acknowledges_suspend: true,
            acknowledges_block: true,
            ..Default::default()
        };
        assert_eq!(both.require_envelope(true, true), Ok(()));
        assert_eq!(both.require_envelope(false, false), Ok(()));
        assert_eq!(
            CallOperationalAcknowledgement::default().require_envelope(false, true),
            Err("call that may block needs a block acknowledgement")
        );
    }
}
