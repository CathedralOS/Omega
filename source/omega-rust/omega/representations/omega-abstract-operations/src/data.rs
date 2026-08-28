use omega_control_flow::StateKey;
use psi_arena::{Arena, Handle, HandleSpan};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractDataPlan {
    pub objects: Arena<AbstractDataObject>,
    pub bytes: Arena<u8>,
    /// Artifact-private selected-conformance tables retained for transitional
    /// instruction selection. `object` is the exact immutable data object whose
    /// pointer becomes the descriptor's table word; semantic rows remain
    /// address-free until object relocation planning.
    pub dynamic_conformance_tables: Arena<AbstractDynamicConformanceTable>,
}

impl Default for AbstractDataPlan {
    fn default() -> Self {
        Self::with_capacity(0, 0)
    }
}

impl AbstractDataPlan {
    pub fn with_capacity(object_capacity: usize, byte_capacity: usize) -> Self {
        Self {
            objects: Arena::with_capacity(object_capacity),
            bytes: Arena::with_capacity(byte_capacity),
            dynamic_conformance_tables: Arena::new(),
        }
    }

    /// Resolve one exact selected-conformance table. Missing, duplicate, or
    /// malformed bindings all return `None`: lowering must never guess a table
    /// from a carrier spelling or accept a stale ordinary data object.
    pub fn dynamic_conformance_table_object(
        &self,
        target_trait: psi_symbols::SymbolHandle,
        conformance: psi_symbols::SymbolHandle,
    ) -> Option<AbstractDataObjectHandle> {
        let mut matches = self
            .dynamic_conformance_tables
            .iter()
            .map(|(_, table)| table)
            .filter(|table| table.target_trait == target_trait && table.conformance == conformance);
        let table = matches.next()?;
        if matches.next().is_some()
            || !self.objects.is_valid(table.object)
            || self.objects.get(table.object).kind
                != AbstractDataObjectKind::DynamicConformanceTable
        {
            return None;
        }
        Some(table.object)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AbstractDynamicConformanceTable {
    pub object: AbstractDataObjectHandle,
    pub target_trait: psi_symbols::SymbolHandle,
    pub conformance: psi_symbols::SymbolHandle,
    pub trait_identity: Arc<str>,
    pub conformance_identity: Arc<str>,
    pub rows: Vec<AbstractDynamicConformanceTableRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractDynamicConformanceTableRow {
    pub requirement_identity: Arc<str>,
    pub realization_identity: Arc<str>,
    pub realization: StateKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractDataObject {
    pub symbol: Arc<str>,
    pub kind: AbstractDataObjectKind,
    pub offset: usize,
    pub bytes: HandleSpan<u8>,
    pub alignment: usize,
    pub source_key: StateKey,
    pub source_statement: usize,
}

pub type AbstractDataObjectHandle = Handle<AbstractDataObject>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AbstractDataObjectKind {
    StaticString,
    RuntimeTextBuffer,
    HostNewline,
    DynamicConformanceTable,
    #[default]
    Other,
}

impl Default for AbstractDataObject {
    fn default() -> Self {
        Self {
            symbol: Arc::from(""),
            kind: AbstractDataObjectKind::Other,
            offset: 0,
            bytes: HandleSpan::empty(),
            alignment: 1,
            source_key: StateKey::default(),
            source_statement: 0,
        }
    }
}

pub type TargetDataPlan = AbstractDataPlan;
pub type TargetDataObject = AbstractDataObject;
pub type TargetDataObjectHandle = AbstractDataObjectHandle;
pub type TargetDataObjectKind = AbstractDataObjectKind;
