//! Runtime implementation selection (`dyn Trait<Args>`).
//!
//! A selector is an ordinary scalar value produced by
//! `OperationKind::SelectImplementation`; it names one table of this catalog
//! and packages no data. `OperationKind::CallDynamic` reads a selector, finds
//! the selected table's row for one interface slot, and invokes that row's
//! machine with the call's ordinary arguments. Rebinding, joins, storage and
//! forwarding are ordinary value flow of the selector
//! ([dynamic dispatch](../../../../../../../wiki/spec/terminal-psi/dynamic_dispatch.md)).
//!
//! The interface owns each slot's call shape: the requirement's signature
//! after its trait arguments are substituted. Every table row realizes that
//! exact shape, so no row is a representative for the others. The shape is
//! a call shape, not a declaration: it names value types, structural types
//! and custody, never a callee's own places or values.

use crate::terminal_module::TerminalIdentityIndex;
use semantic_vocabulary::{MachineId, ScalarType, StructuralTypeId};

use crate::{StructuralAccess, StructuralMultiplicity, StructuralPathSegment};

/// Every interface and table one module can select at runtime.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDynamicDispatchCatalog {
    /// Dense by `id`, starting at zero.
    pub interfaces: Vec<TerminalDynamicInterface>,
    /// Dense by `id`, starting at zero.
    pub tables: Vec<TerminalDynamicTable>,
}

/// One selectable trait application: `dyn Trait<Args>`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDynamicInterface {
    pub id: u32,
    /// Normalized identity of the selector type.
    pub identity: TerminalIdentityIndex,
    /// Canonical requirement slots, dense by `slot`.
    pub slots: Vec<TerminalDynamicSlot>,
}

/// One requirement row of an interface, with the call shape every
/// implementation of that row realizes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDynamicSlot {
    pub slot: u32,
    pub declaring_trait_identity: TerminalIdentityIndex,
    pub requirement_identity: TerminalIdentityIndex,
    /// Canonical value tuple of a finite-family row; empty for a
    /// nongeneric requirement.
    pub family_tuple: Vec<String>,
    /// Runtime scalar formal types in the requirement's authored order.
    pub scalar_parameters: Vec<ScalarType>,
    /// Runtime structural formals, separate from scalar values.
    pub structural_parameters: Vec<TerminalDynamicStructuralFormal>,
    /// Requirement-owned disposal paths. Every realization receives this
    /// frontier under its private entry claim IDs; caller IDs stay local.
    pub entry_claim_paths: Vec<TerminalDynamicClaimPath>,
    /// Place-free requirement result; private callee result homes stay local.
    pub result: crate::BoundaryMachineResult,
    /// Canonical possible result claims, by typed relative path. Every row
    /// realizes this roster under its own private returning claim identities.
    pub result_claim_paths: Vec<Vec<StructuralPathSegment>>,
    /// Requirement-owned returned-borrow regions. These contain no private
    /// callee place identities and grant no permission to form a reference.
    pub result_reference_regions: Vec<TerminalDynamicReferenceRegion>,
}

/// One result leaf may borrow within the selected input region. The region's
/// endpoint type can differ from the leaf's referent type; each implementation
/// supplies its own checked descriptor, access, bounds and origin refinement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDynamicReferenceRegion {
    pub path: Vec<StructuralPathSegment>,
    pub origin: TerminalDynamicReferenceOrigin,
    pub source_path: Vec<StructuralPathSegment>,
    pub access: StructuralAccess,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalDynamicReferenceOrigin {
    Parameter { argument_index: u32 },
    Invocation { carrier_type: StructuralTypeId },
}

impl TerminalDynamicSlot {
    /// Match a caller's result occurrence without importing its private IDs.
    pub fn matches_operation_result(&self, result: &crate::OperationResult) -> bool {
        if !self.result.matches_operation_result(result) {
            return false;
        }
        let mut paths = result.structural().map_or_else(Vec::new, |result| {
            result
                .claims
                .iter()
                .map(|claim| claim.path.as_slice())
                .collect()
        });
        paths.sort();
        paths
            .iter()
            .copied()
            .eq(self.result_claim_paths.iter().map(Vec::as_slice))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDynamicClaimPath {
    pub argument_index: u32,
    pub path: Vec<StructuralPathSegment>,
}

/// One structural formal of a slot: what every row's parameter at
/// `position` declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDynamicStructuralFormal {
    /// Authored position among the requirement's formals.
    pub position: u32,
    pub structural_type: StructuralTypeId,
    pub access: StructuralAccess,
    pub multiplicity: StructuralMultiplicity,
}

/// One named implementation's complete row map for one interface. A
/// selector naming this table dispatches each slot to `rows[slot]`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDynamicTable {
    pub id: u32,
    pub interface: u32,
    /// Normalized identity of the selected named implementation.
    pub implementation_identity: TerminalIdentityIndex,
    /// One row per interface slot. A semantic row without an executable
    /// machine is absent and dispatching it traps.
    pub rows: Vec<Option<MachineId>>,
}

impl TerminalDynamicDispatchCatalog {
    pub fn interface(&self, id: u32) -> Option<&TerminalDynamicInterface> {
        self.interfaces.get(usize::try_from(id).ok()?)
    }

    pub fn table(&self, id: u32) -> Option<&TerminalDynamicTable> {
        self.tables.get(usize::try_from(id).ok()?)
    }

    /// The machine a selector naming `table` dispatches `slot` to.
    pub fn row(&self, table: u32, slot: u32) -> Option<MachineId> {
        *self.table(table)?.rows.get(usize::try_from(slot).ok()?)?
    }
}
