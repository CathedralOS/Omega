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

use semantic_vocabulary::{MachineId, ScalarType, StructuralTypeId};

use crate::{StructuralAccess, StructuralMultiplicity};

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
    pub identity: String,
    /// Canonical requirement slots, dense by `slot`.
    pub slots: Vec<TerminalDynamicSlot>,
}

/// One requirement row of an interface, with the call shape every
/// implementation of that row realizes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDynamicSlot {
    pub slot: u32,
    pub declaring_trait_identity: String,
    pub requirement_identity: String,
    /// Canonical value tuple of a finite-family row; empty for a
    /// nongeneric requirement.
    pub family_tuple: Vec<String>,
    /// Runtime scalar formal types in the requirement's authored order.
    pub scalar_parameters: Vec<ScalarType>,
    /// Runtime structural formals, separate from scalar values.
    pub structural_parameters: Vec<TerminalDynamicStructuralFormal>,
    /// The scalar result; `None` for a Unit requirement.
    pub result: Option<ScalarType>,
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
    pub implementation_identity: String,
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
