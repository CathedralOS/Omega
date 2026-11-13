/*
    Boundary storage loan capture, byte-view presentation, reference output
    binding, and original-residence writeback for external host effects.

    We snapshot primitive and active aggregate storage homes before host
    dispatch, verify staged host mutations against original access and type
    restrictions, and commit updates to the original backing.
*/

pub(crate) mod borrowed_completion {
    use super::{StructuralLoanBinding, TerminalBoundaryStructuralBuffer};
    use crate::boundary::effects::scalar_loans::satisfies_source_restriction;
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::StructuralRuntimePlace;
    use crate::engine::execution::TerminalExecution;
    use crate::storage::byte_sequences::storage::ByteSequenceStorage;
    use crate::storage::record::storage::ConstructorStorage;
    use crate::storage::structural_storage::contents::{
        self, StorageLeafKind, TerminalStorageContents,
    };
    use semantic_vocabulary::CanonicalStructuralPathSegment;
    use std::sync::Arc;
    use terminal_psi::StructuralAccess;

    pub(crate) fn stage(
        execution: &TerminalExecution,
        buffer: &TerminalBoundaryStructuralBuffer,
        binding: &mut StructuralLoanBinding,
    ) -> Result<(), TerminalInterpretError> {
        if !Arc::ptr_eq(&buffer.identity, &binding.identity)
            || buffer.argument_index != binding.argument_index
            || buffer.access != binding.access
            || buffer.structural_type != binding.referent.structural_type
            || (binding.access == StructuralAccess::SharedBorrow
                && buffer.values != binding.original)
            || buffer
                .values
                .windows(2)
                .any(|pair| pair[0].path >= pair[1].path)
        {
            return Err(malformed());
        }
        if binding.access == StructuralAccess::MutableBorrow {
            execution.validate_storage_loan_restoration(
                &StructuralRuntimePlace::from(&binding.referent),
                &binding.normal_qualifications,
            )?;
        }
        for scalar in &binding.scalars {
            if binding.access == StructuralAccess::MutableBorrow {
                execution.validate_scalar_loan_restoration(
                    &scalar.storage,
                    &scalar.restoration_owner,
                    &binding.normal_qualifications,
                )?;
            }
            let current = execution.read_primitive_storage(
                binding.source,
                scalar.storage.clone(),
                scalar.scalar,
                scalar.scalar,
                scalar.shared,
            )?;
            let expected = if binding.direct_original {
                let value = buffer
                    .values
                    .binary_search_by(|value| value.path.cmp(&scalar.path))
                    .ok()
                    .and_then(|position| buffer.values.get(position))
                    .ok_or_else(malformed)?;
                let TerminalStorageContents::Scalar { value, .. } = value.contents.clone() else {
                    return Err(malformed());
                };
                value
            } else {
                scalar.original
            };
            if (binding.direct_original && buffer.original_storage.is_none()) || current != expected
            {
                return Err(malformed());
            }
        }
        for leaf in &binding.leaves {
            if let StorageLeafKind::Case {
                destination,
                structural_type,
                case,
            } = &leaf.kind
            {
                let original = execution.structural_cases.require(destination)?;
                if original.structural_type != *structural_type || original.case != *case {
                    return Err(malformed());
                }
            }
        }
        let leaves = contents::describe(
            execution,
            &binding.referent,
            binding.access,
            Some(&buffer.values),
        )?
        .leaves;
        let visible: Vec<_> = leaves
            .iter()
            .filter(|leaf| {
                matches!(
                    leaf.kind,
                    StorageLeafKind::Scalar { .. } | StorageLeafKind::Case { .. }
                )
            })
            .collect();
        if visible.len() != buffer.values.len() {
            return Err(malformed());
        }
        let changed_case = visible.iter().any(|leaf| matches!(leaf.kind, StorageLeafKind::Case { case, .. }
        if binding.original.binary_search_by(|value| value.path.cmp(&leaf.path)).ok()
            .is_none_or(|position| binding.original[position].contents != TerminalStorageContents::Case(case))));
        if changed_case {
            let changed = visible
                .iter()
                .filter_map(|leaf| match &leaf.kind {
                    StorageLeafKind::Case {
                        destination, case, ..
                    } => Some((destination.clone(), *case)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            execution.validate_normal_case_preservation(
                &binding.referent,
                &binding.normal_qualifications,
                &changed,
            )?;
        }
        let mut staged = ConstructorStorage::new(execution.account.as_ref());
        for (leaf, value) in visible.into_iter().zip(&buffer.values) {
            leaf.validate_value(value).map_err(|reason| {
                TerminalInterpretError::StructuralArgumentPreparation {
                    stage: "validate completed storage contents",
                    detail: reason.to_owned(),
                }
            })?;
            match (&leaf.kind, value.contents.clone()) {
                (
                    StorageLeafKind::Scalar { destination, .. },
                    TerminalStorageContents::Scalar { value, .. },
                ) => {
                    let original = binding
                        .scalars
                        .binary_search_by(|original| original.path.cmp(&leaf.path))
                        .ok()
                        .map(|position| &binding.scalars[position]);
                    if let Some(original) = original
                        && original.restriction.as_ref().is_some_and(|field| {
                            !satisfies_source_restriction(value, field, original.storage.encoding)
                        })
                    {
                        return Err(malformed());
                    }
                    if changed_case {
                        staged.stage_scalar_endpoint(destination, value)?;
                    } else {
                        let original = original.ok_or_else(malformed)?;
                        binding
                            .stores
                            .push((original.storage.clone(), original.scalar, value));
                    }
                }
                (
                    StorageLeafKind::Case {
                        destination,
                        structural_type,
                        case,
                    },
                    TerminalStorageContents::Case(actual),
                ) if actual == *case => {
                    if changed_case {
                        staged.stage_case(destination.clone(), *structural_type, *case)?;
                    }
                }
                _ => return Err(malformed()),
            }
        }
        if changed_case {
            for old in &binding.leaves {
                let retained = match &old.kind {
                StorageLeafKind::Opaque { declaration, .. } => leaves.iter().any(|leaf| leaf.path == old.path && matches!(&leaf.kind, StorageLeafKind::Opaque { declaration: current, .. } if current == declaration)),
                StorageLeafKind::ByteField { declaration, .. } => leaves.iter().any(|leaf| leaf.path == old.path && matches!(&leaf.kind, StorageLeafKind::ByteField { declaration: current, .. } if current == declaration)),
                _ => true,
            };
                if !retained {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "case replacement cannot yet transfer or remove opaque child storage",
                    ));
                }
            }
            for leaf in &leaves {
                match &leaf.kind {
                    StorageLeafKind::Opaque {
                        destination,
                        declaration,
                    } => {
                        if !binding.leaves.iter().any(|old| old.path == leaf.path && matches!(&old.kind, StorageLeafKind::Opaque { declaration: original, .. } if original == declaration)) { return Err(TerminalInterpretError::UnsupportedSemanticVariant("case replacement cannot establish new opaque child storage")); }
                        staged.retain_replacement_subtree(execution, destination)?;
                    }
                    StorageLeafKind::ByteField {
                        parent,
                        field,
                        declaration,
                    } => {
                        if !binding.leaves.iter().any(|old| old.path == leaf.path && matches!(&old.kind, StorageLeafKind::ByteField { declaration: original, .. } if original == declaration)) { return Err(TerminalInterpretError::UnsupportedSemanticVariant("case replacement lacks a semantic codec for new byte fields")); }
                        let namespace = if matches!(
                            leaf.path.iter().rev().nth(1),
                            Some(CanonicalStructuralPathSegment::Case(_))
                        ) {
                            crate::boundary::values::StructuralFieldNamespace::CasePayload
                        } else {
                            crate::boundary::values::StructuralFieldNamespace::Member
                        };
                        staged.retain_byte_field(execution, parent, *field, namespace)?;
                    }
                    StorageLeafKind::EmptyScalarArray {
                        destination,
                        structural_type,
                    } => {
                        staged.stage_array(execution, *structural_type, &[], destination)?;
                    }
                    StorageLeafKind::ByteArray {
                        destination,
                        structural_type,
                        length,
                    } => {
                        let values = (0..*length).map(|element_index| {
                            let mut path = leaf.path.clone();
                            path.push(CanonicalStructuralPathSegment::FixedIndex(element_index));
                            let position = buffer
                                .values
                                .binary_search_by(|value| value.path.cmp(&path))
                                .map_err(|_| malformed())?;
                            let value = &buffer.values[position];
                            let TerminalStorageContents::Scalar {
                                value:
                                    crate::boundary::values::TerminalScalarValue::Integer {
                                        value: semantic_vocabulary::IntegerValue::Unsigned(byte),
                                        ..
                                    },
                                ..
                            } = value.contents.clone()
                            else {
                                return Err(malformed());
                            };
                            u8::try_from(byte).map_err(|_| malformed())
                        });
                        let bytes = ByteSequenceStorage::from_values(
                            usize::try_from(*length).map_err(|_| malformed())?,
                            values,
                            execution.account.as_ref(),
                        )?;
                        staged.stage_byte_array(destination.clone(), *structural_type, bytes)?;
                    }
                    _ => {}
                }
            }
            binding.replacement =
                Some(staged.prepare_replacement(
                    execution,
                    StructuralRuntimePlace::from(&binding.referent),
                )?);
        }
        binding.completed_leaves = Some(leaves);
        Ok(())
    }

    pub(crate) fn commit(
        execution: &mut TerminalExecution,
        binding: StructuralLoanBinding,
    ) -> Result<(), TerminalInterpretError> {
        if binding.access == StructuralAccess::SharedBorrow {
            return Ok(());
        }
        if let Some(replacement) = binding.replacement {
            replacement.publish(execution)?;
        } else {
            for (storage, scalar, value) in binding.stores {
                if binding.direct_original {
                    execution.record_original_scalar_write(&storage)?;
                } else {
                    execution.store_primitive_storage(binding.source, storage, scalar, value)?;
                }
            }
        }
        if let Some(references) = binding.reference_replacement {
            references.publish(execution)?;
        }
        Ok(())
    }
}
pub(crate) mod byte_views {
    use super::{BoundaryStorageInputs, TerminalBoundaryStorageBuffer};
    use crate::boundary::effects::TerminalEffectRejection;
    use crate::boundary::effects::original_storage::TerminalOriginalStorageLoan;
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::structural_inputs::byte_arrays::storage::scalar_byte;
    use crate::boundary::values::TerminalStructuralValue;
    use crate::engine::execution::TerminalExecution;
    use crate::storage::byte_sequences::binding::ByteSequenceBinding;
    use crate::storage::byte_sequences::storage::ByteSequenceStorage;
    use crate::storage::byte_sequences::{ByteSequenceView, OriginalByteArrayWindow};
    use crate::storage::primitive_storage::{PrimitiveStorage, PrimitiveStorageEndpoint};
    use std::sync::Arc;
    use terminal_psi::{StructuralAccess, StructuralParameterDeclaration};
    use terminal_semantics::descriptor_parameter_access;

