//! Permission events: the ownership-event roles, accesses, sources and
//! provenance every checked flow and lowered summary share.

/// Semantic ownership-event roles. Shared by checked flow and every lowered
/// semantic summary so no stage can reinterpret a generic move/drop marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermissionEventKind {
    Establish,
    #[default]
    Transfer,
    Consume,
    AffineDrop,
}

/// Access carried by one permission-context entry. Ownership events use
/// `Owned`; borrow loans use `Shared` or `Exclusive`. Keeping this axis
/// separate from multiplicity prevents a shared loan from being mistaken for
/// a copyable owned value (or an exclusive loan for a linear value).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermissionAccess {
    #[default]
    Owned,
    Shared,
    Exclusive,
}

/// Stable source identity for a permission event across IR stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermissionEventSource {
    #[default]
    StateEntry,
    Statement {
        statement_index: usize,
    },
    Call {
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: symbols::SymbolHandle,
    },
    StateExit,
}

/// Stable origin of the semantic value/obligation carried by a permission
/// event. Transfers preserve this value; they do not mint a fresh origin.
/// `Unknown` is retained for permission events whose producer cannot yet
/// identify where an affine value was established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermissionProvenance {
    #[default]
    Unknown,
    Established {
        machine_symbol: symbols::SymbolHandle,
        state_symbol: symbols::SymbolHandle,
        source: PermissionEventSource,
    },
    /// A call-local correspondence, not an introduced resource or a single
    /// root lineage. Its checked join receipt retains every exact exit source.
    Joined {
        machine_symbol: symbols::SymbolHandle,
        state_symbol: symbols::SymbolHandle,
        source: PermissionEventSource,
        ordinal: u32,
    },
}

/// Identity of one permission/resource claim, independent of its current place
/// and root-lineage provenance. Transfers preserve this identity. A resource
/// transformation may establish fresh child identities while retaining the
/// same [`PermissionProvenance`] lineage.
///
/// The ordinal distinguishes claims established at the same semantic source
/// (for example, multiple linear fields entering one state). It is allocated
/// deterministically by the checked ownership pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermissionClaimIdentity {
    #[default]
    Unknown,
    Established {
        machine_symbol: symbols::SymbolHandle,
        state_symbol: symbols::SymbolHandle,
        source: PermissionEventSource,
        ordinal: u32,
    },
}
