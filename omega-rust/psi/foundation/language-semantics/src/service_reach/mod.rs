//! Service reach and operational interfaces: the synchronous invocation,
//! suspension and blocking plans, their summaries, and the tables that name
//! service reach rows.

use crate::{ServiceReachId, ServiceReachRowId};

/// Whether a machine's service reach is inferred privately or published as a
/// stable caller/provider ceiling. Published omission is represented by an
/// explicit empty row, never by `InternalInferred`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServiceReachInterface {
    #[default]
    InternalInferred,
    PublishedCeiling(ServiceReachRowId),
}

/// The service-reach contract and checked body summary. The checked summary
/// may refine a published ceiling but may never widen it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ServiceReachPlan {
    pub interface: ServiceReachInterface,
    pub checked_inferred: ServiceReachRowId,
}

/// Whether the direct synchronous invocation set is private inference or a
/// published ceiling. Published omission is an explicit empty edge set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SynchronousInvocationInterface {
    #[default]
    InternalInferred,
    PublishedCeiling,
}

/// Erased direct-edge metadata retained in checked artifacts. Targets use
/// canonical positional identities (`parameter:N`) or canonical boundary
/// service names (`service:Name`); they are never replaced by reach closure.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SynchronousInvocationPlan {
    pub interface: SynchronousInvocationInterface,
    pub published: Vec<String>,
    pub checked_inferred: Vec<String>,
}

/// Whether suspension is inferred privately or published as an independent
/// may-ceiling. `PublishedMaySuspend(false)` is the public negative guarantee
/// produced by omitting `suspends;` on an export or requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SuspensionInterface {
    #[default]
    InternalInferred,
    PublishedMaySuspend(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SuspensionPlan {
    pub interface: SuspensionInterface,
    pub checked_may_suspend: bool,
}

/// Whether worker blocking is inferred privately or published as an
/// independent may-ceiling. `PublishedMayBlock(false)` is the public negative
/// guarantee produced by omitting `blocks;` on an export or requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockingInterface {
    #[default]
    InternalInferred,
    PublishedMayBlock(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlockingPlan {
    pub interface: BlockingInterface,
    pub checked_may_block: bool,
}

/// Canonical service reach attached to one flow/graph scope. Rows index the
/// representation root's shared `ServiceReachRowTable`; no spelling or numeric
/// compatibility bit is stored on individual nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ServiceReachSummary {
    pub direct: ServiceReachRowId,
    pub transitive: ServiceReachRowId,
}

/// Suspension possibility attached to one flow/graph scope. Kept separate
/// from blocking so downstream consumers cannot accidentally treat parking an
/// activation as occupying its worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SuspensionSummary {
    pub direct_may_suspend: bool,
    pub transitive_may_suspend: bool,
}

/// Worker-blocking possibility attached to one flow/graph scope. Kept
/// separate from suspension because the two public may-ceilings compose and
/// admit independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlockingSummary {
    pub direct_may_block: bool,
    pub transitive_may_block: bool,
}

/// One interned row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ServiceReachRow {
    /// A normalized concrete service set.
    Concrete(Vec<ServiceReachId>),
    /// An independent abstract row bounded above by `bound`. Refinement
    /// clause locations mint these for `reaches _;`: the row names no
    /// services itself and is bounded by the covered requirement's inherited
    /// row, never correlated with a sibling requirement's row.
    AbstractBounded(ServiceReachRowId),
}

/// Deterministic normalizer for service-only rows. Boundary-trait identity is
/// minted before rows are interned; this table owns set normalization and
/// preserves the empty published ceiling as the fixed row id 1.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServiceReachRowTable {
    pub(crate) rows: Vec<ServiceReachRow>,
}

impl ServiceReachRowTable {
    pub const EMPTY_ROW: ServiceReachRowId = ServiceReachRowId(1);

    /// Every retained row ID must still name the same normalized service set.
    pub fn starts_with(&self, retained: &Self) -> bool {
        self.rows.starts_with(&retained.rows)
    }

    pub fn intern(&mut self, mut services: Vec<ServiceReachId>) -> ServiceReachRowId {
        services.sort_by_key(|service| service.0);
        services.dedup();
        if self.rows.is_empty() {
            self.rows.push(ServiceReachRow::Concrete(Vec::new()));
        }
        if let Some(position) = self
            .rows
            .iter()
            .position(|row| matches!(row, ServiceReachRow::Concrete(row) if *row == services))
        {
            return ServiceReachRowId(u32::try_from(position + 1).expect("row table fits u32"));
        }
        self.rows.push(ServiceReachRow::Concrete(services));
        ServiceReachRowId(u32::try_from(self.rows.len()).expect("row table fits u32"))
    }