    #[derive(Debug)]
    pub struct TerminalBoundaryByteViewBuffer {
        pub(crate) identity: Arc<()>,
        pub(crate) argument_index: usize,
        pub(crate) access: StructuralAccess,
        length: u64,
        pub(crate) original_storage: Option<TerminalOriginalStorageLoan>,
        acknowledged: bool,
    }

    impl TerminalBoundaryByteViewBuffer {
        pub fn argument_index(&self) -> usize {
            self.argument_index
        }
        pub fn access(&self) -> StructuralAccess {
            self.access
        }
        pub fn length(&self) -> u64 {
            self.length
        }
        pub fn original_storage(&self) -> Option<&TerminalOriginalStorageLoan> {
            self.original_storage.as_ref()
        }
        pub fn acknowledge_original_write(&mut self) -> Result<(), TerminalEffectRejection> {
            if self.access != StructuralAccess::MutableBorrow
                || self.original_storage.is_none()
                || self.acknowledged
            {
                return Err(TerminalEffectRejection::new(
                    "byte-view completion is not an unacknowledged original mutable loan",
                ));
            }
            self.acknowledged = true;
            Ok(())
        }
    }

    pub(crate) struct BoundaryByteViewBinding {
        identity: Arc<()>,
        pub(crate) argument_index: usize,
        pub(crate) access: StructuralAccess,
        pub(crate) source: ByteSequenceBinding,
        referent: TerminalStructuralValue,
        original: ByteSequenceStorage,
        pub(crate) residents: Vec<std::rc::Rc<dyn crate::TerminalScalarBacking>>,
    }

    impl BoundaryStorageInputs {
        pub(crate) fn capture_byte_view(
            &mut self,
            execution: &TerminalExecution,
            parameter: &StructuralParameterDeclaration,
            referent: &TerminalStructuralValue,
            source: &ByteSequenceBinding,
            argument_index: usize,
        ) -> Result<(), TerminalInterpretError> {
            let access =
                descriptor_parameter_access(parameter, execution.structural_types().values())
                    .ok_or_else(malformed)?;
            if parameter.structural_type != referent.structural_type
                || !matches!(
                    access,
                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                )
                || access != source.access()
                || !parameter.projected_qualifications.is_empty()
                || (access == StructuralAccess::MutableBorrow
                    && (!parameter.qualifications.is_empty()
                        || !referent.qualifications.is_empty()
                        || !referent.projected_qualifications.is_empty()))
            {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "original byte-view capture requires its exact readable access and qualification scope",
                ));
            }
            let length = execution.byte_descriptor_length(source, referent)?;
            let bytes = match source {
                ByteSequenceBinding::Immutable(bytes) => {
                    ByteSequenceStorage::copy(bytes.bytes(), execution.account.as_ref())?
                }
                ByteSequenceBinding::MutableArray {
                    array,
                    offset,
                    length,
                    ..
                } => {
                    let start = usize::try_from(*offset).map_err(|_| malformed())?;
                    let end = usize::try_from(offset.checked_add(*length).ok_or_else(malformed)?)
                        .map_err(|_| malformed())?;
                    let view = execution.stored_byte_array_read_view(array, start, end)?;
                    ByteSequenceStorage::from_values(
                        view.len(),
                        (0..view.len()).map(|index| view.read(index)),
                        execution.account.as_ref(),
                    )?
                }
                _ => {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "original byte-view capture does not support a replaceable bounded byte owner",
                    ));
                }
            };
            if u64::try_from(bytes.bytes().len()).ok() != Some(length) {
                return Err(malformed());
            }
            let identity = Arc::new(());
            self.buffers.push(TerminalBoundaryStorageBuffer::ByteView(
                TerminalBoundaryByteViewBuffer {
                    identity: Arc::clone(&identity),
                    argument_index,
                    access,
                    length,
                    original_storage: None,
                    acknowledged: false,
                },
            ));
            self.byte_views.push(BoundaryByteViewBinding {
                identity,
                argument_index,
                access,
                source: source.clone(),
                referent: referent.clone(),
                original: bytes,
                residents: Vec::new(),
            });
            Ok(())
        }

        pub(crate) fn validate_byte_views(
            &self,
            execution: &TerminalExecution,
        ) -> Result<(), TerminalInterpretError> {
            let buffers = self
                .buffers
                .iter()
                .filter_map(|buffer| match buffer {
                    TerminalBoundaryStorageBuffer::ByteView(buffer) => Some(buffer),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if buffers.len() != self.byte_views.len() {
                return Err(malformed());
            }
            for (buffer, binding) in buffers.into_iter().zip(&self.byte_views) {
                if !Arc::ptr_eq(&buffer.identity, &binding.identity)
                    || buffer.argument_index != binding.argument_index
                    || buffer.access != binding.access
                    || buffer.original_storage.is_none()
                    || buffer.length
                        != execution.byte_descriptor_length(&binding.source, &binding.referent)?
                    || (buffer.access == StructuralAccess::MutableBorrow && !buffer.acknowledged)
                {
                    return Err(malformed());
                }
                if let Some(array) = binding.array_window()? {
                    let end = array
                        .offset
                        .checked_add(array.length)
                        .ok_or_else(malformed)?;
                    for (relative, index) in (array.offset..end).enumerate() {
                        let current = execution.stored_byte_array_read(&array.array, index)?;
                        if buffer.access == StructuralAccess::SharedBorrow
                            && current != binding.original.bytes()[relative]
                        {
                            return Err(malformed());
                        }
                        if buffer.access == StructuralAccess::MutableBorrow {
                            let storage = PrimitiveStorage {
                                endpoint: PrimitiveStorageEndpoint::ArrayElement {
                                    array: array.array.clone(),
                                    index,
                                },
                                encoding: None,
                            };
                            execution.validate_scalar_loan_qualifications(&storage)?;
                        }
                    }
                } else {
                    if buffer.access != StructuralAccess::SharedBorrow
                        || binding.residents.len() != binding.original.bytes().len()
                    {
                        return Err(malformed());
                    }
                    for (resident, expected) in
                        binding.residents.iter().zip(binding.original.bytes())
                    {
                        let current = resident
                            .read()
                            .map_err(TerminalInterpretError::ResidentStorage)?;
                        if scalar_byte(&current)? != *expected {
                            return Err(malformed());
                        }
                    }
                }
            }
            Ok(())
        }

        pub(crate) fn commit_byte_views(
            &mut self,
            execution: &mut TerminalExecution,
        ) -> Result<(), TerminalInterpretError> {
            for binding in &self.byte_views {
                let buffer = self
                    .buffers
                    .iter()
                    .find_map(|buffer| match buffer {
                        TerminalBoundaryStorageBuffer::ByteView(buffer)
                            if Arc::ptr_eq(&buffer.identity, &binding.identity) =>
                        {
                            Some(buffer)
                        }
                        _ => None,
                    })
                    .ok_or_else(malformed)?;
                if binding.access != StructuralAccess::MutableBorrow {
                    continue;
                }
                let array = binding.array_window()?.ok_or_else(malformed)?;
                if !buffer.acknowledged {
                    return Err(malformed());
                }
                for index in array.offset
                    ..array
                        .offset
                        .checked_add(array.length)
                        .ok_or_else(malformed)?
                {
                    execution.record_original_scalar_write(&PrimitiveStorage {
                        endpoint: PrimitiveStorageEndpoint::ArrayElement {
                            array: array.array.clone(),
                            index,
                        },
                        encoding: None,
                    })?;
                }
            }
            Ok(())
        }
    }

    impl BoundaryByteViewBinding {
        pub(crate) fn entry_bytes(&self) -> &[u8] {
            self.original.bytes()
        }
        pub(crate) fn view_type(&self) -> semantic_vocabulary::StructuralTypeId {
            self.referent.structural_type
        }
        pub(crate) fn array_window(
            &self,
        ) -> Result<Option<OriginalByteArrayWindow>, TerminalInterpretError> {
            Ok(match &self.source {
                ByteSequenceBinding::Immutable(bytes) => bytes.original_array().cloned(),
                ByteSequenceBinding::MutableArray {
                    array,
                    array_type,
                    offset,
                    length,
                    ..
                } => Some(OriginalByteArrayWindow {
                    array: array.clone(),
                    array_type: *array_type,
                    offset: usize::try_from(*offset).map_err(|_| malformed())?,
                    length: usize::try_from(*length).map_err(|_| malformed())?,
                }),
                _ => return Err(malformed()),
            })
        }

        pub(crate) fn raw_bytes(&self) -> Result<&ByteSequenceView, TerminalInterpretError> {
            match &self.source {
                ByteSequenceBinding::Immutable(bytes) if bytes.original_array().is_none() => {
                    Ok(bytes)
                }
                _ => Err(malformed()),
            }
        }
    }
}
pub(crate) mod original_residence {
    use super::{BoundaryStorageInputs, TerminalBoundaryStorageBuffer};
    use crate::boundary::structural_inputs::byte_arrays::byte_array_length;
    use crate::boundary::structural_inputs::byte_arrays::storage::scalar_byte;
    use crate::{
        boundary::effects::{TerminalEffectHandler, original_storage::*},
        boundary::errors::{TerminalInterpretError, malformed},
        boundary::values::{
            StructuralFieldNamespace, StructuralRuntimePlace, TerminalStructuralValue,
        },
        engine::execution::TerminalExecution,
        engine::execution::frame_field_mut,
        storage::primitive_storage::{ArrayCell, PrimitiveStorage, PrimitiveStorageEndpoint},
        storage::scalar_storage::ScalarCell,
    };
    use semantic_vocabulary::{
        BoundaryMachineId, CanonicalStructuralPathSegment as Path, IntegerType,
    };
    use terminal_psi::{StructuralAccess, StructuralPathSegment};

    struct CapturedOriginal {
        owner: Option<StructuralRuntimePlace>,
        request: TerminalOriginalStorageRequest,
        endpoints: Vec<PrimitiveStorageEndpoint>,
    }

