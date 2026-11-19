//! Retained identities for external bindings, declaration table coordinates and service reach.

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub enum ExternalBindingMechanism {
    Syscall,
    CompilerIntrinsic,
    VtableSlot,
    VtableField,
    TableFunction,
}

impl ExternalBindingMechanism {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Syscall => "syscall",
            Self::CompilerIntrinsic => "compiler_intrinsic",
            Self::VtableSlot => "vtable_slot",
            Self::VtableField => "vtable_field",
            Self::TableFunction => "table_function",
        }
    }

    pub const fn identity_tag(self) -> u8 {
        match self {
            Self::Syscall => 2,
            Self::CompilerIntrinsic => 3,
            Self::VtableSlot => 4,
            Self::VtableField => 5,
            Self::TableFunction => 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub enum ExternalBindingIdentity {
    Syscall { number: i64 },
    CompilerIntrinsic,
    VtableSlot { index: i64 },
    VtableField { field: String },
    TableFunction { field: String },
}

impl ExternalBindingIdentity {
    pub const fn mechanism(&self) -> ExternalBindingMechanism {
        match self {
            Self::Syscall { .. } => ExternalBindingMechanism::Syscall,
            Self::CompilerIntrinsic => ExternalBindingMechanism::CompilerIntrinsic,
            Self::VtableSlot { .. } => ExternalBindingMechanism::VtableSlot,
            Self::VtableField { .. } => ExternalBindingMechanism::VtableField,
            Self::TableFunction { .. } => ExternalBindingMechanism::TableFunction,
        }
    }
}

macro_rules! semantic_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, record_storage::Record)]
        pub struct $name(pub u32);

        impl $name {
            pub const NULL: Self = Self(0);

            pub fn is_valid(self) -> bool {
                self.0 != 0
            }
        }
    };
}

semantic_id!(SemanticDomainId);
semantic_id!(ServiceReachId);
semantic_id!(ServiceReachRowId);
semantic_id!(ExternalBindingId);
semantic_id!(RankingViewId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, record_storage::Record)]
pub enum ServiceReachInterface {
    #[default]
    InternalInferred,
    PublishedCeiling(ServiceReachRowId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, record_storage::Record)]
pub struct ServiceReachPlan {
    pub interface: ServiceReachInterface,
    pub checked_inferred: ServiceReachRowId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, record_storage::Record)]
pub enum SynchronousInvocationInterface {
    #[default]
    InternalInferred,
    PublishedCeiling,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct SynchronousInvocationPlan {
    pub interface: SynchronousInvocationInterface,
    pub published: Vec<String>,
    pub checked_inferred: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, record_storage::Record)]
pub enum SuspensionInterface {
    #[default]
    InternalInferred,
    PublishedMaySuspend(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, record_storage::Record)]
pub struct SuspensionPlan {
    pub interface: SuspensionInterface,
    pub checked_may_suspend: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, record_storage::Record)]
pub enum BlockingInterface {
    #[default]
    InternalInferred,
    PublishedMayBlock(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, record_storage::Record)]
pub struct BlockingPlan {
    pub interface: BlockingInterface,
    pub checked_may_block: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, record_storage::Record)]
pub struct ServiceReachSummary {
    pub direct: ServiceReachRowId,
    pub transitive: ServiceReachRowId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, record_storage::Record)]
pub struct SuspensionSummary {
    pub direct_may_suspend: bool,
    pub transitive_may_suspend: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, record_storage::Record)]
pub struct BlockingSummary {
    pub direct_may_block: bool,
    pub transitive_may_block: bool,
}

pub const RANKING_VIEW_CORE_SOURCE: &str = "nat.omg";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RankingViewDeclaration {
    pub view: RankingViewId,
    pub source: &'static str,
    pub namespace: &'static str,
    pub name: &'static str,
    pub parameter: &'static str,
    pub subject_count: usize,
    pub result: &'static str,
}

impl RankingViewDeclaration {
    pub fn path(&self) -> String {
        format!("{}::{}", self.namespace, self.name)
    }
}

impl RankingViewId {
    pub const NAT_DESCENDING: Self = Self(1);
    pub const NAT_BOUNDED_DISTANCE: Self = Self(2);
    pub const SLICE_LENGTH: Self = Self(3);
    pub const NAT_INCREASING_TO: Self = Self(4);

    pub fn canonical(path: &str) -> Option<Self> {
        match path {
            "Nat::Descending" => Some(Self::NAT_DESCENDING),
            "Nat::BoundedDistance" => Some(Self::NAT_BOUNDED_DISTANCE),
            "Slice::Length" => Some(Self::SLICE_LENGTH),
            "Nat::IncreasingTo" => Some(Self::NAT_INCREASING_TO),
            _ => None,
        }
    }

    pub fn canonical_path(self) -> Option<&'static str> {
        match self {
            Self::NAT_DESCENDING => Some("Nat::Descending"),
            Self::NAT_BOUNDED_DISTANCE => Some("Nat::BoundedDistance"),
            Self::SLICE_LENGTH => Some("Slice::Length"),
            Self::NAT_INCREASING_TO => Some("Nat::IncreasingTo"),
            _ => None,
        }
    }

    pub fn catalog_declaration(self) -> Option<RankingViewDeclaration> {
        let (name, subject_count) = match self {
            Self::NAT_DESCENDING => ("Descending", 1),
            Self::NAT_BOUNDED_DISTANCE => ("BoundedDistance", 2),
            _ => return None,
        };
        Some(RankingViewDeclaration {
            view: self,
            source: RANKING_VIEW_CORE_SOURCE,
            namespace: "Nat",
            name,
            parameter: "u64",
            subject_count,
            result: "u64",
        })
    }

    pub fn from_catalog_declaration(namespace: &str, name: &str) -> Option<RankingViewDeclaration> {
        [
            Self::NAT_DESCENDING,
            Self::NAT_BOUNDED_DISTANCE,
            Self::SLICE_LENGTH,
            Self::NAT_INCREASING_TO,
        ]
        .into_iter()
        .filter_map(Self::catalog_declaration)
        .find(|declaration| declaration.namespace == namespace && declaration.name == name)
    }
}

#[cfg(test)]
mod ranking_tests {
    use crate::RankingViewId;

    #[test]
    fn canonical_view_catalog_round_trips() {
        for id in [
            RankingViewId::NAT_DESCENDING,
            RankingViewId::NAT_BOUNDED_DISTANCE,
            RankingViewId::SLICE_LENGTH,
            RankingViewId::NAT_INCREASING_TO,
        ] {
            assert!(id.is_valid());
            let path = id.canonical_path().expect("builtin has a spelling");
            assert_eq!(RankingViewId::canonical(path), Some(id));
        }
        assert_eq!(RankingViewId::canonical("Card::PowerOrder"), None);
        assert_eq!(RankingViewId::NULL.canonical_path(), None);
    }
}
