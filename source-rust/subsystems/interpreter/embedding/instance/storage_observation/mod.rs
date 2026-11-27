//! Inspect the storage the interpreter retains at a completed return or a paused host call.
use super::TerminalInstance;
use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::boundary::structural_inputs::byte_arrays::byte_array_length;
use crate::boundary::values::{
    StructuralByteSequenceRuntimeField, StructuralFieldNamespace, StructuralRuntimePlace,
    TerminalStructuralValue,
};
use crate::engine::execution::TerminalExecution;
use crate::storage::byte_sequences::binding::ByteSequenceBinding;
use crate::storage::structural_storage::contents::{self, StorageLeaf, StorageLeafKind};
use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, PlaceId};
use terminal_psi::{StructuralAccess, StructuralTypeShape};

/// The interpreter establishes when each source of observable storage is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalObservationRoot {
    Result,
    ReturnedArgument(u32),
    ArgumentAtHostCall(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalStorageObservationValue<'storage> {
    Storage(crate::TerminalStorageContents),
    Bytes(crate::TerminalByteRead<'storage>),
    /// Identity of an actual host-supplied value, without exposing
    /// its contents or permitting source code to construct one.
    Opaque(TerminalStructuralValue),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalStorageObservationError {
    Closed,
    NormalReturnRequired,
    PendingHostComputationRequired,
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
    /// Read the issued value at a borrowed empty record or inline field after
    /// normal return. A record constructed by the guest has no issued value.
    pub fn borrowed_entry_host_atom(
        &self,
        argument_index: u32,
        path: &[terminal_psi::StructuralPathSegment],
    ) -> Result<Option<TerminalStructuralValue>, TerminalInterpretError> {
        let execution = self.execution.as_ref().ok_or_else(malformed)?;
        if execution.result.is_none() {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "projected host identity observation requires normal return",
            ));
        }
        let (_, _, root) = execution
            .borrowed_entry_storage
            .iter()
            .find(|(index, _, _)| *index == argument_index)
            .ok_or_else(malformed)?;
        let mut home = StructuralRuntimePlace::from(root);
        for (index, step) in path.iter().enumerate() {
            match step {
                terminal_psi::StructuralPathSegment::Field(_) => {}
                terminal_psi::StructuralPathSegment::Case(case) => {
                    let carrier = crate::engine::custody::resolve_structural_path_type(
                        &execution.structural_types(),
                        root.structural_type,
                        &path[..index],
                    )?;
                    if !execution
                        .case_at(&home)
                        .is_some_and(|tag| tag.structural_type == carrier && tag.case == *case)
                    {
                        return Err(malformed());
                    }
                }
                _ => return Err(malformed()),
            }
            home.path.push(step.clone());
        }
        let mut selected = execution.inline_empty_host_value(root, path)?;
        let home = StructuralRuntimePlace::from(&selected);
        match execution.structural_host_atoms.get(&home) {
            None => Ok(None),
            Some(crate::storage::structural_storage::HostStructuralPayload::Opaque(_)) => {
                execution
                    .forward_host_atom(&mut selected)
                    .ok_or_else(malformed)?;
                Ok(Some(selected))
            }
            Some(_) => Err(malformed()),
        }
    }

    pub fn observe_storage(
        &self,
        root: TerminalObservationRoot,
        path: &[Path],
    ) -> Result<TerminalStorageObservationValue<'_>, TerminalStorageObservationError> {
        let execution = self
            .execution
            .as_ref()
            .ok_or(TerminalStorageObservationError::Closed)?;
        if let Some((place, value)) = borrowed_argument(execution, root)? {
            return observe_leaf(execution, place, value, path);
        }
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
                    .ok_or(malformed())?
                    .place;
                observe_leaf(execution, source, &returned.observation_value(), path)
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
    /// Read the extent selected by the same rooted path used to observe its elements.
    pub fn observe_sequence_length(
        &self,
        root: TerminalObservationRoot,
        path: &[Path],
    ) -> Result<u64, TerminalStorageObservationError> {
        let execution = self
            .execution
            .as_ref()
            .ok_or(TerminalStorageObservationError::Closed)?;
        let returned_value;
        let value = if let Some((_, value)) = borrowed_argument(execution, root)? {
            value
        } else {
            let result = execution
                .result
                .as_ref()
                .ok_or(TerminalStorageObservationError::NormalReturnRequired)?;
            let crate::TerminalExecutionResult::Structural(returned) = result else {
                return Err(TerminalStorageObservationError::NotStructuralResult);
            };
            returned_value = returned.observation_value();
            &returned_value
        };
        let (_, leaf) = selected_leaf(execution, value, path)?;
        let StorageLeafKind::Opaque {
            destination,
            declaration,
        } = leaf.kind
        else {
            return Err(TerminalStorageObservationError::UnknownPath);
        };
        if !matches!(
            execution.structural_declaration(declaration)?.shape,
            StructuralTypeShape::ElementView { .. }
        ) {
            return Err(TerminalStorageObservationError::UnknownPath);
        }
        let descriptor = TerminalStructuralValue {
            opaque_identity: destination.opaque_identity,
            structural_type: declaration,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            path: destination.path,
        };
        let view = execution.element_view_at_resolved_home(&descriptor)?;
        u64::try_from(view.len()).map_err(|_| malformed().into())
    }
}