    fn capture_byte_view(
        execution: &mut TerminalExecution<'_>,
        binding: &super::byte_views::BoundaryByteViewBinding,
        identity_cursor: &mut Option<u64>,
    ) -> Result<CapturedOriginal, TerminalInterpretError> {
        let (mut capture, offset, length) = if let Some(window) = binding.array_window()? {
            let original = execution
                .original_byte_array_owner(&window.array, identity_cursor)
                .map_err(|error| {
                    error.in_argument_preparation("resolve original byte-array owner")
                })?;
            if byte_array_length(&execution.structural_types(), window.array_type).is_none() {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "original byte-view window requires its actual whole-u8 array geometry",
                ));
            }
            let end = window
                .offset
                .checked_add(window.length)
                .ok_or_else(malformed)?;
            if end > execution.stored_byte_array_length(&window.array)? {
                return Err(malformed());
            }
            if window.length != binding.entry_bytes().len() {
                return Err(malformed());
            }
            if binding.access == StructuralAccess::MutableBorrow
                && !execution.normal_qualifications_cover(
                    &original,
                    &original.qualifications,
                    &original.projected_qualifications,
                    &[],
                )?
            {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "mutable original byte array lacks its owner qualification preservation",
                ));
            }
            let mut values = Vec::new();
            let mut endpoints = Vec::new();
            for index in window.offset..end {
                let endpoint = PrimitiveStorageEndpoint::ArrayElement {
                    array: window.array.clone(),
                    index,
                };
                let storage = PrimitiveStorage {
                    endpoint: endpoint.clone(),
                    encoding: None,
                };
                if binding.access == StructuralAccess::MutableBorrow {
                    execution.validate_scalar_loan_qualifications(&storage)?;
                }
                let value = execution
                    .original_scalar(&original, &storage)
                    .map_err(|error| {
                        error.in_argument_preparation("capture original byte-array cell")
                    })?;
                if scalar_byte(&value.value)? != binding.entry_bytes()[index - window.offset] {
                    return Err(malformed());
                }
                values.push(value);
                endpoints.push(endpoint);
            }
            let request = execution.original_storage_request(
                &original,
                binding.argument_index,
                binding.access,
                Vec::new(),
                values,
            )?;
            (
                CapturedOriginal {
                    owner: Some(StructuralRuntimePlace::from(&original)),
                    request,
                    endpoints,
                },
                window.offset,
                window.length,
            )
        } else {
            let bytes = binding.raw_bytes()?;
            let (namespace, extent, offset) = bytes.original_byte_allocation().ok_or(
                TerminalInterpretError::UnsupportedSemanticVariant(
                    "original byte allocation identity is exhausted",
                ),
            )?;
            let length = bytes.len();
            if binding.access != StructuralAccess::SharedBorrow
                || offset.checked_add(length).is_none_or(|end| end > extent)
            {
                return Err(malformed());
            }
            let scalar_type = IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                .map_err(|_| malformed())?;
            let values = bytes
                .bytes()
                .iter()
                .enumerate()
                .map(|(relative, byte)| {
                    let index = offset.checked_add(relative).ok_or_else(malformed)?;
                    let backing = execution
                        .original_byte_cells
                        .get(&(namespace, index))
                        .map(|cell| {
                            if !cell.owner.shares_backing(bytes) {
                                return Err(malformed());
                            }
                            Ok(cell.backing.clone())
                        })
                        .transpose()?;
                    Ok(TerminalOriginalScalarValue {
                        path: vec![Path::FixedIndex(
                            u64::try_from(index).map_err(|_| malformed())?,
                        )],
                        value: crate::TerminalScalarValue::Integer {
                            scalar_type,
                            value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(*byte)),
                        },
                        backing,
                    })
                })
                .collect::<Result<Vec<_>, TerminalInterpretError>>()?;
            (
                CapturedOriginal {
                    owner: None,
                    request: TerminalOriginalStorageRequest {
                        root: TerminalOriginalStorageRoot {
                            artifact: execution.program.identity.semantic(),
                            namespace,
                            allocation: 1,
                        },
                        structural_type: binding.view_type(),
                        argument_index: binding.argument_index,
                        referent: Vec::new(),
                        access: binding.access,
                        values,
                        allocation_contents: Vec::new(),
                        argument_path: Vec::new(),
                        view_type: binding.view_type(),
                        references: Vec::new(),
                        scalar_projection: None,
                        byte_extent: Some(extent),
                        byte_window: None,
                    },
                    endpoints: Vec::new(),
                },
                offset,
                length,
            )
        };
        capture.request.view_type = binding.view_type();
        capture.request.byte_window = Some((offset, length));
        capture.request.scalar_projection = Some(
            capture
                .request
                .values
                .iter()
                .enumerate()
                .map(|(relative, value)| {
                    Ok((
                        vec![Path::FixedIndex(
                            u64::try_from(relative).map_err(|_| malformed())?,
                        )],
                        value.path.clone(),
                    ))
                })
                .collect::<Result<Vec<_>, TerminalInterpretError>>()?,
        );
        Ok(capture)
    }

    impl BoundaryStorageInputs {
        pub(crate) fn prepare_original_storage(
            &mut self,
            execution: &mut TerminalExecution<'_>,
            handler: &mut impl TerminalEffectHandler,
            boundary: BoundaryMachineId,
            identity_cursor: &mut Option<u64>,
            original: bool,
        ) -> Result<(), TerminalInterpretError> {
            if !original {
                return Ok(());
            }
            let mut captures = Vec::new();
            let mut scalar_storages = Vec::with_capacity(self.scalars.bindings.len());
            for binding in &self.scalars.bindings {
                let storage = original_scalar_storage(execution, &binding.storage)?;
                scalar_storages.push(storage.clone());
                let original = execution.original_scalar_owner(&storage, identity_cursor)?;
                if binding.access == StructuralAccess::MutableBorrow
                    && !execution.normal_qualifications_cover(
                        &original,
                        &original.qualifications,
                        &original.projected_qualifications,
                        &[],
                    )?
                {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "mutable original scalar storage lacks its owner qualification preservation",
                    ));
                }
                let value = execution.original_scalar(&original, &storage)?;
                let allocation_contents =
                    execution.original_scalar_allocation(binding.source, &original, &storage)?;
                let mut request = execution.original_storage_request(
                    &original,
                    binding.argument_index,
                    binding.access,
                    value.path.clone(),
                    vec![value],
                )?;
                request.allocation_contents = allocation_contents;
                captures.push(CapturedOriginal {
                    owner: Some(StructuralRuntimePlace::from(&original)),
                    request,
                    endpoints: vec![storage.endpoint.clone()],
                });
            }
            for binding in &self.structural {
                capture_graph(
                    execution,
                    binding.source,
                    &binding.source_path,
                    &binding.referent,
                    binding.aggregate.as_ref(),
                    Some(&binding.scalars),
                    identity_cursor,
                    binding.argument_index,
                    binding.access,
                    &binding.normal_qualifications,
                    Vec::new(),
                    &mut Vec::new(),
                    &mut captures,
                )?;
            }
            for binding in &self.byte_views {
                captures.push(
                    capture_byte_view(execution, binding, identity_cursor).map_err(|error| {
                        error.in_argument_preparation("capture original byte view")
                    })?,
                );
            }
            let owners = captures
                .iter()
                .map(|capture| capture.owner.clone())
                .collect::<Vec<_>>();
            let (requests, endpoints): (Vec<_>, Vec<_>) = captures
                .into_iter()
                .map(|capture| (capture.request, capture.endpoints))
                .unzip();
            let returned = handler
                .prepare_original_storage(boundary, &requests)
                .map_err(TerminalInterpretError::ResidentStorage)?;
            if returned.len() != requests.len() {
                return Err(malformed());
            }
            let mut prepared = Vec::new();
            let mut raw_prepared = Vec::new();
            let mut raw_cells: crate::storage::runtime_rows::RuntimeRows<
                (u64, usize),
                OriginalByteCell,
            > = crate::storage::runtime_rows::RuntimeRows::new(execution.account.as_ref());
            for ((request, endpoints), backings) in requests.iter().zip(endpoints).zip(returned) {
                if backings.len() != request.values.len() {
                    return Err(malformed());
                }
                if request.byte_extent.is_some() {
                    if !endpoints.is_empty() || request.access != StructuralAccess::SharedBorrow {
                        return Err(malformed());
                    }
                    for (value, backing) in request.values.iter().zip(&backings) {
                        ScalarCell::resident(value.value, backing.clone())?;
                        let [Path::FixedIndex(index)] = value.path.as_slice() else {
                            return Err(malformed());
                        };
                        let key = (
                            request.root.namespace,
                            usize::try_from(*index).map_err(|_| malformed())?,
                        );
                        let owner = self
                            .byte_views
                            .iter()
                            .find(|binding| binding.argument_index == request.argument_index)
                            .ok_or_else(malformed)?
                            .raw_bytes()?;
                        if let Some(cell) = execution
                            .original_byte_cells
                            .get(&key)
                            .or_else(|| raw_cells.get(&key))
                        {
                            if !cell.owner.shares_backing(owner)
                                || !std::rc::Rc::ptr_eq(&cell.backing, backing)
                            {
                                return Err(malformed());
                            }
                        } else {
                            raw_cells.insert(
                                key,
                                OriginalByteCell {
                                    owner: owner.clone(),
                                    backing: backing.clone(),
                                },
                            )?;
                        }
                    }
                    raw_prepared.push((request.argument_index, backings));
                    continue;
                }
                if endpoints.len() != backings.len() {
                    return Err(malformed());
                }
                for ((endpoint, original), backing) in
                    endpoints.into_iter().zip(&request.values).zip(backings)
                {
                    prepared.push((
                        endpoint,
                        ScalarCell::resident(original.value, backing).map_err(|error| {
                            error.in_argument_preparation("validate original resident scalar")
                        })?,
                    ));
                }
            }
            for (endpoint, resident) in &prepared {
                let storage = PrimitiveStorage {
                    endpoint: endpoint.clone(),
                    encoding: None,
                };
                if let Some(cell) = execution.array_cell(&storage)? {
                    let (home, index) = match cell {
                        ArrayCell::Frame { home, index } => (home, index),
                        ArrayCell::Structural { array, index } => {
                            execution
                                .structural_byte_storage
                                .get_mut(&array)
                                .ok_or_else(malformed)?
                                .prepare_resident_install(
                                    index,
                                    resident,
                                    execution.account.as_ref(),
                                )?;
                            continue;
                        }
                    };
                    let (depth, place, _) = home.scalar_array_frame();
                    let arrays = frame_field_mut!(execution, depth, scalar_array_values);
                    arrays
                        .backing_mut(&place)
                        .ok_or_else(malformed)?
                        .prepare_resident_install(index, resident)
                        .map_err(|error| {
                            error.in_argument_preparation("prepare original frame-array residence")
                        })?;
                }
            }
            let mut original_roots =
                crate::storage::runtime_rows::RuntimeRows::new(execution.account.as_ref());
            for (owner, request) in owners.into_iter().zip(&requests) {
                let Some(owner) = owner else {
                    continue;
                };
                let allocation = OriginalStorageAllocation {
                    root: request.root,
                    structural_type: request.structural_type,
                };
                if execution
                    .original_storage_roots
                    .get(&owner)
                    .is_some_and(|previous| *previous != allocation)
                {
                    return Err(malformed());
                }
                original_roots.insert(owner, allocation)?;
            }
            execution.original_byte_cells.validate_append(&raw_cells)?;
            execution
                .original_storage_roots
                .append(&mut original_roots)?;
            execution.original_byte_cells.append(&mut raw_cells)?;
            for (endpoint, resident) in prepared {
                let cell = match endpoint {
                    PrimitiveStorageEndpoint::Scalar(home) => {
                        execution.structural_primitive_storage.get_mut(&home)
                    }
                    PrimitiveStorageEndpoint::ScalarField(field) => {
                        execution.structural_scalar_fields.get_mut(&field)
                    }
                    _ => {
                        let storage = PrimitiveStorage {
                            endpoint,
                            encoding: None,
                        };
                        let (home, index) =
                            match execution.array_cell(&storage)?.ok_or_else(malformed)? {
                                ArrayCell::Frame { home, index } => (home, index),
                                ArrayCell::Structural { array, index } => {
                                    execution
                                        .structural_byte_storage
                                        .get_mut(&array)
                                        .ok_or_else(malformed)?
                                        .install_resident(index, resident)?;
                                    continue;
                                }
                            };
                        let (depth, place, _) = home.scalar_array_frame();
                        let arrays = frame_field_mut!(execution, depth, scalar_array_values);
                        arrays
                            .backing_mut(&place)
                            .ok_or_else(malformed)?
                            .install_resident(index, resident)?;
                        continue;
                    }
                }
                .ok_or_else(malformed)?;
                *cell = resident;
            }
            for (argument_index, residents) in raw_prepared {
                self.byte_views
                    .iter_mut()
                    .find(|binding| binding.argument_index == argument_index)
                    .ok_or_else(malformed)?
                    .residents = residents;
            }
            for request in requests {
                if !request.argument_path.is_empty() {
                    continue;
                }
                let loan = request.loan();
                let buffer = self
                    .buffers
                    .iter_mut()
                    .find(|buffer| match buffer {
                        TerminalBoundaryStorageBuffer::Scalar(buffer) => {
                            buffer.argument_index() == request.argument_index
                        }
                        TerminalBoundaryStorageBuffer::Structural(buffer) => {
                            buffer.argument_index() == request.argument_index
                        }
                        TerminalBoundaryStorageBuffer::ByteView(buffer) => {
                            buffer.argument_index() == request.argument_index
                        }
                    })
                    .ok_or_else(malformed)?;
                match buffer {
                    TerminalBoundaryStorageBuffer::Scalar(buffer) => {
                        buffer.original_storage = Some(loan)
                    }
                    TerminalBoundaryStorageBuffer::Structural(buffer) => {
                        buffer.original_storage = Some(loan)
                    }
                    TerminalBoundaryStorageBuffer::ByteView(buffer) => {
                        buffer.original_storage = Some(loan)
                    }
                }
            }
            for (binding, storage) in self.scalars.bindings.iter_mut().zip(scalar_storages) {
                binding.direct_original = true;
                binding.resident_storage = Some(storage);
            }
            for binding in &mut self.structural {
                binding.direct_original = true;
            }
            Ok(())
        }
    }

    fn original_scalar_storage(
        execution: &TerminalExecution<'_>,
        storage: &PrimitiveStorage,
    ) -> Result<PrimitiveStorage, TerminalInterpretError> {
        let PrimitiveStorageEndpoint::ViewByte { bytes, index } = &storage.endpoint else {
            return Ok(storage.clone());
        };
        let Some(window) = bytes.original_array() else {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "original byte-view scalar requires its captured array window",
            ));
        };
        if *index >= window.length {
            return Err(malformed());
        }
        let index = window.offset.checked_add(*index).ok_or_else(malformed)?;
        if index >= execution.stored_byte_array_length(&window.array)? {
            return Err(malformed());
        }
        Ok(PrimitiveStorage {
            endpoint: PrimitiveStorageEndpoint::ArrayElement {
                array: window.array.clone(),
                index,
            },
            encoding: storage.encoding,
        })
    }

    fn endpoint_parent(
        endpoint: &PrimitiveStorageEndpoint,
    ) -> Result<&StructuralRuntimePlace, TerminalInterpretError> {
        match endpoint {
            PrimitiveStorageEndpoint::Scalar(home) => Ok(home),
            PrimitiveStorageEndpoint::ScalarField(field) => Ok(&field.parent),
            _ => Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "original foreign storage requires its captured array or descriptor home",
            )),
        }
    }

    impl TerminalExecution<'_> {
        fn original_byte_array_owner(
            &mut self,
            array: &crate::boundary::structural_inputs::byte_arrays::storage::ArrayStorage,
            identity_cursor: &mut Option<u64>,
        ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
            use crate::boundary::structural_inputs::byte_arrays::storage::ArrayStorageHome;
            match array.home {
                ArrayStorageHome::Frame {
                    depth,
                    place,
                    structural_type,
                } => {
                    let identity = self
                    .array_storage_identity_staged(depth, place, identity_cursor)
                    .map_err(|error| TerminalInterpretError::StructuralArgumentPreparation {
                        stage: "resolve original frame-array identity",
                        detail: format!(
                            "depth {depth}, place {place:?}, type {structural_type:?}, current depth {}: {error:?}",
                            self.call_stack.len()
                        ),
                    })?;
                    Ok(TerminalStructuralValue::minted(
                        identity,
                        structural_type,
                        self.frame_array(depth, place)
                            .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
                                "original frame-array payload is absent",
                            ))?
                            .qualifications
                            .clone(),
                    ))
                }
                ArrayStorageHome::Runtime => {
                    let structural_type = self
                        .structural_byte_storage
                        .get(&array.referent)
                        .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
                            "original runtime byte-array payload is absent",
                        ))?
                        .structural_type;
                    let mut source = TerminalStructuralValue::minted(
                        array.referent.opaque_identity,
                        structural_type,
                        Vec::new(),
                    );
                    source.path = array.referent.path.clone();
                    self.original_storage_owner(&source).map_err(|error| {
                        error.in_argument_preparation(
                            "resolve original runtime-array enclosing owner",
                        )
                    })
                }
            }
        }

        fn original_scalar_allocation(
            &self,
            source: semantic_vocabulary::PlaceId,
            original: &TerminalStructuralValue,
            storage: &PrimitiveStorage,
        ) -> Result<Vec<TerminalOriginalScalarValue>, TerminalInterpretError> {
            if let Some(ArrayCell::Frame { home, index }) = self.array_cell(storage)? {
                let (depth, place, _) = home.scalar_array_frame();
                let length = self.frame_array(depth, place).ok_or_else(malformed)?.len();
                return (0..length)
                    .filter(|cell| *cell != index)
                    .map(|cell| {
                        self.original_scalar(
                            original,
                            &PrimitiveStorage {
                                endpoint: PrimitiveStorageEndpoint::OwnedArrayElement {
                                    home,
                                    index: cell,
                                },
                                encoding: None,
                            },
                        )
                    })
                    .collect();
            }
            let leaves = crate::storage::structural_storage::contents::describe(
                self,
                original,
                StructuralAccess::SharedBorrow,
                None,
            )?
            .leaves;
            let (_, scalars) = crate::storage::structural_storage::contents::capture(
                self,
                source,
                original,
                None,
                None,
                StructuralAccess::SharedBorrow,
                &leaves,
                &[],
            )?;
            scalars
                .into_iter()
                .filter(|scalar| scalar.storage.endpoint != storage.endpoint)
                .map(|scalar| self.original_scalar(original, &scalar.storage))
                .collect()
        }

        fn original_scalar_owner(
            &mut self,
            storage: &PrimitiveStorage,
            identity_cursor: &mut Option<u64>,
        ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
            if let Some(cell) = self.array_cell(storage)? {
                let home = match cell {
                    ArrayCell::Frame { home, .. } => home,
                    ArrayCell::Structural { array, .. } => {
                        let structural_type = self
                            .structural_byte_storage
                            .require(&array)?
                            .structural_type;
                        let mut source = TerminalStructuralValue::minted(
                            array.opaque_identity,
                            structural_type,
                            Vec::new(),
                        );
                        source.path = array.path;
                        return self.original_storage_owner(&source);
                    }
                };
                let (depth, place, structural_type) = home.scalar_array_frame();
                let identity = self.array_storage_identity_staged(depth, place, identity_cursor)?;
                return Ok(TerminalStructuralValue::minted(
                    identity,
                    structural_type,
                    self.frame_array(depth, place)
                        .ok_or_else(malformed)?
                        .qualifications
                        .clone(),
                ));
            }
            let parent = endpoint_parent(&storage.endpoint)?;
            let mut source = TerminalStructuralValue::minted(
                parent.opaque_identity,
                self.actual_storage_type_from_owner(parent)?,
                Vec::new(),
            );
            source.path = parent.path.clone();
            self.original_storage_owner(&source)
        }

        fn original_storage_owner(
            &self,
            source: &TerminalStructuralValue,
        ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
            let source_home = StructuralRuntimePlace::from(source);
            let selected = self
                .original_storage_roots
                .iter()
                .filter(|(home, _)| {
                    home.opaque_identity == source_home.opaque_identity
                        && source_home.path.starts_with(&home.path)
                })
                .min_by_key(|(home, _)| home.path.len());
            if let Some((home, allocation)) = selected {
                let mut original = TerminalStructuralValue::minted(
                    home.opaque_identity,
                    allocation.structural_type,
                    Vec::new(),
                );
                original.path = home.path.clone();
                return Ok(original);
            }
            let original = self
                .original_owned_allocation(source)
                .map_err(|error| error.in_argument_preparation("resolve live original allocation"))?
                .value;
            if !original.path.is_empty() {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "original storage requires retained enclosing geometry after projected transfer",
                ));
            }
            Ok(original)
        }

        fn original_storage_request(
            &self,
            original: &TerminalStructuralValue,
            argument_index: usize,
            access: StructuralAccess,
            referent: Vec<Path>,
            values: Vec<TerminalOriginalScalarValue>,
        ) -> Result<TerminalOriginalStorageRequest, TerminalInterpretError> {
            if self.component_borrowed_addresses.contains_key(
                &crate::storage::storage_addresses::ScalarStorageAddressKey::Scalar(
                    StructuralRuntimePlace::from(original),
                ),
            ) {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "component scalar loan requires original allocation geometry for storage export",
                ));
            }
            Ok(TerminalOriginalStorageRequest {
                root: self
                    .original_storage_roots
                    .get(&StructuralRuntimePlace::from(original))
                    .map(|allocation| allocation.root)
                    .unwrap_or(TerminalOriginalStorageRoot {
                        artifact: self.program.identity.semantic(),
                        namespace: self.local_structural_identities.exposure_namespace.ok_or(
                            TerminalInterpretError::UnsupportedSemanticVariant(
                                "original storage occurrence namespaces exhausted",
                            ),
                        )?,
                        allocation: original.opaque_identity,
                    }),
                structural_type: original.structural_type,
                argument_index,
                referent,
                access,
                values,
                allocation_contents: Vec::new(),
                argument_path: Vec::new(),
                view_type: original.structural_type,
                references: Vec::new(),
                scalar_projection: None,
                byte_extent: None,
                byte_window: None,
            })
        }

        fn original_scalar(
            &self,
            original: &TerminalStructuralValue,
            storage: &PrimitiveStorage,
        ) -> Result<TerminalOriginalScalarValue, TerminalInterpretError> {
            if storage
                .encoding
                .is_some_and(|encoding| encoding.view != encoding.source)
            {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "original scalar storage requires its exact representation-view codec",
                ));
            }
            if let Some(cell) = self.array_cell(storage)? {
                let (home, index) = match cell {
                    ArrayCell::Frame { home, index } => (home, index),
                    ArrayCell::Structural { array, index } => {
                        let payload = self.structural_byte_storage.require(&array)?;
                        let (selected, mut path) =
                            self.actual_storage_projection(original, &array)?;
                        if selected != payload.structural_type {
                            return Err(malformed());
                        }
                        let (value, backing) = payload.capture_scalar(index)?;
                        if storage
                            .encoding
                            .is_some_and(|encoding| encoding.source != value.scalar_type())
                        {
                            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                                "original byte array scalar requires its exact whole-byte codec",
                            ));
                        }
                        path.push(Path::FixedIndex(
                            u64::try_from(index).map_err(|_| malformed())?,
                        ));
                        return Ok(TerminalOriginalScalarValue {
                            path,
                            value,
                            backing,
                        });
                    }
                };
                let (depth, place, structural_type) = home.scalar_array_frame();
                let array = self.frame_array(depth, place).ok_or_else(malformed)?;
                if array.storage_identity != Some(original.opaque_identity)
                    || structural_type != original.structural_type
                    || !original.path.is_empty()
                {
                    return Err(malformed());
                }
                let cell = array.cell(index).ok_or_else(malformed)?;
                if storage
                    .encoding
                    .is_some_and(|encoding| encoding.source != cell.scalar_type())
                {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "original packed scalar requires its exact whole-cell codec",
                    ));
                }
                let path = terminal_semantics::scalar_array_leaf_path(
                    self.structural_types().values(),
                    structural_type,
                    u64::try_from(index).map_err(|_| malformed())?,
                )
                .ok_or_else(malformed)?
                .into_iter()
                .map(|step| match step {
                    StructuralPathSegment::FixedIndex(index) => Ok(Path::FixedIndex(index)),
                    _ => Err(malformed()),
                })
                .collect::<Result<Vec<_>, _>>()?;
                return Ok(TerminalOriginalScalarValue {
                    path,
                    value: cell.read()?,
                    backing: match cell {
                        ScalarCell::Resident { backing, .. } => Some(backing.clone()),
                        _ => None,
                    },
                });
            }
            let parent = endpoint_parent(&storage.endpoint)?;
            let (selected, mut path) = self.actual_storage_projection(original, parent)?;
            let cell = match &storage.endpoint {
                PrimitiveStorageEndpoint::Scalar(home) => {
                    self.structural_primitive_storage.get(home)
                }
                PrimitiveStorageEndpoint::ScalarField(field) => {
                    if field.namespace == StructuralFieldNamespace::CasePayload {
                        let active = self.structural_cases.require(parent)?;
                        if active.structural_type != selected {
                            return Err(malformed());
                        }
                        path.push(Path::Case(active.case));
                    }
                    path.push(Path::Field(field.field));
                    self.structural_scalar_fields.get(field)
                }
                _ => return Err(malformed()),
            }
            .ok_or_else(malformed)?;
            Ok(TerminalOriginalScalarValue {
                path,
                value: cell.read()?,
                backing: match cell {
                    ScalarCell::Resident { backing, .. } => Some(backing.clone()),
                    _ => None,
                },
            })
        }
    }

    fn capture_graph(
        execution: &mut TerminalExecution<'_>,
        source: semantic_vocabulary::PlaceId,
        source_path: &[StructuralPathSegment],
        referent: &TerminalStructuralValue,
        aggregate: Option<&crate::storage::reference::AggregateBacking>,
        captured_scalars: Option<&[crate::storage::structural_storage::contents::PinnedScalar]>,
        identity_cursor: &mut Option<u64>,
        argument_index: usize,
        access: StructuralAccess,
        normal_qualifications: &[crate::boundary::effects::normal_qualifications::CapturedNormalQualification],
        argument_path: Vec<Path>,
        active: &mut Vec<semantic_vocabulary::StructuralTypeId>,
        captures: &mut Vec<CapturedOriginal>,
    ) -> Result<TerminalOriginalStorageLoan, TerminalInterpretError> {
        use crate::storage::structural_storage::contents::{self, StorageLeafKind};
        use terminal_psi::{StructuralPathSegment, StructuralTypeShape};
        if active.contains(&referent.structural_type) {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "native original reference graph must have a finite acyclic declaration",
            ));
        }
        active.push(referent.structural_type);
        let backing = aggregate.map_or(referent, |backing| &backing.value);
        let leaves = contents::describe(execution, referent, access, None)?.leaves;
        let observed_scalars;
        let scalars = match captured_scalars {
            Some(scalars) => scalars,
            None => {
                observed_scalars = contents::capture(
                    execution,
                    source,
                    referent,
                    None,
                    aggregate,
                    access,
                    &leaves,
                    normal_qualifications,
                )?
                .1;
                &observed_scalars
            }
        };
        let original = match scalars.first() {
            Some(scalar) => execution.original_scalar_owner(&scalar.storage, identity_cursor)?,
            None => execution.original_storage_owner(backing)?,
        };
        if access == StructuralAccess::MutableBorrow
            && !execution.normal_qualifications_cover(
                &original,
                &original.qualifications,
                &original.projected_qualifications,
                normal_qualifications,
            )?
        {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "mutable original storage lacks its owner qualification preservation",
            ));
        }
        let path = if leaves
            .iter()
            .any(|leaf| matches!(leaf.kind, StorageLeafKind::Opaque { .. }))
        {
            let (selected, path) = execution
                .actual_storage_projection(&original, &StructuralRuntimePlace::from(backing))?;
            if selected != backing.structural_type {
                return Err(malformed());
            }
            path
        } else {
            Vec::new()
        };
        let values = scalars
            .iter()
            .map(|scalar| execution.original_scalar(&original, &scalar.storage))
            .collect::<Result<Vec<_>, _>>()?;
        let mut request =
            execution.original_storage_request(&original, argument_index, access, path, values)?;
        request.argument_path = argument_path.clone();
        request.view_type = referent.structural_type;
        {
            request.scalar_projection = Some(
                scalars
                    .iter()
                    .zip(&request.values)
                    .map(|(scalar, original)| (scalar.path.clone(), original.path.clone()))
                    .collect(),
            );
        }
        for leaf in &leaves {
            let StorageLeafKind::Opaque { declaration, .. } = leaf.kind else {
                continue;
            };
            let shape = &execution.structural_declaration(declaration)?.shape;
            let StructuralTypeShape::Reference {
                referent: child_type,
                access: child_access,
            } = *shape
            else {
                continue;
            };
            let child_access = if access == StructuralAccess::SharedBorrow {
                StructuralAccess::SharedBorrow
            } else {
                child_access
            };
            if !matches!(
                child_access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            ) {
                return Err(malformed());
            }
            let relative = terminal_semantics::canonical_structural_storage_path(
                execution.structural_types().values(),
                referent.structural_type,
                &leaf.path,
            )
            .ok_or_else(malformed)?;
            let mut full = source_path.to_vec();
            full.extend_from_slice(&relative);
            full.push(StructuralPathSegment::Referent);
            let child = execution.resolve_live_projection(source, &full, child_access)?;
            if child.structural_type != child_type {
                return Err(malformed());
            }
            let mut child_path = argument_path.clone();
            child_path.extend_from_slice(&leaf.path);
            child_path.push(Path::Referent);
            let target = capture_graph(
                execution,
                source,
                &full,
                &child,
                None,
                None,
                identity_cursor,
                argument_index,
                child_access,
                normal_qualifications,
                child_path,
                active,
                captures,
            )?;
            let mut slot = request.referent.clone();
            slot.extend_from_slice(&leaf.path);
            request.references.push(TerminalOriginalReferenceValue {
                path: slot,
                target,
                referent_type: child_type,
                access: child_access,
            });
        }
        active.pop();
        let loan = request.loan();
        captures.push(CapturedOriginal {
            owner: Some(StructuralRuntimePlace::from(&original)),
            request,
            endpoints: scalars
                .iter()
                .map(|scalar| scalar.storage.endpoint.clone())
                .collect(),
        });
        Ok(loan)
    }
}
pub(crate) mod reference_outputs {
    use super::structural_buffer::{ReferenceOutputWrite, TerminalBoundaryReferenceSelection};
    use super::{CapturedReferenceInput, StructuralLoanBinding, TerminalBoundaryStorageBuffer};
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::engine::execution::TerminalExecution;
    use crate::storage::record::storage::ConstructorStorage;
    use crate::storage::structural_storage::contents::StorageLeafKind;
    use std::sync::Arc;
    use terminal_psi::{
        BoundaryLifetimeDependency, BoundaryMachineDeclaration, BoundaryReferenceDestination,
        StructuralAccess, StructuralPathQualification, StructuralPathSegment, StructuralTypeShape,
    };

