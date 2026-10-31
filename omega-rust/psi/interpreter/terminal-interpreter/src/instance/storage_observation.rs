//! Observe actual completed storage without constructing a guest value.
//!
//! Borrowed entry backing and returned owned backing use the same typed leaf
//! selection. Packed scalar returns retain their existing immutable snapshots.
//! No observation conveys a qualification, reference or disposal permission.

use super::TerminalInstance;
use crate::errors::TerminalInterpretError;
use crate::execution::TerminalExecution;
use crate::structural_storage::contents::{self, StorageLeafKind};
use crate::values::{
    StructuralByteSequenceRuntimeField, StructuralFieldNamespace, TerminalStructuralValue,
};
use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, PlaceId};
use terminal_psi::StructuralAccess;

/// Plain observed contents, borrowing any byte payload from its real owner.
/// No variant grants authority to construct, mutate or dispose guest storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalStorageObservationValue<'storage> {
    Storage(crate::TerminalStorageContents),
    Bytes(crate::TerminalByteRead<'storage>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalStorageObservationError {
    Closed,
    NormalReturnRequired,
    NotBorrowedEntryArgument { argument_index: u32 },
    NotStructuralResult,
    UnknownPath,
    OpaqueContents,
    Execution(TerminalInterpretError),
}

impl From<TerminalInterpretError> for TerminalStorageObservationError {
    fn from(error: TerminalInterpretError) -> Self {
        Self::Execution(error)
    }
}

impl TerminalInstance<'_, '_> {
    /// Observe a semantic leaf of original host-lent backing after normal
    /// return. Paths use this program's exact declaration IDs. Owned inputs
    /// are not borrow observations, even when their bits remain resident.
    pub fn observe_borrowed_entry(
        &self,
        argument_index: u32,
        path: &[Path],
    ) -> Result<TerminalStorageObservationValue<'_>, TerminalStorageObservationError> {
        let execution = self
            .execution
            .as_ref()
            .ok_or(TerminalStorageObservationError::Closed)?;
        if execution.result.is_none() {
            return Err(TerminalStorageObservationError::NormalReturnRequired);
        }
        let (_, place, value) = execution
            .borrowed_entry_storage
            .iter()
            .find(|(index, _, _)| *index == argument_index)
            .ok_or(TerminalStorageObservationError::NotBorrowedEntryArgument { argument_index })?;
        observe_leaf(execution, *place, value, path)
    }

    /// Observe one leaf of the actual normal structural result. No callback,
    /// cleanup or guest operation runs. Active Case steps and declared array
    /// bounds authenticate the path; inactive/unknown paths refuse. Bytes
    /// borrow the invocation's retained backing and cannot outlive closing it.
    /// Returned claims and qualifications remain with their original result.
    pub fn observe_result(
        &self,
        path: &[Path],
    ) -> Result<TerminalStorageObservationValue<'_>, TerminalStorageObservationError> {
        let execution = self
            .execution
            .as_ref()
            .ok_or(TerminalStorageObservationError::Closed)?;
        let result = execution
            .result
            .as_ref()
            .ok_or(TerminalStorageObservationError::NormalReturnRequired)?;
        match result {
            crate::TerminalExecutionResult::Structural(returned) => {
                let source = execution
                    .program
                    .machine(execution.current_machine)?
                    .result
                    .structural()
                    .ok_or_else(crate::errors::malformed)?
                    .place;
                observe_leaf(execution, source, &returned.value, path)
            }
            crate::TerminalExecutionResult::ScalarCase(returned) => {
                observe_case(execution, returned.value(), path)
            }
            crate::TerminalExecutionResult::ScalarArray(returned) => {
                observe_array(execution, &returned.value, path)
            }
            crate::TerminalExecutionResult::Unit | crate::TerminalExecutionResult::Scalar(_) => {
                Err(TerminalStorageObservationError::NotStructuralResult)
            }
        }
    }
}

fn observe_leaf<'storage>(
    execution: &'storage TerminalExecution<'_>,
    source: PlaceId,
    value: &TerminalStructuralValue,
    path: &[Path],
) -> Result<TerminalStorageObservationValue<'storage>, TerminalStorageObservationError> {
    let leaf = contents::select(execution, value, path)?
        .ok_or(TerminalStorageObservationError::UnknownPath)?;
    match &leaf.kind {
        StorageLeafKind::Scalar { .. } | StorageLeafKind::Case { .. } => {
            let (mut values, _) = contents::capture(
                execution,
                source,
                value,
                None,
                None,
                StructuralAccess::SharedBorrow,
                std::slice::from_ref(&leaf),
                &[],
            )?;
            let observed = values
                .pop()
                .ok_or(TerminalStorageObservationError::UnknownPath)?;
            Ok(TerminalStorageObservationValue::Storage(observed.contents))
        }
        StorageLeafKind::ByteField { parent, field, .. } => {
            let namespace = if matches!(path.iter().rev().nth(1), Some(Path::Case(_))) {
                StructuralFieldNamespace::CasePayload
            } else {
                StructuralFieldNamespace::Member
            };
            let bytes = execution.byte_field_read(&StructuralByteSequenceRuntimeField {
                parent: parent.clone(),
                namespace,
                field: *field,
            })?;
            Ok(TerminalStorageObservationValue::Bytes(bytes))
        }
        StorageLeafKind::EmptyScalarArray {
            destination,
            structural_type,
        } => {
            if crate::structural_inputs::byte_arrays::byte_array_length(
                &execution.structural_types(),
                *structural_type,
            ) == Some(0)
                && execution.initialized_empty_scalar_array(destination, *structural_type)?
            {
                Ok(TerminalStorageObservationValue::Bytes(
                    crate::TerminalByteRead::contiguous(&[]),
                ))
            } else {
                Err(TerminalStorageObservationError::OpaqueContents)
            }
        }
        StorageLeafKind::ByteArray { destination, .. } => {
            if execution.initialized_empty_byte_array(destination)? {
                return Ok(TerminalStorageObservationValue::Bytes(
                    crate::TerminalByteRead::contiguous(&[]),
                ));
            }
            let bytes = execution
                .structural_byte_storage
                .get(destination)
                .ok_or(TerminalStorageObservationError::OpaqueContents)?;
            Ok(TerminalStorageObservationValue::Bytes(bytes.read_view()?))
        }
        StorageLeafKind::Opaque { .. } => Err(TerminalStorageObservationError::OpaqueContents),
    }
}

