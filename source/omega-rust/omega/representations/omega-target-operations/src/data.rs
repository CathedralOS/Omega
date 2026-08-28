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
    pub target_trait: psi_symbols::SymbolHandle,
    pub conformance: psi_symbols::SymbolHandle,
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
                    TargetDataObjectKind::DynamicConformanceTable => {
                        omega_abstract_operations::AbstractDataObjectKind::DynamicConformanceTable
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

        for (_, table) in data.dynamic_conformance_tables.iter() {
            abstract_data.dynamic_conformance_tables.insert(
                omega_abstract_operations::AbstractDynamicConformanceTable {
                    object: omega_abstract_operations::AbstractDataObjectHandle::from_parts(
                        table.object.arena_index(),
                        table.object.generation(),
                    ),
                    target_trait: table.target_trait,
                    conformance: table.conformance,
                    trait_identity: Arc::clone(&table.trait_identity),
                    conformance_identity: Arc::clone(&table.conformance_identity),
                    rows: table
                        .rows
                        .iter()
                        .map(
                            |row| omega_abstract_operations::AbstractDynamicConformanceTableRow {
                                requirement_identity: Arc::clone(&row.requirement_identity),
                                realization_identity: Arc::clone(&row.realization_identity),
                                realization: row.realization,
                            },
                        )
                        .collect(),
                },
            );
        }

        abstract_data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abstract_data_retains_exact_dynamic_table_binding() {
        let target_trait = psi_symbols::SymbolHandle::from_arena_index(7);
        let conformance = psi_symbols::SymbolHandle::from_arena_index(11);
        let realization = StateKey {
            machine: psi_symbols::SymbolHandle::from_arena_index(13),
            state: psi_symbols::SymbolHandle::from_arena_index(17),
            ..StateKey::default()
        };
        let mut target = TargetDataPlan::with_capacity(1, 8);
        let bytes = target.bytes.insert_many([0; 8]);
        let object = target.objects.insert(TargetDataObject {
            symbol: Arc::from("omega_dynamic_conformance_fixture"),
            kind: TargetDataObjectKind::DynamicConformanceTable,
            offset: 0,
            bytes,
            alignment: 8,
            ..TargetDataObject::default()
        });
        target
            .dynamic_conformance_tables
            .insert(DynamicConformanceTable {
                object,
                target_trait,
                conformance,
                trait_identity: Arc::from("Shape"),
                conformance_identity: Arc::from("Item::Primary"),
                rows: vec![DynamicConformanceTableRow {
                    requirement_identity: Arc::from("Shape::code() -> i32"),
                    realization_identity: Arc::from("Item::code() -> i32"),
                    realization,
                }],
            });

        let mut abstract_data = omega_abstract_operations::AbstractDataPlan::from(&target);
        let abstract_object = abstract_data
            .dynamic_conformance_table_object(target_trait, conformance)
            .expect("one exact table binding");
        assert_eq!(
            abstract_data.objects.get(abstract_object).kind,
            omega_abstract_operations::AbstractDataObjectKind::DynamicConformanceTable
        );
        let [table] = abstract_data.dynamic_conformance_tables.storage_slice() else {
            panic!("one abstract table");
        };
        assert_eq!(table.object, abstract_object);
        assert_eq!(table.target_trait, target_trait);
        assert_eq!(table.conformance, conformance);
        assert_eq!(table.trait_identity.as_ref(), "Shape");
        assert_eq!(table.conformance_identity.as_ref(), "Item::Primary");
        assert_eq!(table.rows.len(), 1);
        assert_eq!(table.rows[0].realization, realization);

        abstract_data
            .dynamic_conformance_tables
            .insert(table.clone());
        assert_eq!(
            abstract_data.dynamic_conformance_table_object(target_trait, conformance),
            None,
            "duplicate semantic bindings must fail closed"
        );
    }
}