    pub(crate) fn source<'captured>(
        selection: &TerminalBoundaryReferenceSelection,
        captures: &'captured [CapturedReferenceInput],
    ) -> Result<&'captured CapturedReferenceInput, TerminalInterpretError> {
        let mut matching = captures.iter().filter(|capture| {
            capture.argument_index == selection.argument_index
                && Arc::ptr_eq(&capture.identity, &selection.identity)
        });
        let selected = matching.next().ok_or_else(malformed)?;
        if matching.next().is_some() {
            return Err(malformed());
        }
        Ok(selected)
    }

    pub(crate) fn stage(
        execution: &TerminalExecution,
        declaration: &BoundaryMachineDeclaration,
        buffers: &[TerminalBoundaryStorageBuffer],
        bindings: &mut [StructuralLoanBinding],
        captures: &[CapturedReferenceInput],
        identity_cursor: &mut Option<u64>,
    ) -> Result<(), TerminalInterpretError> {
        if declaration.reference_outputs.is_empty() {
            if buffers.iter().any(|buffer| matches!(buffer, TerminalBoundaryStorageBuffer::Structural(buffer) if !buffer.references.is_empty())) {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant("boundary has an undeclared reference output request"));
        }
            return Ok(());
        }
        let types = execution.structural_types();
        for output in &declaration.reference_outputs {
            let BoundaryReferenceDestination::Parameter(index) = output.destination;
            if bindings
                .iter()
                .filter(|binding| binding.argument_index == index as usize)
                .count()
                != 1
            {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "boundary reference output has no unique captured destination",
                ));
            }
        }
        let mut staged = Vec::new();
        let mut request_count = 0usize;
        for (binding_index, binding) in bindings.iter().enumerate() {
            let mut matching = buffers.iter().filter_map(|buffer| match buffer {
                TerminalBoundaryStorageBuffer::Structural(buffer)
                    if buffer.argument_index == binding.argument_index
                        && Arc::ptr_eq(&buffer.identity, &binding.identity) =>
                {
                    Some(buffer)
                }
                _ => None,
            });
            let buffer = matching.next().ok_or_else(malformed)?;
            if matching.next().is_some() {
                return Err(malformed());
            }
            request_count = request_count
                .checked_add(buffer.references.len())
                .ok_or_else(malformed)?;
            let destination_index =
                u32::try_from(binding.argument_index).map_err(|_| malformed())?;
            let mut outputs = declaration
                .reference_outputs
                .iter()
                .filter(|output| {
                    output.destination == BoundaryReferenceDestination::Parameter(destination_index)
                })
                .peekable();
            if outputs.peek().is_none() {
                continue;
            }
            let leaves = binding.completed_leaves.as_deref().ok_or_else(malformed)?;
            let cases = leaves
                .iter()
                .filter_map(|leaf| match &leaf.kind {
                    StorageLeafKind::Case { case, .. } => Some((leaf, *case)),
                    _ => None,
                })
                .map(|(leaf, case)| {
                    terminal_semantics::canonical_structural_storage_path(
                        types.values(),
                        binding.referent.structural_type,
                        &leaf.path,
                    )
                    .map(|path| (path, case))
                    .ok_or_else(malformed)
                })
                .collect::<Result<Vec<_>, _>>()?;
            for output in outputs {
                let endpoint = terminal_semantics::runtime_structural_path_tip(
                    types.values(),
                    binding.referent.structural_type,
                    &output.path,
                )
                .ok_or_else(malformed)?;
                let StructuralTypeShape::Reference { access, .. } = endpoint.shape else {
                    return Err(malformed());
                };
                if access != output.access || binding.access != StructuralAccess::MutableBorrow {
                    return Err(malformed());
                }
                let active = crate::boundary::structural_inputs::case_membership::case_projection_with_membership_is_active(
                &types, binding.referent.structural_type, &output.path,
                &|prefix| cases.iter().find(|(path, _)| path == prefix).map(|(_, case)| *case),
            )?;
                if !active {
                    continue;
                }
                let mut requests = buffer
                    .references
                    .iter()
                    .filter(|request| request.path == output.path);
                let request = requests.next().ok_or_else(malformed)?;
                if requests.next().is_some() {
                    return Err(malformed());
                }
                let selected = source(&request.source, captures)?;
                if request.source.access != output.access
                    || !crate::storage::reference::access_covers(selected.access, output.access)
                {
                    return Err(malformed());
                }
                let relative = terminal_semantics::canonical_structural_storage_path(
                    types.values(),
                    selected.referent.structural_type,
                    &request.source.path,
                )
                .ok_or_else(malformed)?;
                if let Some(source_binding) = bindings
                    .iter()
                    .find(|binding| binding.argument_index == selected.argument_index)
                {
                    let leaves = source_binding
                        .completed_leaves
                        .as_deref()
                        .ok_or_else(malformed)?;
                    let prefix_end = relative
                        .iter()
                        .position(|step| matches!(step, StructuralPathSegment::Referent))
                        .unwrap_or(relative.len());
                    let active = crate::boundary::structural_inputs::case_membership::case_projection_with_membership_is_active(
                    &types, selected.referent.structural_type, &relative[..prefix_end],
                    &|prefix| leaves.iter().find_map(|leaf| match &leaf.kind {
                        StorageLeafKind::Case { case, .. } => {
                            let path = terminal_semantics::canonical_structural_storage_path(types.values(), selected.referent.structural_type, &leaf.path)?;
                            (path == prefix).then_some(*case)
                        },
                        _ => None,
                    }),
                )?;
                    if !active {
                        return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                            "reference output selects storage invalidated by staged input case replacement",
                        ));
                    }
                }
                let mut anchor_matches = false;
                for dependency in &output.dependencies {
                    let (argument_index, expected_access, path) = match dependency {
                        BoundaryLifetimeDependency::Referent {
                            argument_index,
                            access,
                        } => (*argument_index as usize, *access, None),
                        BoundaryLifetimeDependency::Stored {
                            argument_index,
                            path,
                            access,
                        } => (*argument_index as usize, *access, Some(path)),
                    };
                    let parameter = declaration
                        .structural_parameters
                        .get(argument_index)
                        .ok_or_else(malformed)?;
                    let mut anchor_captures = captures
                        .iter()
                        .filter(|capture| capture.argument_index == argument_index);
                    let capture = anchor_captures.next().ok_or_else(malformed)?;
                    if anchor_captures.next().is_some() {
                        return Err(malformed());
                    }
                    if let Some(path) = path {
                        let tip = terminal_semantics::runtime_structural_path_tip(
                            types.values(),
                            capture.referent.structural_type,
                            path,
                        )
                        .ok_or_else(malformed)?;
                        if !matches!(tip.shape, StructuralTypeShape::Reference { access, .. } if crate::storage::reference::access_covers(access, expected_access))
                        {
                            return Err(malformed());
                        }
                        let mut full = capture.source_argument.path.clone();
                        full.extend_from_slice(path);
                        execution.resolve_live_projection(
                            capture.source_argument.place,
                            &full,
                            StructuralAccess::SharedBorrow,
                        )?;
                    } else if capture.access == StructuralAccess::Owned
                        || !crate::storage::reference::access_covers(
                            parameter.access,
                            expected_access,
                        )
                    {
                        return Err(malformed());
                    }
                    if argument_index == request.source.argument_index
                        && crate::storage::reference::access_covers(expected_access, output.access)
                    {
                        anchor_matches |= match (
                            relative
                                .iter()
                                .rposition(|step| matches!(step, StructuralPathSegment::Referent)),
                            path,
                        ) {
                            (None, None) => selected.access != StructuralAccess::Owned,
                            (Some(position), Some(anchor)) => {
                                &relative[..position] == anchor.as_slice()
                            }
                            _ => false,
                        };
                    }
                }
                if !anchor_matches {
                    return Err(malformed());
                }
                let mut argument = selected.source_argument.clone();
                argument.path.extend(relative);
                argument.access = output.access;
                let actual = execution.resolve_live_projection(
                    argument.place,
                    &argument.path,
                    output.access,
                )?;
                if selected.access != StructuralAccess::SharedBorrow {
                    let normal = bindings
                        .iter()
                        .find(|binding| binding.argument_index == selected.argument_index)
                        .map_or(&[][..], |binding| binding.normal_qualifications.as_slice());
                    if !execution.normal_qualifications_cover(
                        &actual,
                        &actual.qualifications,
                        &actual.projected_qualifications,
                        normal,
                    )? {
                        return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                            "reference output source lacks exact normal qualification preservation",
                        ));
                    }
                }
                let required = declaration
                    .normal_qualifications
                    .iter()
                    .filter(|row| row.argument_index == destination_index)
                    .filter_map(|row| {
                        row.path.strip_prefix(output.path.as_slice()).map(|path| {
                            StructuralPathQualification {
                                path: path.to_vec(),
                                domain: row.domain,
                            }
                        })
                    })
                    .collect::<Vec<_>>();
                if required
                    .iter()
                    .any(|row| !row.path.starts_with(&[StructuralPathSegment::Referent]))
                {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "reference output cannot establish membership on the descriptor itself",
                    ));
                }
                let (required_whole, required_paths) = StructuralPathQualification::select(
                    &[],
                    &required,
                    &[StructuralPathSegment::Referent],
                );
                let required_paths =
                crate::boundary::structural_inputs::case_membership::active_projected_qualifications(
                    &types,
                    &actual,
                    execution.structural_cases.as_map(),
                    &required_paths,
                )?;
                if required_whole
                    .iter()
                    .any(|domain| !actual.qualifications.contains(domain))
                    || required_paths
                        .iter()
                        .any(|row| !actual.projected_qualifications.contains(row))
                {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "reference output selected source lacks a declared normal membership",
                    ));
                }
                let mut projected = Vec::new();
                StructuralPathQualification::append_under(
                    &actual.qualifications,
                    &actual.projected_qualifications,
                    &[StructuralPathSegment::Referent],
                    &mut projected,
                );
                projected.sort();
                projected.dedup();
                let captured = execution.capture_reference(
                    &argument,
                    endpoint.id,
                    &projected,
                    None,
                    identity_cursor,
                )?;
                let mut leaves = leaves.iter().filter_map(|leaf| match &leaf.kind {
                    StorageLeafKind::Opaque {
                        destination,
                        declaration,
                    } if *declaration == endpoint.id => {
                        let path = terminal_semantics::canonical_structural_storage_path(
                            types.values(),
                            binding.referent.structural_type,
                            &leaf.path,
                        )?;
                        (path == output.path).then_some(destination)
                    }
                    _ => None,
                });
                let destination = leaves.next().ok_or_else(malformed)?.clone();
                if leaves.next().is_some() {
                    return Err(malformed());
                }
                if request.write == ReferenceOutputWrite::Address {
                    let previous = execution.reference_referents.require(&destination)?;
                    let stored_scalar =
                        |reference: &crate::storage::reference::ReferenceReferent| {
                            reference.primitive_loan.as_ref().map(|loan| {
                                loan.storage.storage_scalar().unwrap_or(loan.scalar_type)
                            })
                        };
                    let StructuralTypeShape::Reference { referent, .. } = endpoint.shape else {
                        return Err(malformed());
                    };
                    let primitive = matches!(
                        types.declaration(&referent)?.shape,
                        StructuralTypeShape::PrimitiveScalar(_)
                    );
                    let old_scalar = stored_scalar(previous);
                    let new_scalar = stored_scalar(&captured.referent);
                    if old_scalar != new_scalar || (primitive && old_scalar.is_none()) {
                        return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                            "address-only reference output changes its captured scalar storage interpretation",
                        ));
                    }
                }
                staged.push((binding_index, destination, captured.referent));
            }
        }
        if request_count != staged.len() {
            return Err(malformed());
        }
        for (index, destination, reference) in staged {
            bindings[index]
                .reference_replacement
                .get_or_insert_with(|| ConstructorStorage::new(execution.account.as_ref()))
                .stage_reference(destination, reference)?;
        }
        Ok(())
    }
}
pub(crate) mod reference_result {
    use super::{BoundaryStorageInputs, structural_buffer::TerminalBoundaryReferenceSelection};
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::engine::execution::TerminalExecution;
    use crate::storage::reference::PreparedReference;
    use crate::storage::structural_storage::contents::StorageLeafKind;
    use terminal_psi::{BoundaryMachineDeclaration, StructuralAccess, StructuralPathSegment};

