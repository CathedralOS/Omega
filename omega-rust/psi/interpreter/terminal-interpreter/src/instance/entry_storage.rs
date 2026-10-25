//! Readback is observation of original host-lent backing after normal return,
//! not a guest value constructor or a second result/cleanup protocol. Frame
//! bindings may have retired; the invocation retains the original borrowed
//! roots until close. Paths are declaration IDs from this program's catalog.

use super::TerminalInstance;
use crate::effects::storage_inputs::contents::{self, StorageLeafKind};
use crate::errors::TerminalInterpretError;
use crate::values::{StructuralByteSequenceRuntimeField, StructuralFieldNamespace};
use semantic_vocabulary::CanonicalStructuralPathSegment;
use terminal_psi::StructuralAccess;

/// Plain observed contents. No variant conveys a qualification, reference,
/// opaque host object, or permission to construct guest storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEntryStorageValue<'storage> {
    Storage(crate::TerminalStorageContents),
    Bytes(crate::TerminalByteRead<'storage>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEntryStorageError {
    Closed,
    NormalReturnRequired,
    NotBorrowedEntryArgument { argument_index: u32 },
    UnknownPath,
    OpaqueContents,
    Execution(TerminalInterpretError),
}

impl From<TerminalInterpretError> for TerminalEntryStorageError {
    fn from(error: TerminalInterpretError) -> Self {
        Self::Execution(error)
    }
}

impl TerminalInstance<'_, '_> {
    /// Observe one semantic leaf of an original borrowed structural input.
    /// `argument_index` is its dense structural-input position, not a source
    /// parameter position. Reads before normal return (including suspension,
    /// crash and domain exit), after close, and from owned inputs refuse.
    /// Byte payloads borrow the retained backing. The caller can reserve result
    /// custody before copying into its own output, or validate without copying.
    pub fn observe_borrowed_entry(
        &self,
        argument_index: u32,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<TerminalEntryStorageValue<'_>, TerminalEntryStorageError> {
        let execution = self
            .execution
            .as_ref()
            .ok_or(TerminalEntryStorageError::Closed)?;
        if execution.result.is_none() {
            return Err(TerminalEntryStorageError::NormalReturnRequired);
        }
        let (_, place, value) = execution
            .borrowed_entry_storage
            .iter()
            .find(|(index, _, _)| *index == argument_index)
            .ok_or(TerminalEntryStorageError::NotBorrowedEntryArgument { argument_index })?;
        let leaf = contents::select(execution, value, path)?
            .ok_or(TerminalEntryStorageError::UnknownPath)?;
        match &leaf.kind {
            StorageLeafKind::Scalar { .. } | StorageLeafKind::Case { .. } => {
                let (mut values, _) = contents::capture(
                    execution,
                    *place,
                    value,
                    None,
                    StructuralAccess::SharedBorrow,
                    std::slice::from_ref(&leaf),
                )?;
                let observed = values.pop().ok_or(TerminalEntryStorageError::UnknownPath)?;
                Ok(TerminalEntryStorageValue::Storage(observed.contents))
            }
            StorageLeafKind::ByteField { parent, field, .. } => {
                let namespace = if matches!(
                    path.iter().rev().nth(1),
                    Some(CanonicalStructuralPathSegment::Case(_))
                ) {
                    StructuralFieldNamespace::CasePayload
                } else {
                    StructuralFieldNamespace::Member
                };
                let bytes = execution.byte_field_read(&StructuralByteSequenceRuntimeField {
                    parent: parent.clone(),
                    namespace,
                    field: *field,
                })?;
                Ok(TerminalEntryStorageValue::Bytes(bytes))
            }
            StorageLeafKind::ByteArray { destination, .. } => {
                let bytes = execution
                    .structural_byte_storage
                    .get(destination)
                    .ok_or(TerminalEntryStorageError::OpaqueContents)?;
                Ok(TerminalEntryStorageValue::Bytes(
                    crate::TerminalByteRead::contiguous(bytes.bytes()),
                ))
            }
            StorageLeafKind::Opaque { .. } => Err(TerminalEntryStorageError::OpaqueContents),
        }
    }
}