fn borrowed_argument<'a>(
    execution: &'a TerminalExecution<'_>,
    root: TerminalObservationRoot,
) -> Result<Option<(PlaceId, &'a TerminalStructuralValue)>, TerminalStorageObservationError> {
    let argument_index = match root {
        TerminalObservationRoot::Result => return Ok(None),
        TerminalObservationRoot::ReturnedArgument(_) if execution.result.is_none() => {
            return Err(TerminalStorageObservationError::NormalReturnRequired);
        }
        TerminalObservationRoot::ArgumentAtHostCall(_)
            if !execution
                .awaiting_effect
                .as_ref()
                .is_some_and(|call| !call.awaiting_provider_readiness) =>
        {
            return Err(TerminalStorageObservationError::PendingHostComputationRequired);
        }
        TerminalObservationRoot::ReturnedArgument(index)
        | TerminalObservationRoot::ArgumentAtHostCall(index) => index,
    };
    execution
        .borrowed_entry_storage
        .iter()
        .find(|(index, _, _)| *index == argument_index)
        .map(|(_, place, value)| Some((*place, value)))
        .ok_or(TerminalStorageObservationError::NotBorrowedEntryArgument { argument_index })
}

fn selected_leaf(
    execution: &TerminalExecution<'_>,
    value: &TerminalStructuralValue,
    path: &[Path],
) -> Result<(TerminalStructuralValue, StorageLeaf), TerminalStorageObservationError> {
    contents::select(execution, value, path)
        .map_err(|error| error.in_argument_preparation("select observed storage"))?
        .ok_or(TerminalStorageObservationError::UnknownPath)
}

fn observe_leaf<'storage>(
    execution: &'storage TerminalExecution<'_>,
    source: PlaceId,
    value: &TerminalStructuralValue,
    path: &[Path],
) -> Result<TerminalStorageObservationValue<'storage>, TerminalStorageObservationError> {
    let (selected, leaf) = selected_leaf(execution, value, path)?;
    match &leaf.kind {
        StorageLeafKind::Opaque {
            destination,
            declaration,
        } if execution.structural_declaration(*declaration)?.shape
            == StructuralTypeShape::Opaque =>
        {
            let relative = destination
                .path
                .strip_prefix(selected.path.as_slice())
                .filter(|_| destination.opaque_identity == selected.opaque_identity)
                .ok_or(TerminalStorageObservationError::UnknownPath)?;
            let (qualifications, projected_qualifications) =
                terminal_psi::StructuralPathQualification::select(
                    &selected.qualifications,
                    &selected.projected_qualifications,
                    relative,
                );
            let mut actual = TerminalStructuralValue {
                opaque_identity: destination.opaque_identity,
                structural_type: *declaration,
                qualifications,
                projected_qualifications,
                path: destination.path.clone(),
            };
            if execution.forward_host_atom(&mut actual).is_none() {
                return Err(TerminalStorageObservationError::OpaqueContents);
            }
            Ok(TerminalStorageObservationValue::Opaque(actual))
        }
        StorageLeafKind::Scalar { .. } | StorageLeafKind::Case { .. } => {
            // Only a backing crossing supplies a retained original
            // owner. Inline/presented values keep ordinary owner lookup.
            let source_owner = (selected.opaque_identity != value.opaque_identity
                || selected.path != value.path
                || selected.structural_type != value.structural_type)
                .then_some(&selected);
            let (mut values, _) = contents::capture(
                execution,
                source,
                &selected,
                None,
                None,
                StructuralAccess::SharedBorrow,
                std::slice::from_ref(&leaf),
                &[],
                source_owner,
            )?;
            let observed = values
                .pop()
                .ok_or(TerminalStorageObservationError::UnknownPath)?;
            Ok(TerminalStorageObservationValue::Storage(observed.contents))
        }
        StorageLeafKind::ByteField { .. } => Ok(TerminalStorageObservationValue::Bytes(
            leaf.read_byte_view(execution, path, None)?,
        )),
        StorageLeafKind::EmptyScalarArray {
            destination,
            structural_type,
        } => {
            if byte_array_length(&execution.structural_types(), *structural_type) == Some(0)
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
        StorageLeafKind::Opaque { declaration, .. }
            if execution
                .structural_declaration(*declaration)?
                .shape
                .is_shared_readable_byte_view() =>
        {
            Ok(TerminalStorageObservationValue::Bytes(
                leaf.read_byte_view(execution, path, Some(value))?,
            ))
        }
        StorageLeafKind::Opaque { .. } => Err(TerminalStorageObservationError::OpaqueContents),
    }
}