    impl BoundaryStorageInputs {
        pub(crate) fn require_result_captures(
            &self,
            declaration: &BoundaryMachineDeclaration,
        ) -> Result<(), TerminalInterpretError> {
            for region in &declaration.result_reference_regions {
                let terminal_psi::TerminalDynamicReferenceOrigin::Parameter { argument_index } =
                    region.origin
                else {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "host reference result requires captured input storage",
                    ));
                };
                if !region.path.is_empty()
                    || !self.reference_inputs.iter().any(|capture| {
                        capture.argument_index == argument_index as usize
                            && capture.access != StructuralAccess::Owned
                            && crate::storage::reference::access_covers(
                                capture.access,
                                region.access,
                            )
                    })
                {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "host reference result has no admitted borrowed input capture",
                    ));
                }
            }
            Ok(())
        }

        pub(crate) fn prepare_reference_result(
            &self,
            execution: &TerminalExecution,
            declaration: &BoundaryMachineDeclaration,
            result: &terminal_psi::StructuralOperationResult,
            selection: &TerminalBoundaryReferenceSelection,
            identity_cursor: &mut Option<u64>,
        ) -> Result<PreparedReference, TerminalInterpretError> {
            if !execution.supports_captured_reference_result(declaration, result) {
                return Err(malformed());
            }
            let selected = super::reference_outputs::source(selection, &self.reference_inputs)?;
            let types = execution.structural_types();
            let terminal_psi::StructuralTypeShape::Reference { access, .. } = types
                .get(&result.structural_type)
                .ok_or_else(malformed)?
                .shape
            else {
                return Err(malformed());
            };
            if selection.access != access
                || !crate::storage::reference::access_covers(selected.access, access)
            {
                return Err(malformed());
            }
            let relative = terminal_semantics::canonical_structural_storage_path(
                types.values(),
                selected.referent.structural_type,
                &selection.path,
            )
            .ok_or_else(malformed)?;
            let allowed = declaration.result_reference_regions.iter().any(|region| {
                if !region.path.is_empty()
                    || region.access != access
                    || region.origin
                        != (terminal_psi::TerminalDynamicReferenceOrigin::Parameter {
                            argument_index: selected.argument_index as u32,
                        })
                {
                    return false;
                }
                let prefix = terminal_semantics::reference_source_storage_path(
                    types.values(),
                    selected.referent.structural_type,
                    &region.source_path,
                );
                relative.starts_with(prefix)
            });
            if !allowed {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "host reference result exceeds its public input regions",
                ));
            }
            if let Some(binding) = self
                .structural
                .iter()
                .find(|binding| binding.argument_index == selected.argument_index)
            {
                let leaves = binding.completed_leaves.as_deref().ok_or_else(malformed)?;
                let prefix_end = relative
                    .iter()
                    .position(|step| matches!(step, StructuralPathSegment::Referent))
                    .unwrap_or(relative.len());
                let active = crate::boundary::structural_inputs::case_membership::case_projection_with_membership_is_active(
                &types, selected.referent.structural_type, &relative[..prefix_end],
                &|prefix| leaves.iter().find_map(|leaf| match leaf.kind {
                    StorageLeafKind::Case { case, .. } => {
                        let path = terminal_semantics::canonical_structural_storage_path(
                            types.values(), selected.referent.structural_type, &leaf.path)?;
                        (path == prefix).then_some(case)
                    }
                    _ => None,
                }))?;
                if !active {
                    return Err(malformed());
                }
            }
            let mut argument = selected.source_argument.clone();
            argument.path.extend(relative);
            argument.access = access;
            let actual =
                execution.resolve_live_projection(argument.place, &argument.path, access)?;
            if selected.access != StructuralAccess::SharedBorrow {
                let normal = self
                    .structural
                    .iter()
                    .find(|binding| binding.argument_index == selected.argument_index)
                    .map_or(&[][..], |binding| binding.normal_qualifications.as_slice());
                if !execution.normal_qualifications_cover(
                    &actual,
                    &actual.qualifications,
                    &actual.projected_qualifications,
                    normal,
                )? {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "host reference result lacks exact normal qualification preservation",
                    ));
                }
            }
            execution.capture_reference(
                &argument,
                result.structural_type,
                &result.projected_qualifications,
                None,
                identity_cursor,
            )
        }
    }
}
pub(crate) mod structural_buffer {
    use crate::boundary::effects::TerminalEffectRejection;
    use crate::boundary::effects::original_storage::TerminalOriginalStorageLoan;
    use crate::storage::structural_storage::contents::TerminalBoundaryStorageValue;
    use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, StructuralTypeId};
    use std::sync::Arc;
    use terminal_psi::{StructuralAccess, StructuralPathSegment};

    #[derive(Debug)]
    pub struct TerminalBoundaryStructuralBuffer {
        pub(crate) original_storage: Option<TerminalOriginalStorageLoan>,
        pub(crate) identity: Arc<()>,
        pub(crate) argument_index: usize,
        pub(crate) access: StructuralAccess,
        pub(crate) structural_type: StructuralTypeId,
        pub(crate) values: Vec<TerminalBoundaryStorageValue>,
        pub(crate) references: Vec<BoundaryReferenceReplacement>,
    }

    #[derive(Debug, Clone)]
    pub struct TerminalBoundaryReferenceSelection {
        pub(crate) identity: Arc<()>,
        pub(crate) argument_index: usize,
        pub(crate) path: Vec<Path>,
        pub(crate) access: StructuralAccess,
    }

    impl PartialEq for TerminalBoundaryReferenceSelection {
        fn eq(&self, other: &Self) -> bool {
            Arc::ptr_eq(&self.identity, &other.identity)
                && self.argument_index == other.argument_index
                && self.path == other.path
                && self.access == other.access
        }
    }
    impl Eq for TerminalBoundaryReferenceSelection {}

    #[derive(Debug)]
    pub(crate) struct BoundaryReferenceReplacement {
        pub path: Vec<StructuralPathSegment>,
        pub source: TerminalBoundaryReferenceSelection,
        pub write: ReferenceOutputWrite,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum ReferenceOutputWrite {
        Descriptor,
        Address,
    }

    impl TerminalBoundaryStructuralBuffer {
        pub fn original_storage(&self) -> Option<&TerminalOriginalStorageLoan> {
            self.original_storage.as_ref()
        }
        pub fn argument_index(&self) -> usize {
            self.argument_index
        }
        pub fn access(&self) -> StructuralAccess {
            self.access
        }
        pub fn structural_type(&self) -> StructuralTypeId {
            self.structural_type
        }
        pub fn values(&self) -> &[TerminalBoundaryStorageValue] {
            &self.values
        }

        pub fn replace_reference(
            &mut self,
            path: &[StructuralPathSegment],
            source: TerminalBoundaryReferenceSelection,
        ) -> Result<(), TerminalEffectRejection> {
            self.stage_reference_replacement(path, source, ReferenceOutputWrite::Descriptor)
        }

        pub fn replace_reference_address(
            &mut self,
            path: &[StructuralPathSegment],
            source: TerminalBoundaryReferenceSelection,
        ) -> Result<(), TerminalEffectRejection> {
            self.stage_reference_replacement(path, source, ReferenceOutputWrite::Address)
        }

        fn stage_reference_replacement(
            &mut self,
            path: &[StructuralPathSegment],
            source: TerminalBoundaryReferenceSelection,
            write: ReferenceOutputWrite,
        ) -> Result<(), TerminalEffectRejection> {
            if self.access != StructuralAccess::MutableBorrow
                || self
                    .references
                    .iter()
                    .any(|replacement| replacement.path == path)
            {
                return Err(TerminalEffectRejection::new(
                    "reference output is not one unique mutable slot",
                ));
            }
            self.references.push(BoundaryReferenceReplacement {
                path: path.to_vec(),
                source,
                write,
            });
            Ok(())
        }

        pub fn replace(
            &mut self,
            values: Vec<TerminalBoundaryStorageValue>,
        ) -> Result<(), TerminalEffectRejection> {
            if self.access != StructuralAccess::MutableBorrow {
                return Err(TerminalEffectRejection::new(
                    "owned or shared boundary storage cannot be replaced",
                ));
            }
            self.values = values;
            Ok(())
        }
    }
}