fn observe_case(
    execution: &TerminalExecution<'_>,
    value: &crate::TerminalScalarAggregateValue,
    path: &[Path],
) -> Result<TerminalStorageObservationValue<'static>, TerminalStorageObservationError> {
    let selected = value.result_case.ok_or_else(crate::errors::malformed)?;
    if path.is_empty() {
        return Ok(TerminalStorageObservationValue::Storage(
            crate::TerminalStorageContents::Case(selected),
        ));
    }
    let [Path::Case(case), Path::Field(field)] = path else {
        return Err(TerminalStorageObservationError::UnknownPath);
    };
    if *case != selected {
        return Err(TerminalStorageObservationError::UnknownPath);
    }
    let scalar = value
        .fields
        .iter()
        .find(|(candidate, _)| candidate.as_slice() == [Path::Field(*field)])
        .map(|(_, scalar)| *scalar)
        .ok_or(TerminalStorageObservationError::UnknownPath)?;
    let declaration = execution.structural_declaration(value.structural_type)?;
    let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return Err(crate::errors::malformed().into());
    };
    let declaration = terminal_psi::find_structural_case(cases, selected)
        .and_then(|case| case.fields.iter().find(|candidate| candidate.id == *field))
        .ok_or_else(crate::errors::malformed)?;
    let stored_scalar = declaration
        .storage
        .as_ref()
        .and_then(|storage| storage.stored_scalar())
        .unwrap_or(scalar.scalar_type());
    Ok(TerminalStorageObservationValue::Storage(
        crate::TerminalStorageContents::Scalar {
            value: scalar,
            stored_scalar,
        },
    ))
}

fn observe_array(
    execution: &TerminalExecution<'_>,
    value: &crate::TerminalScalarArrayValue,
    path: &[Path],
) -> Result<TerminalStorageObservationValue<'static>, TerminalStorageObservationError> {
    let mut structural_type = value.structural_type;
    let mut remaining = value.len();
    let mut offset = 0usize;
    for segment in path {
        let Path::FixedIndex(index) = segment else {
            return Err(TerminalStorageObservationError::UnknownPath);
        };
        let declaration = execution.structural_declaration(structural_type)?;
        let terminal_psi::StructuralTypeShape::FixedArray { element, length } = declaration.shape
        else {
            return Err(TerminalStorageObservationError::UnknownPath);
        };
        let length = length.literal().ok_or_else(crate::errors::malformed)?;
        if *index >= length {
            return Err(TerminalStorageObservationError::UnknownPath);
        }
        let length = usize::try_from(length).map_err(|_| crate::errors::malformed())?;
        if remaining.checked_rem(length) != Some(0) {
            return Err(crate::errors::malformed().into());
        }
        remaining /= length;
        offset = offset
            .checked_add(
                usize::try_from(*index)
                    .map_err(|_| crate::errors::malformed())?
                    .checked_mul(remaining)
                    .ok_or_else(crate::errors::malformed)?,
            )
            .ok_or_else(crate::errors::malformed)?;
        structural_type = element;
    }
    let declaration = execution.structural_declaration(structural_type)?;
    let terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar_type) = declaration.shape else {
        return Err(TerminalStorageObservationError::UnknownPath);
    };
    if remaining != 1 {
        return Err(crate::errors::malformed().into());
    }
    let scalar = *value
        .elements()
        .get(offset)
        .ok_or_else(crate::errors::malformed)?;
    if scalar.scalar_type() != scalar_type {
        return Err(crate::errors::malformed().into());
    }
    Ok(TerminalStorageObservationValue::Storage(
        crate::TerminalStorageContents::Scalar {
            value: scalar,
            stored_scalar: scalar_type,
        },
    ))
}

#[cfg(test)]
mod architecture {
    #[test]
    fn result_and_entry_observations_share_the_typed_storage_reader() {
        let source = include_str!("storage_observation.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        assert_eq!(source.matches("contents::select(").count(), 1);
        assert_eq!(source.matches("contents::capture(").count(), 1);
        assert_eq!(source.matches("observe_leaf(execution,").count(), 2);
        assert!(!source.contains("TypedTrees"));
        assert!(!source.contains("CheckedTrees"));
        assert!(!source.contains("StructuralValue::minted"));
    }
}
