use omega_control_flow::StateKey;
use psi_arena::{Arena, Handle, HandleSpan};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDataPlan {
    pub objects: Arena<TargetDataObject>,
    pub bytes: Arena<u8>,
    /// Artifact-private selected-conformance tables. `object` identifies the
    /// zero-filled pointer slots in `bytes`; row targets remain address-free
    /// until relocation planning binds them to private functions.
    pub dynamic_conformance_tables: Arena<DynamicConformanceTable>,
}

impl Default for TargetDataPlan {
    fn default() -> Self {
        Self::with_capacity(0, 0)
    }
}

impl TargetDataPlan {
    pub fn with_capacity(object_capacity: usize, byte_capacity: usize) -> Self {
        Self {
            objects: Arena::with_capacity(object_capacity),
            bytes: Arena::with_capacity(byte_capacity),
            dynamic_conformance_tables: Arena::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DynamicConformanceTable {
    pub object: TargetDataObjectHandle,
    pub trait_identity: Arc<str>,
    pub conformance_identity: Arc<str>,
    pub rows: Vec<DynamicConformanceTableRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicConformanceTableRow {
    pub requirement_identity: Arc<str>,
    pub realization_identity: Arc<str>,
    /// Exact address-free private function target for the future relocation.
    pub realization: StateKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDataObject {
    pub symbol: Arc<str>,
    pub kind: TargetDataObjectKind,
    pub offset: usize,
    pub bytes: HandleSpan<u8>,
    pub alignment: usize,
    pub source_key: StateKey,
    pub source_statement: usize,
}

pub type TargetDataObjectHandle = Handle<TargetDataObject>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TargetDataObjectKind {
    StaticString,
    RuntimeTextBuffer,
    HostNewline,
    DynamicConformanceTable,
    #[default]
    Other,
}

impl Default for TargetDataObject {
    fn default() -> Self {
        Self {
            symbol: Arc::from(""),
            kind: TargetDataObjectKind::Other,
            offset: 0,
            bytes: HandleSpan::empty(),
            alignment: 1,
            source_key: StateKey::default(),
            source_statement: 0,
        }
    }
}

pub fn target_data_handle_from_abstract(
    handle: omega_abstract_operations::AbstractDataObjectHandle,
) -> TargetDataObjectHandle {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

impl From<&TargetDataPlan> for omega_abstract_operations::AbstractDataPlan {
    fn from(data: &TargetDataPlan) -> Self {
        let mut abstract_data = omega_abstract_operations::AbstractDataPlan::with_capacity(
            data.objects.len(),
            data.bytes.len(),
        );

        abstract_data.bytes = data.bytes.clone();
        for (_, object) in data.objects.iter() {
            abstract_data
                .objects
                .insert(omega_abstract_operations::AbstractDataObject {
                    symbol: Arc::clone(&object.symbol),
                    kind: match object.kind {
                        TargetDataObjectKind::StaticString => {
                            omega_abstract_operations::AbstractDataObjectKind::StaticString
                        }
                        TargetDataObjectKind::RuntimeTextBuffer => {
                            omega_abstract_operations::AbstractDataObjectKind::RuntimeTextBuffer
                        }
                        TargetDataObjectKind::HostNewline => {
                            omega_abstract_operations::AbstractDataObjectKind::HostNewline
                        }
                        // The abstract-operation lane does not consume the
                        // table's semantic rows yet. Its byte object remains
                        // visible as ordinary immutable target data while the
                        // exact table plan stays on `TargetDataPlan`.
                        TargetDataObjectKind::DynamicConformanceTable => {
                            omega_abstract_operations::AbstractDataObjectKind::Other
                        }
                        TargetDataObjectKind::Other => {
                            omega_abstract_operations::AbstractDataObjectKind::Other
                        }
                    },
                    offset: object.offset,
                    bytes: object.bytes,
                    alignment: object.alignment,
                    source_key: object.source_key,
                    source_statement: object.source_statement,
                });
        }

        abstract_data
    }
}