use crate::boundary::effects::scalar_loans::{BoundaryScalarLoans, TerminalBoundaryScalarBuffer};
use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::boundary::values::TerminalStructuralValue;
use crate::engine::execution::TerminalExecution;
use crate::storage::record::storage::{ConstructorStorage, PreparedConstructorReplacement};
use crate::storage::structural_storage::contents::{self, TerminalBoundaryStorageValue};
use std::sync::Arc;
use terminal_psi::{StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration};

pub use byte_views::TerminalBoundaryByteViewBuffer;
pub use structural_buffer::{TerminalBoundaryReferenceSelection, TerminalBoundaryStructuralBuffer};

#[derive(Debug)]
pub enum TerminalBoundaryStorageBuffer {
    Scalar(TerminalBoundaryScalarBuffer),
    Structural(TerminalBoundaryStructuralBuffer),
    ByteView(TerminalBoundaryByteViewBuffer),
}

impl TerminalBoundaryStorageBuffer {
    pub fn select_reference(
        &self,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        access: terminal_psi::StructuralAccess,
    ) -> Result<TerminalBoundaryReferenceSelection, crate::boundary::effects::TerminalEffectRejection>
    {
        let (identity, argument_index, available) = match self {
            Self::ByteView(_) => {
                return Err(crate::boundary::effects::TerminalEffectRejection::new(
                    "byte-view input cannot select an untyped reference output",
                ));
            }
            Self::Scalar(buffer) => (
                buffer.capture_identity(),
                buffer.argument_index(),
                buffer.access(),
            ),
            Self::Structural(buffer) => (
                Arc::clone(&buffer.identity),
                buffer.argument_index,
                buffer.access,
            ),
        };
        if !crate::storage::reference::access_covers(available, access)
            || !matches!(
                access,
                terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            )
        {
            return Err(crate::boundary::effects::TerminalEffectRejection::new(
                "reference selection exceeds captured input access",
            ));
        }
        Ok(TerminalBoundaryReferenceSelection {
            identity,
            argument_index,
            path: path.to_vec(),
            access,
        })
    }
}