    /// Intern an independent abstract row bounded above by `bound`. The same
    /// bound interns to the same row so order-independent meets stay stable.
    pub fn intern_abstract(&mut self, bound: ServiceReachRowId) -> ServiceReachRowId {
        if self.rows.is_empty() {
            self.rows.push(ServiceReachRow::Concrete(Vec::new()));
        }
        if let Some(position) = self.rows.iter().position(
            |row| matches!(row, ServiceReachRow::AbstractBounded(existing) if *existing == bound),
        ) {
            return ServiceReachRowId(u32::try_from(position + 1).expect("row table fits u32"));
        }
        self.rows.push(ServiceReachRow::AbstractBounded(bound));
        ServiceReachRowId(u32::try_from(self.rows.len()).expect("row table fits u32"))
    }

    /// The row's ceiling service set. An abstract row reports its bound's
    /// set — the row may narrow further at the bound fit check but never
    /// exceeds it.
    pub fn services(&self, row: ServiceReachRowId) -> &[ServiceReachId] {
        match row
            .0
            .checked_sub(1)
            .and_then(|index| self.rows.get(index as usize))
        {
            Some(ServiceReachRow::Concrete(services)) => services.as_slice(),
            Some(ServiceReachRow::AbstractBounded(bound)) => self.services(*bound),
            None => &[],
        }
    }

    /// Whether the row is an independent abstract row bounded by another row.
    pub fn is_abstract(&self, row: ServiceReachRowId) -> bool {
        matches!(self.entry(row), Some(ServiceReachRow::AbstractBounded(_)))
    }

    /// The row bounding an abstract row.
    pub fn abstract_bound(&self, row: ServiceReachRowId) -> Option<ServiceReachRowId> {
        match self.entry(row) {
            Some(ServiceReachRow::AbstractBounded(bound)) => Some(*bound),
            _ => None,
        }
    }

    fn entry(&self, row: ServiceReachRowId) -> Option<&ServiceReachRow> {
        row.0
            .checked_sub(1)
            .and_then(|index| self.rows.get(index as usize))
    }
}

/// One normalized boundary-service declaration. `symbol` is the resolved
/// declaration identity used inside a compilation; `name` is retained for
/// diagnostics and artifact rendering. Parent closure is normalized once from
/// resolved boundary-trait composition and never reconstructed from spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceReachDefinition {
    pub symbol: symbols::SymbolHandle,
    pub name: String,
    pub parents: Vec<ServiceReachId>,
}

/// Deterministic registry of boundary-service identities. The resolved-tree
/// normalizer interns declarations in canonical name order after symbol
/// assignment, so unrelated source ordering does not perturb row order.
/// Ordinary traits never enter this table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServiceReachTable {
    definitions: Vec<ServiceReachDefinition>,
}

impl ServiceReachTable {
    pub fn intern(&mut self, symbol: symbols::SymbolHandle, name: &str) -> ServiceReachId {
        if let Some((index, _)) = self
            .definitions
            .iter()
            .enumerate()
            .find(|(_, definition)| definition.symbol == symbol)
        {
            return ServiceReachId(u32::try_from(index + 1).expect("service table fits u32"));
        }
        self.definitions.push(ServiceReachDefinition {
            symbol,
            name: name.to_owned(),
            parents: Vec::new(),
        });
        ServiceReachId(u32::try_from(self.definitions.len()).expect("service table fits u32"))
    }

    pub fn id_for_symbol(&self, symbol: symbols::SymbolHandle) -> Option<ServiceReachId> {
        self.definitions
            .iter()
            .position(|definition| definition.symbol == symbol)
            .map(|index| ServiceReachId(u32::try_from(index + 1).expect("service table fits u32")))
    }

    /// Resolve a canonical authored service name to its symbol-backed
    /// identity. This is intentionally an exact, case-sensitive lookup: the
    /// table contains declarations, not the retired global effect catalog.
    pub fn id_for_name(&self, name: &str) -> Option<ServiceReachId> {
        self.definitions
            .iter()
            .position(|definition| definition.name == name)
            .map(|index| ServiceReachId(u32::try_from(index + 1).expect("service table fits u32")))
    }

    pub fn definition(&self, id: ServiceReachId) -> Option<&ServiceReachDefinition> {
        id.0.checked_sub(1)
            .and_then(|index| self.definitions.get(index as usize))
    }

    pub fn definitions(&self) -> &[ServiceReachDefinition] {
        &self.definitions
    }

    pub fn set_parents(&mut self, id: ServiceReachId, mut parents: Vec<ServiceReachId>) {
        parents.sort_by_key(|parent| parent.0);
        parents.dedup();
        if let Some(definition) =
            id.0.checked_sub(1)
                .and_then(|index| self.definitions.get_mut(index as usize))
        {
            definition.parents = parents;
        }
    }

    /// Append `service` and its already-normalized parent closure.
    pub fn extend_closure(&self, service: ServiceReachId, services: &mut Vec<ServiceReachId>) {
        if services.contains(&service) {
            return;
        }
        services.push(service);
        if let Some(definition) = self.definition(service) {
            for parent in &definition.parents {
                self.extend_closure(*parent, services);
            }
        }
    }
}
