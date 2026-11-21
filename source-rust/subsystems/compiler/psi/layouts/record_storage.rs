//! The record storage and opaque carrier selected by a foreign boundary.
//! A target resolves a layout application before a consumer reads its fields.

use crate::{LayoutApplicationId, StructuralFieldType, TargetLayoutRealization, TerminalModule};
use semantic_vocabulary::{ScalarApplicationId, ScalarType, StructuralTypeId};

pub struct ForeignRecordStorage<'module> {
    pub storage: Option<semantic_vocabulary::AuthoredRecordStorage>,
    fields: Option<&'module [crate::TargetLayoutFieldStorage]>,
}

impl ForeignRecordStorage<'_> {
    pub fn field(
        &self,
        field: &crate::StructuralFieldDeclaration,
    ) -> Result<Option<semantic_vocabulary::AuthoredFieldStorage>, String> {
        match self.fields {
            Some(fields) => Ok(Some(
                fields
                    .iter()
                    .find(|candidate| candidate.field == field.id)
                    .ok_or("target boundary layout omitted a live field")?
                    .storage
                    .clone(),
            )),
            None => Ok(field.storage.clone()),
        }
    }
}

pub fn foreign_record_storage<'module>(
    module: &'module TerminalModule,
    record: StructuralTypeId,
    storage: crate::StructuralRecordStorage,
    target: &str,
) -> Result<ForeignRecordStorage<'module>, String> {
    use crate::StructuralRecordStorage;
    let selected = match storage {
        StructuralRecordStorage::TargetLayout(application) => {
            let application = module
                .layouts
                .target_layout_applications
                .iter()
                .find(|candidate| candidate.id == application && candidate.record == record)
                .ok_or("boundary record has no exact target layout application")?;
            Some(
                application
                    .realizations
                    .iter()
                    .find(|row| row.target == target)
                    .ok_or("boundary record has no layout for its selected target")?,
            )
        }
        _ => None,
    };
    Ok(ForeignRecordStorage {
        storage: match storage {
            StructuralRecordStorage::Packed => None,
            StructuralRecordStorage::Authored(storage) => Some(storage),
            StructuralRecordStorage::TargetLayout(_) => selected.map(|row| row.storage),
            StructuralRecordStorage::AccessView(_) | StructuralRecordStorage::Accessor(_) => {
                return Err("placed data has no ordinary owned boundary storage".into());
            }
        },
        fields: selected.map(|row| row.fields.as_slice()),
    })
}

pub fn foreign_opaque_carrier<'module>(
    module: &'module TerminalModule,
    declaration: &crate::StructuralTypeDeclaration,
) -> Result<Option<(usize, &'module crate::TerminalOpaqueRepresentation)>, String> {
    let selected = module
        .types
        .opaque_representations
        .iter()
        .enumerate()
        .find(|(_, row)| row.opaque == declaration.id);
    if selected.is_none() && declaration.quotient_carrier.is_some() {
        return Err(
            "nominally opaque boundary value requires retained selected-carrier custody".into(),
        );
    }
    Ok(selected)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageGeometryError {
    MissingType(StructuralTypeId),
    RecursiveType(StructuralTypeId),
    NestingLimit,
    UnsupportedType(StructuralTypeId),
    UnsupportedScalar(ScalarType),
    UnsupportedField,
    UnsupportedByteSequence(crate::ByteSequenceCarrier),
    UnresolvedExtent(ScalarApplicationId),
    UnresolvedLayout(StructuralTypeId),
    MissingFieldStorage(StructuralTypeId, semantic_vocabulary::StructuralFieldId),
    InvalidRecordStorage(StructuralTypeId),
    Overflow,
    MissingCarrierLayout,
    InvalidCarrierLayout,
    AggregateLayout(layout_plans::AggregateGeometryError),
}

impl std::fmt::Display for StorageGeometryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "storage geometry: {self:?}")
    }
}
impl std::error::Error for StorageGeometryError {}

pub trait RecordStorageResolver {
    /// A representation consumer can supply its current physical carrier
    /// footprint. Aggregates still use the shared checked field/path walk;
    /// this query only replaces reference and sequence carrier geometry.
    /// `None` retains source geometry, while a physical lookup error must
    /// propagate rather than fall back to a different representation.
    fn carrier_layout(
        &self,
        _owner: StructuralTypeId,
        _carrier: &StructuralFieldType,
    ) -> Result<Option<(u64, u64)>, StorageGeometryError> {
        Ok(None)
    }

    fn realization(
        &self,
        record: StructuralTypeId,
        application: LayoutApplicationId,
    ) -> Result<&TargetLayoutRealization, StorageGeometryError>;
}