pub(crate) struct StructuralLoanBinding {
    direct_original: bool,
    identity: Arc<()>,
    argument_index: usize,
    source: semantic_vocabulary::PlaceId,
    source_path: Vec<terminal_psi::StructuralPathSegment>,
    access: terminal_psi::StructuralAccess,
    referent: TerminalStructuralValue,
    aggregate: Option<crate::storage::reference::AggregateBacking>,
    normal_qualifications:
        Vec<crate::boundary::effects::normal_qualifications::CapturedNormalQualification>,
    original: Vec<TerminalBoundaryStorageValue>,
    leaves: Vec<contents::StorageLeaf>,
    completed_leaves: Option<Vec<contents::StorageLeaf>>,
    scalars: Vec<contents::PinnedScalar>,
    replacement: Option<PreparedConstructorReplacement>,
    reference_replacement: Option<ConstructorStorage>,
    stores: Vec<(
        crate::storage::primitive_storage::PrimitiveStorage,
        semantic_vocabulary::ScalarType,
        crate::boundary::values::TerminalScalarValue,
    )>,
}

struct OwnedPayloadSnapshot {
    identity: Arc<()>,
    argument_index: usize,
    structural_type: semantic_vocabulary::StructuralTypeId,
    values: Vec<TerminalBoundaryStorageValue>,
}