impl StorageLeaf {
    /// Read the descriptor at its actual storage home. Only a result observed at
    /// its root may use the return slot when there is no stored descriptor.
    pub(crate) fn read_byte_view<'execution>(
        &self,
        execution: &'execution TerminalExecution<'_>,
        path: &[Path],
        returned_root: Option<&TerminalStructuralValue>,
    ) -> Result<crate::TerminalByteRead<'execution>, TerminalInterpretError> {
        match &self.kind {
            StorageLeafKind::ByteField { parent, field, .. } => {
                let namespace = if matches!(path.iter().rev().nth(1), Some(Path::Case(_))) {
                    StructuralFieldNamespace::CasePayload
                } else {
                    StructuralFieldNamespace::Member
                };
                execution
                    .byte_field_read(&StructuralByteSequenceRuntimeField {
                        parent: parent.clone(),
                        namespace,
                        field: *field,
                    })
                    .map_err(|error| error.in_argument_preparation("read observed byte field"))
            }
            StorageLeafKind::Opaque {
                destination,
                declaration,
            } => {
                let unsupported = TerminalInterpretError::UnsupportedSemanticVariant;
                let binding = if let Some(binding) =
                    execution.structural_byte_descriptors.get(destination)
                {
                    let home = match binding {
                        ByteSequenceBinding::Immutable(bytes) => bytes.descriptor_home(),
                        binding => binding.descriptor_home(),
                    }
                    .ok_or_else(|| {
                        unsupported("observed byte field has no retained descriptor home")
                    })?;
                    if home.structural_type != *declaration
                        || StructuralRuntimePlace::from(home) != *destination
                        || !binding.matches_shared_home(home)
                    {
                        return Err(unsupported(
                            "observed byte field differs from its retained descriptor home",
                        ));
                    }
                    binding
                } else {
                    let value = returned_root
                        .filter(|root| {
                            path.is_empty()
                                && root.structural_type == *declaration
                                && StructuralRuntimePlace::from(*root) == *destination
                        })
                        .ok_or_else(|| {
                            unsupported("observed byte field has no retained descriptor binding")
                        })?;
                    let binding = execution.returned_byte_sequence.as_ref().ok_or_else(|| {
                        unsupported("returned byte value has no retained descriptor binding")
                    })?;
                    if !binding.matches_shared_home(value) {
                        return Err(unsupported(
                            "returned byte binding differs from its actual result home",
                        ));
                    }
                    binding
                };
                binding.read_view(execution)
            }
            _ => Err(malformed()),
        }
    }
}

fn observe_case(
    execution: &TerminalExecution<'_>,
    value: &crate::TerminalAggregateValue,
    path: &[Path],
) -> Result<TerminalStorageObservationValue<'static>, TerminalStorageObservationError> {
    let selected = value.result_case.ok_or(malformed())?;
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
    let field_value = value
        .fields
        .iter()
        .find(|candidate| candidate.path.as_slice() == [Path::Field(*field)])
        .ok_or(TerminalStorageObservationError::UnknownPath)?;
    let crate::TerminalStorageContents::Scalar {
        value: scalar,
        stored_scalar: actual_stored,
    } = field_value.contents
    else {
        return Err(malformed().into());
    };
    let declaration = execution.structural_declaration(value.structural_type)?;
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return Err(malformed().into());
    };
    let declaration = terminal_psi::find_structural_case(cases, selected)
        .and_then(|case| case.fields.iter().find(|candidate| candidate.id == *field))
        .ok_or(malformed())?;
    let stored_scalar = declaration
        .storage
        .as_ref()
        .and_then(|storage| storage.stored_scalar())
        .unwrap_or(scalar.scalar_type());
    if actual_stored != stored_scalar {
        return Err(malformed().into());
    }
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
        let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
            return Err(TerminalStorageObservationError::UnknownPath);
        };
        let length = length.literal().ok_or(malformed())?;
        if *index >= length {
            return Err(TerminalStorageObservationError::UnknownPath);
        }
        let length = usize::try_from(length).map_err(|_| malformed())?;
        if remaining.checked_rem(length) != Some(0) {
            return Err(malformed().into());
        }
        remaining /= length;
        offset = offset
            .checked_add(
                usize::try_from(*index)
                    .map_err(|_| malformed())?
                    .checked_mul(remaining)
                    .ok_or(malformed())?,
            )
            .ok_or(malformed())?;
        structural_type = element;
    }
    let declaration = execution.structural_declaration(structural_type)?;
    let StructuralTypeShape::PrimitiveScalar(scalar_type) = declaration.shape else {
        return Err(TerminalStorageObservationError::UnknownPath);
    };
    if remaining != 1 {
        return Err(malformed().into());
    }
    let scalar = *value.elements().get(offset).ok_or(malformed())?;
    if scalar.scalar_type() != scalar_type {
        return Err(malformed().into());
    }
    Ok(TerminalStorageObservationValue::Storage(
        crate::TerminalStorageContents::Scalar {
            value: scalar,
            stored_scalar: scalar_type,
        },
    ))
}

#[cfg(test)]
mod tests;