#[derive(Default)]
pub(crate) struct BoundaryStorageInputs {
    pub(crate) buffers: Vec<TerminalBoundaryStorageBuffer>,
    scalars: BoundaryScalarLoans,
    structural: Vec<StructuralLoanBinding>,
    owned: Vec<OwnedPayloadSnapshot>,
    reference_inputs: Vec<CapturedReferenceInput>,
    byte_views: Vec<byte_views::BoundaryByteViewBinding>,
}

pub(crate) struct CapturedReferenceInput {
    identity: Arc<()>,
    argument_index: usize,
    source_argument: StructuralArgument,
    referent: TerminalStructuralValue,
    access: terminal_psi::StructuralAccess,
}

impl BoundaryStorageInputs {
    pub(crate) fn is_empty(&self) -> bool {
        self.buffers.is_empty()
            && self.scalars.is_empty()
            && self.structural.is_empty()
            && self.owned.is_empty()
            && self.reference_inputs.is_empty()
            && self.byte_views.is_empty()
    }

    pub(crate) fn capture_scalar(
        &mut self,
        execution: &TerminalExecution,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
        argument_index: usize,
        identity_cursor: &mut Option<u64>,
    ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
        let referent = self.scalars.capture(
            execution,
            parameter,
            argument,
            argument_index,
            identity_cursor,
        )?;
        let buffer = self.scalars.buffers.pop().ok_or_else(malformed)?;
        self.buffers
            .push(TerminalBoundaryStorageBuffer::Scalar(buffer));
        Ok(referent)
    }

    pub(crate) fn capture_fields(
        &mut self,
        execution: &TerminalExecution,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
        referent: &TerminalStructuralValue,
        array_loan: Option<&crate::storage::element_views::ElementView>,
        aggregate: Option<&crate::storage::reference::AggregateBacking>,
        argument_index: usize,
        normal_guarantees: &[terminal_psi::StructuralDomainRequirement],
    ) -> Result<(), TerminalInterpretError> {
        if parameter.access == terminal_psi::StructuralAccess::Owned {
            if argument.access != parameter.access
                || parameter.structural_type != referent.structural_type
            {
                return Err(malformed());
            }
            let leaves = contents::describe(execution, referent, parameter.access, None)?.leaves;
            let (values, _) = contents::capture(
                execution,
                argument.place,
                referent,
                array_loan,
                aggregate,
                parameter.access,
                &leaves,
                &[],
            )?;
            let identity = Arc::new(());
            self.buffers.push(TerminalBoundaryStorageBuffer::Structural(
                TerminalBoundaryStructuralBuffer {
                    original_storage: None,
                    identity: Arc::clone(&identity),
                    argument_index,
                    access: parameter.access,
                    structural_type: referent.structural_type,
                    values: values.clone(),
                    references: Vec::new(),
                },
            ));
            self.owned.push(OwnedPayloadSnapshot {
                identity,
                argument_index,
                structural_type: referent.structural_type,
                values,
            });
            return Ok(());
        }
        if parameter.access != argument.access
            || parameter.structural_type != referent.structural_type
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
        {
            return Err(malformed());
        }
        let promised = normal_guarantees
            .iter()
            .filter(|guarantee| guarantee.argument_index as usize == argument_index)
            .map(|guarantee| terminal_psi::StructuralPathQualification {
                path: guarantee.path.clone(),
                domain: guarantee.domain,
            })
            .collect::<Vec<_>>();
        let normal_qualifications = execution.capture_normal_qualifications(referent, &promised)?;
        if parameter.access == terminal_psi::StructuralAccess::MutableBorrow {
            if !execution.normal_qualifications_cover(
                referent,
                &parameter.qualifications,
                &parameter.projected_qualifications,
                &normal_qualifications,
            )? || !execution.normal_qualifications_cover(
                referent,
                &referent.qualifications,
                &referent.projected_qualifications,
                &normal_qualifications,
            )? {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "mutable host storage lacks exact normal qualification preservation",
                ));
            }
            execution.validate_storage_loan_restoration(
                &crate::boundary::values::StructuralRuntimePlace::from(referent),
                &normal_qualifications,
            )?;
        }
        let leaves = contents::describe(execution, referent, parameter.access, None)?.leaves;
        let (original, scalars) = contents::capture(
            execution,
            argument.place,
            referent,
            array_loan,
            aggregate,
            parameter.access,
            &leaves,
            &normal_qualifications,
        )?;
        let identity = Arc::new(());
        self.buffers.push(TerminalBoundaryStorageBuffer::Structural(
            TerminalBoundaryStructuralBuffer {
                original_storage: None,
                identity: Arc::clone(&identity),
                argument_index,
                access: parameter.access,
                structural_type: referent.structural_type,
                values: original.clone(),
                references: Vec::new(),
            },
        ));
        self.structural.push(StructuralLoanBinding {
            direct_original: false,
            identity,
            argument_index,
            source: argument.place,
            source_path: argument.path.clone(),
            access: parameter.access,
            referent: referent.clone(),
            aggregate: aggregate.cloned(),
            normal_qualifications,
            original,
            leaves,
            completed_leaves: None,
            scalars,
            replacement: None,
            reference_replacement: None,
            stores: Vec::new(),
        });
        Ok(())
    }

    pub(crate) fn capture_reference_inputs(
        &mut self,
        declaration: &terminal_psi::BoundaryMachineDeclaration,
        arguments: &[StructuralArgument],
        values: &[TerminalStructuralValue],
    ) -> Result<(), TerminalInterpretError> {
        if declaration.reference_outputs.is_empty()
            && declaration.result_reference_regions.is_empty()
        {
            return Ok(());
        }
        if !self.reference_inputs.is_empty()
            || arguments.len() != declaration.structural_parameters.len()
            || values.len() != arguments.len()
        {
            return Err(malformed());
        }
        for buffer in &self.buffers {
            let (identity, argument_index, access) = match buffer {
                TerminalBoundaryStorageBuffer::ByteView(_) => {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "byte-view reference outputs require an exact descriptor lifetime contract",
                    ));
                }
                TerminalBoundaryStorageBuffer::Scalar(buffer) => (
                    buffer.capture_identity(),
                    buffer.argument_index(),
                    buffer.access(),
                ),
                TerminalBoundaryStorageBuffer::Structural(buffer) => (
                    Arc::clone(&buffer.identity),
                    buffer.argument_index,
                    buffer.access,
                ),
            };
            let parameter = declaration
                .structural_parameters
                .get(argument_index)
                .ok_or_else(malformed)?;
            let argument = arguments.get(argument_index).ok_or_else(malformed)?;
            let referent = values.get(argument_index).ok_or_else(malformed)?;
            if parameter.access != access
                || argument.access != access
                || parameter.structural_type != referent.structural_type
                || self
                    .reference_inputs
                    .iter()
                    .any(|capture| capture.argument_index == argument_index)
            {
                return Err(malformed());
            }
            self.reference_inputs.push(CapturedReferenceInput {
                identity,
                argument_index,
                source_argument: argument.clone(),
                referent: referent.clone(),
                access,
            });
        }
        Ok(())
    }

    pub(crate) fn validate_writeback(
        &mut self,
        execution: &TerminalExecution,
        declaration: &terminal_psi::BoundaryMachineDeclaration,
        identity_cursor: &mut Option<u64>,
    ) -> Result<(), TerminalInterpretError> {
        self.validate_byte_views(execution)?;
        let scalar_buffers: Vec<_> = self
            .buffers
            .iter()
            .filter_map(|buffer| match buffer {
                TerminalBoundaryStorageBuffer::Scalar(buffer) => Some(buffer),
                _ => None,
            })
            .collect();
        self.scalars
            .validate_writeback(execution, &scalar_buffers)?;
        let structural_buffers: Vec<_> = self
            .buffers
            .iter()
            .filter_map(|buffer| match buffer {
                TerminalBoundaryStorageBuffer::Structural(buffer)
                    if buffer.access != terminal_psi::StructuralAccess::Owned =>
                {
                    Some(buffer)
                }
                _ => None,
            })
            .collect();
        let owned_buffers: Vec<_> = self
            .buffers
            .iter()
            .filter_map(|buffer| match buffer {
                TerminalBoundaryStorageBuffer::Structural(buffer)
                    if buffer.access == terminal_psi::StructuralAccess::Owned =>
                {
                    Some(buffer)
                }
                _ => None,
            })
            .collect();
        if owned_buffers.len() != self.owned.len() {
            return Err(malformed());
        }
        for (buffer, original) in owned_buffers.into_iter().zip(&self.owned) {
            if !Arc::ptr_eq(&buffer.identity, &original.identity)
                || buffer.argument_index != original.argument_index
                || buffer.structural_type != original.structural_type
                || buffer.values != original.values
                || !buffer.references.is_empty()
            {
                return Err(malformed());
            }
        }
        if structural_buffers.len() != self.structural.len() {
            return Err(malformed());
        }
        for (buffer, binding) in structural_buffers.into_iter().zip(&mut self.structural) {
            borrowed_completion::stage(execution, buffer, binding)?;
        }
        reference_outputs::stage(
            execution,
            declaration,
            &self.buffers,
            &mut self.structural,
            &self.reference_inputs,
            identity_cursor,
        )?;
        Ok(())
    }

    pub(crate) fn commit(
        mut self,
        execution: &mut TerminalExecution,
    ) -> Result<(), TerminalInterpretError> {
        self.commit_byte_views(execution)?;
        let mut scalars = Vec::new();
        for buffer in self.buffers {
            match buffer {
                TerminalBoundaryStorageBuffer::Scalar(buffer) => scalars.push(buffer),
                TerminalBoundaryStorageBuffer::Structural(_)
                | TerminalBoundaryStorageBuffer::ByteView(_) => {}
            }
        }
        self.scalars.commit(execution, scalars)?;
        for binding in self.structural {
            borrowed_completion::commit(execution, binding)?;
        }
        Ok(())
    }
}
