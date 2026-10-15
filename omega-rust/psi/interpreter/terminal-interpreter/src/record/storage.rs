//! Constructor operands have one storage transaction, regardless of their
//! producer's payload representation. Array results retain packed values
//! rather than opaque homes; nesting publishes their contents in
//! the parent's typed subtree storage. Placed operands relocate that same
//! storage. No child mutates or publishes until the whole constructor's type,
//! qualification and custody checks succeed.
use crate::byte_sequences::ByteSequenceView;
use crate::custody::resolve_single_structural_argument;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::scalar_operations::terminal_scalar_belongs_to_type;
use crate::values::{
    StructuralByteSequenceRuntimeField, StructuralRuntimePlace, StructuralScalarRuntimeField,
};
use crate::values::{TerminalScalarValue, TerminalStructuralValue};
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeMap;
use terminal_psi::{StructuralPathSegment, StructuralTypeShape};

pub(crate) enum ConstructorValue<'execution> {
    Placed(TerminalStructuralValue),
    PackedScalars {
        structural_type: StructuralTypeId,
        elements: &'execution [TerminalScalarValue],
        qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
        projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
    },
}

impl<'execution> ConstructorValue<'execution> {
    pub(crate) fn resolve(
        execution: &'execution TerminalExecution,
        argument: &terminal_psi::StructuralArgument,
    ) -> Result<Self, TerminalInterpretError> {
        if let Some(array) = execution.scalar_array_values.get(&argument.place) {
            if execution.structural_values.contains_key(&argument.place) {
                return Err(malformed());
            }
            let types = execution.program.structural_types();
            let mut structural_type = array.structural_type;
            let mut elements = array.elements.as_slice();
            // A packed tree's static child is a contiguous row-major span.
            // Select the actual source span before any receiving declaration
            // is consulted; neither a referent nor byte window owns this data.
            for segment in &argument.path {
                let StructuralPathSegment::FixedIndex(index) = segment else {
                    return Err(malformed());
                };
                let StructuralTypeShape::FixedArray { element, length } =
                    types.get(&structural_type).ok_or_else(malformed)?.shape
                else {
                    return Err(malformed());
                };
                if *index >= length {
                    return Err(malformed());
                }
                let length = usize::try_from(length).map_err(|_| malformed())?;
                if elements.len() % length != 0 {
                    return Err(malformed());
                }
                let count = elements.len() / length;
                let start = usize::try_from(*index)
                    .map_err(|_| malformed())?
                    .checked_mul(count)
                    .ok_or_else(malformed)?;
                elements = elements
                    .get(start..start.checked_add(count).ok_or_else(malformed)?)
                    .ok_or_else(malformed)?;
                structural_type = element;
            }
            let (qualifications, projected_qualifications) =
                terminal_psi::StructuralPathQualification::select(
                    &array.qualifications,
                    &[],
                    &argument.path,
                );
            return Ok(Self::PackedScalars {
                structural_type,
                elements,
                qualifications,
                projected_qualifications,
            });
        }
        resolve_single_structural_argument(
            &execution.program.structural_types(),
            &execution.structural_values,
            argument,
        )
        .map(Self::Placed)
    }

    pub(crate) fn structural_type(&self) -> StructuralTypeId {
        match self {
            Self::Placed(value) => value.structural_type,
            Self::PackedScalars {
                structural_type, ..
            } => *structural_type,
        }
    }

    /// Snapshot a scalar tree and resolve its receiving cells in the original
    /// owner before any store. Packed arrays and placed values differ only in
    /// how their completed payload is read, never in where replacement writes.
    /// Non-scalar subtrees retain their descriptor/case transplant transaction.
    pub(crate) fn replacement_writes(
        &self,
        execution: &TerminalExecution,
        source: &terminal_psi::StructuralArgument,
        destination: semantic_vocabulary::PlaceId,
        destination_path: &[StructuralPathSegment],
    ) -> Result<Option<crate::primitive_storage::PrimitiveStorageWrites>, TerminalInterpretError>
    {
        let mut writes = crate::primitive_storage::PrimitiveStorageWrites::default();
        let supported =
            self.visit_scalar_leaves(execution, source, |selectors, scalar_type, value| {
                let mut path = destination_path.to_vec();
                path.extend_from_slice(selectors);
                let (storage, actual, _) = execution.primitive_access(destination, true, &path)?;
                if actual != scalar_type {
                    return Err(malformed());
                }
                writes.stage(execution, destination, storage, scalar_type, value)
            })?;
        Ok(supported.map(|_| writes))
    }

    /// Read the completed scalar leaves once in row-major order. Consumers
    /// stage copies or writes without reconstructing producer-specific storage.
    pub(crate) fn visit_scalar_leaves(
        &self,
        execution: &TerminalExecution,
        source: &terminal_psi::StructuralArgument,
        mut visit: impl FnMut(
            &[StructuralPathSegment],
            semantic_vocabulary::ScalarType,
            TerminalScalarValue,
        ) -> Result<(), TerminalInterpretError>,
    ) -> Result<Option<semantic_vocabulary::ScalarType>, TerminalInterpretError> {
        let types = execution.program.structural_types();
        let root = self.structural_type();
        let (scalar_type, count) = match types.get(&root).ok_or_else(malformed)?.shape {
            StructuralTypeShape::PrimitiveScalar(scalar_type) => (scalar_type, 1),
            StructuralTypeShape::FixedArray { .. } => {
                let Some(shape) = terminal_semantics::scalar_array_leaf_shape(types.values(), root)
                else {
                    return Ok(None);
                };
                shape
            }
            _ => return Ok(None),
        };
        if let Self::PackedScalars { elements, .. } = self
            && u64::try_from(elements.len()).ok() != Some(count)
        {
            return Err(malformed());
        }
        // The shape query validates every dimension, including below a zero
        // extent. Enumerating leaves instead of nested dimensions avoids work
        // proportional to a huge outer array whose inner arrays are empty.
        let mut dimensions = Vec::new();
        let mut current = root;
        while let StructuralTypeShape::FixedArray { element, length } =
            types.get(&current).ok_or_else(malformed)?.shape
        {
            dimensions.push(length);
            current = element;
        }
        let mut selectors = vec![0; dimensions.len()];
        for leaf_ordinal in 0..count {
            let mut remaining = leaf_ordinal;
            for (selector, length) in selectors.iter_mut().zip(&dimensions).rev() {
                *selector = remaining.checked_rem(*length).ok_or_else(malformed)?;
                remaining = remaining.checked_div(*length).ok_or_else(malformed)?;
            }
            let value = match self {
                Self::PackedScalars { elements, .. } => *elements
                    .get(usize::try_from(leaf_ordinal).map_err(|_| malformed())?)
                    .ok_or_else(malformed)?,
                Self::Placed(value) => {
                    let (storage, actual, shared) = if dimensions.is_empty() {
                        // An owned primitive parameter is a completed payload,
                        // not an ordinary whole-root borrowing operation.
                        execution.primitive_view_access(value, &[], scalar_type, false)?
                    } else {
                        // A borrowed array can be presented by opaque metadata
                        // while its bytes remain in a suspended caller. Keep
                        // the actual operand's home/path to follow that loan.
                        let mut path = source.path.clone();
                        path.extend(
                            selectors
                                .iter()
                                .copied()
                                .map(StructuralPathSegment::FixedIndex),
                        );
                        execution.primitive_access(source.place, false, &path)?
                    };
                    execution.read_primitive_storage(
                        source.place,
                        storage,
                        actual,
                        scalar_type,
                        shared,
                    )?
                }
            };
            if value.scalar_type() != scalar_type || !terminal_scalar_belongs_to_type(value) {
                return Err(malformed());
            }
            let path = selectors
                .iter()
                .copied()
                .map(StructuralPathSegment::FixedIndex)
                .collect::<Vec<_>>();
            visit(&path, scalar_type, value)?;
        }
        Ok(Some(scalar_type))
    }

    pub(crate) fn memberships(
        &self,
    ) -> (
        &[semantic_vocabulary::StructuralDomainId],
        &[terminal_psi::StructuralPathQualification],
    ) {
        match self {
            Self::Placed(value) => (&value.qualifications, &value.projected_qualifications),
            Self::PackedScalars {
                qualifications,
                projected_qualifications,
                ..
            } => (qualifications, projected_qualifications),
        }
    }

    /// Relocate the selected value's actual memberships with its storage.
    /// Packed arrays retain the predicate memberships proven on their bytes.
    pub(crate) fn append_memberships(
        &self,
        prefix: &[StructuralPathSegment],
        destination: &mut Vec<terminal_psi::StructuralPathQualification>,
    ) {
        let (whole, projected) = self.memberships();
        terminal_psi::StructuralPathQualification::append_under(
            whole,
            projected,
            prefix,
            destination,
        );
    }
}

#[derive(Default)]
pub(crate) struct ConstructorStorage {
    scalars: BTreeMap<StructuralScalarRuntimeField, TerminalScalarValue>,
    bytes: BTreeMap<
        StructuralByteSequenceRuntimeField,
        crate::byte_sequences::binding::ByteSequenceBinding,
    >,
    views: BTreeMap<StructuralRuntimePlace, crate::element_views::ElementView>,
    references: BTreeMap<StructuralRuntimePlace, crate::reference::ReferenceReferent>,
    primitives: BTreeMap<StructuralRuntimePlace, TerminalScalarValue>,
    byte_arrays: BTreeMap<StructuralRuntimePlace, ByteSequenceView>,
    owner_writes: Vec<(
        crate::byte_sequences::binding::ByteOwnerHome,
        ByteSequenceView,
    )>,
    cases: BTreeMap<
        StructuralRuntimePlace,
        crate::structural_inputs::case_membership::StructuralCaseContents,
    >,
}

impl ConstructorStorage {
    /// Custody is staged against the same unpublished tags as the new bytes.
    pub(crate) fn case_at_projection(
        &self,
        execution: &TerminalExecution,
        root: &TerminalStructuralValue,
        path: &[StructuralPathSegment],
    ) -> Option<semantic_vocabulary::StructuralCaseId> {
        crate::structural_inputs::case_membership::stored_case_at_projection(
            &execution.program.structural_types(),
            root,
            &self.cases,
            path,
        )
    }

    pub(crate) fn stage_byte_field(
        &mut self,
        destination: StructuralByteSequenceRuntimeField,
        bytes: crate::byte_sequences::binding::ByteSequenceBinding,
    ) {
        self.bytes.insert(destination, bytes);
    }

    pub(crate) fn stage_byte_array(&mut self, destination: StructuralRuntimePlace, bytes: Vec<u8>) {
        // Byte arrays have one backing, never duplicate primitive cells. This
        // uses the same publication shape as constructor array establishment.
        self.primitives.retain(|home, _| {
            home.opaque_identity != destination.opaque_identity
                || !home.path.starts_with(&destination.path)
        });
        self.byte_arrays
            .insert(destination, ByteSequenceView::new(bytes));
    }
    /// Completion reseats this exact root immediately after pruning it. Retained
    /// permission carriers never gain a second live identity or transfer custody.
    pub(crate) fn retain_replacement_subtree(
        &mut self,
        execution: &TerminalExecution,
        root: &StructuralRuntimePlace,
    ) -> Result<(), TerminalInterpretError> {
        self.stage_subtree(execution, root, root, true)
    }

    pub(crate) fn retain_byte_field(
        &mut self,
        execution: &TerminalExecution,
        parent: &StructuralRuntimePlace,
        field: semantic_vocabulary::StructuralFieldId,
        namespace: crate::values::StructuralFieldNamespace,
    ) -> Result<(), TerminalInterpretError> {
        let cell = StructuralByteSequenceRuntimeField {
            parent: parent.clone(),
            namespace,
            field,
        };
        let bytes = execution
            .structural_byte_sequence_fields
            .get(&cell)
            .ok_or_else(malformed)?;
        self.bytes.insert(cell, bytes.clone());
        Ok(())
    }

    pub(crate) fn stage_case(
        &mut self,
        destination: StructuralRuntimePlace,
        structural_type: StructuralTypeId,
        case: semantic_vocabulary::StructuralCaseId,
    ) {
        self.cases.insert(
            destination,
            crate::structural_inputs::case_membership::StructuralCaseContents {
                structural_type,
                case,
            },
        );
    }

    pub(crate) fn stage_scalar_endpoint(
        &mut self,
        destination: &crate::primitive_storage::PrimitiveStorageEndpoint,
        value: TerminalScalarValue,
    ) -> Result<(), TerminalInterpretError> {
        match destination {
            crate::primitive_storage::PrimitiveStorageEndpoint::Scalar(home) => {
                self.primitives.insert(home.clone(), value);
            }
            crate::primitive_storage::PrimitiveStorageEndpoint::ScalarField(field) => {
                self.scalars.insert(field.clone(), value);
            }
            _ => return Err(malformed()),
        }
        Ok(())
    }
    pub(crate) fn stage(
        &mut self,
        execution: &TerminalExecution,
        argument: &terminal_psi::StructuralArgument,
        child: &ConstructorValue<'_>,
        destination: &StructuralRuntimePlace,
        transfers_custody: bool,
    ) -> Result<(), TerminalInterpretError> {
        match child {
            ConstructorValue::Placed(value) => {
                self.stage_placed(execution, argument, value, destination, transfers_custody)
            }
            ConstructorValue::PackedScalars {
                structural_type,
                elements,
                ..
            } => {
                if transfers_custody {
                    return Err(malformed());
                }
                self.stage_array(execution, *structural_type, elements, destination)
            }
        }
    }

    /// Row-major array payloads become the ordinary typed leaf storage used
    /// by reads, writes and borrowed descriptors. Byte arrays keep one byte
    /// backing, not both byte and primitive cells for the same leaves.
    fn stage_array(
        &mut self,
        execution: &TerminalExecution,
        structural_type: StructuralTypeId,
        elements: &[TerminalScalarValue],
        destination: &StructuralRuntimePlace,
    ) -> Result<(), TerminalInterpretError> {
        let declaration = execution
            .program
            .structural_types()
            .get(&structural_type)
            .ok_or_else(malformed)?;
        match declaration.shape {
            StructuralTypeShape::PrimitiveScalar(scalar_type) => {
                let [value] = elements else {
                    return Err(malformed());
                };
                if value.scalar_type() != scalar_type || !terminal_scalar_belongs_to_type(*value) {
                    return Err(malformed());
                }
                self.primitives.insert(destination.clone(), *value);
            }
            StructuralTypeShape::FixedArray { element, length } => {
                let (scalar_type, leaf_count) = terminal_semantics::scalar_array_leaf_shape(
                    execution.program.structural_types().values(),
                    structural_type,
                )
                .ok_or_else(malformed)?;
                if u64::try_from(elements.len()).ok() != Some(leaf_count) {
                    return Err(malformed());
                }
                if matches!(
                    execution
                        .program
                        .structural_types()
                        .get(&element)
                        .map(|row| &row.shape),
                    Some(StructuralTypeShape::PrimitiveScalar(_))
                ) && matches!(scalar_type, semantic_vocabulary::ScalarType::Integer(carrier)
                        if carrier.bits() == 8 && !carrier.is_address()
                            && carrier.sign() == semantic_vocabulary::IntegerSign::Unsigned)
                {
                    let bytes = elements
                        .iter()
                        .map(|value| {
                            if value.scalar_type() != scalar_type {
                                return Err(malformed());
                            }
                            let TerminalScalarValue::Integer {
                                value: semantic_vocabulary::IntegerValue::Unsigned(byte),
                                ..
                            } = value
                            else {
                                return Err(malformed());
                            };
                            u8::try_from(*byte).map_err(|_| malformed())
                        })
                        .collect::<Result<Vec<_>, TerminalInterpretError>>()?;
                    self.byte_arrays
                        .insert(destination.clone(), ByteSequenceView::new(bytes));
                } else {
                    let element_count = match execution
                        .program
                        .structural_types()
                        .get(&element)
                        .map(|row| &row.shape)
                    {
                        Some(StructuralTypeShape::PrimitiveScalar(_)) => 1,
                        _ => {
                            terminal_semantics::scalar_array_leaf_shape(
                                execution.program.structural_types().values(),
                                element,
                            )
                            .ok_or_else(malformed)?
                            .1
                        }
                    };
                    for element_index in 0..length {
                        let start = usize::try_from(
                            element_index
                                .checked_mul(element_count)
                                .ok_or_else(malformed)?,
                        )
                        .map_err(|_| malformed())?;
                        let end = start
                            .checked_add(usize::try_from(element_count).map_err(|_| malformed())?)
                            .ok_or_else(malformed)?;
                        let mut home = destination.clone();
                        home.path
                            .push(StructuralPathSegment::FixedIndex(element_index));
                        self.stage_array(
                            execution,
                            element,
                            elements.get(start..end).ok_or_else(malformed)?,
                            &home,
                        )?;
                    }
                }
            }
            _ => return Err(malformed()),
        }
        Ok(())
    }

    fn stage_placed(
        &mut self,
        execution: &TerminalExecution,
        argument: &terminal_psi::StructuralArgument,
        child: &TerminalStructuralValue,
        destination: &StructuralRuntimePlace,
        transfers_custody: bool,
    ) -> Result<(), TerminalInterpretError> {
        if execution
            .program
            .structural_types()
            .get(&child.structural_type)
            .is_some_and(|row| matches!(row.shape, StructuralTypeShape::ElementView { .. }))
        {
            self.views.insert(
                destination.clone(),
                execution.element_view_argument(argument, child)?.clone(),
            );
        }
        if execution
            .program
            .structural_types()
            .get(&child.structural_type)
            .is_some_and(|row| row.shape.is_shared_readable_byte_view())
        {
            let bytes = execution.shared_byte_descriptor(argument, child)?;
            self.byte_arrays
                .insert(destination.clone(), bytes.detached_descriptor());
        }
        if execution
            .program
            .structural_types()
            .get(&child.structural_type)
            .is_some_and(|row| row.shape.is_bounded_owned_byte_sequence())
        {
            let source = terminal_psi::StructuralArgument {
                access: terminal_psi::StructuralAccess::SharedBorrow,
                ..argument.clone()
            };
            let (_, bytes) = execution.bounded_owner_snapshot(&source)?;
            // Replacement reaches the captured owner's actual storage, just
            // as byte reads and writes do. A field is not a standalone opaque
            // cell, and a suspended caller's local stays in its pinned frame.
            match execution.captured_byte_owner_binding_at(destination, child.structural_type) {
                Some(crate::byte_sequences::binding::ByteSequenceBinding::MutableField {
                    field,
                    ..
                }) => {
                    self.bytes.insert(
                        field.clone(),
                        crate::byte_sequences::binding::ByteSequenceBinding::Immutable(
                            bytes.detached_descriptor(),
                        ),
                    );
                }
                Some(crate::byte_sequences::binding::ByteSequenceBinding::MutableOwner {
                    home,
                    ..
                }) => {
                    execution.byte_owner_bytes(home)?;
                    self.owner_writes
                        .push((home.clone(), bytes.detached_descriptor()));
                }
                Some(_) => return Err(malformed()),
                None => {
                    self.byte_arrays
                        .insert(destination.clone(), bytes.detached_descriptor());
                }
            }
        }
        self.stage_subtree(
            execution,
            &StructuralRuntimePlace::from(child),
            destination,
            transfers_custody,
        )
    }

    /// Constructors and owned argument snapshots relocate every typed cell,
    /// not only scalar fields. A snapshot may copy shared descriptors but may
    /// not duplicate exclusive permission. Physical paths use stored case
    /// membership to recover each resident descriptor's actual declaration.
    pub(crate) fn stage_subtree(
        &mut self,
        execution: &TerminalExecution,
        source: &StructuralRuntimePlace,
        destination: &StructuralRuntimePlace,
        transfers_custody: bool,
    ) -> Result<(), TerminalInterpretError> {
        let relocate = |home: &StructuralRuntimePlace| {
            if home.opaque_identity != source.opaque_identity
                || !home.path.starts_with(&source.path)
            {
                return None;
            }
            let mut path = destination.path.clone();
            path.extend_from_slice(&home.path[source.path.len()..]);
            Some(StructuralRuntimePlace {
                opaque_identity: destination.opaque_identity,
                path,
            })
        };
        for (home, descriptor) in
            crate::structural_storage::cells(&execution.structural_element_view_fields, source)
        {
            if let Some(target) = relocate(home) {
                self.views.insert(target, descriptor.clone());
            }
        }
        for (home, referent) in
            crate::structural_storage::cells(&execution.reference_referents, source)
        {
            if let Some(target) = relocate(home) {
                if !transfers_custody {
                    let carrier_type = execution.actual_storage_type_from_owner(home)?;
                    if !execution
                        .program
                        .structural_types()
                        .get(&carrier_type)
                        .is_some_and(|declaration| {
                            matches!(
                                declaration.shape,
                                StructuralTypeShape::Reference {
                                    access: terminal_psi::StructuralAccess::SharedBorrow,
                                    ..
                                }
                            )
                        })
                    {
                        return Err(malformed());
                    }
                }
                self.references.insert(target, referent.clone());
            }
        }
        for (field, scalar) in
            crate::structural_storage::cells(&execution.structural_scalar_fields, source)
        {
            if let Some(parent) = relocate(&field.parent) {
                self.scalars.insert(field.relocated(parent), *scalar);
            }
        }
        for (field, bytes) in
            crate::structural_storage::cells(&execution.structural_byte_sequence_fields, source)
        {
            if let Some(parent) = relocate(&field.parent) {
                if !transfers_custody
                    && !matches!(
                        bytes,
                        crate::byte_sequences::binding::ByteSequenceBinding::Immutable(_)
                    )
                {
                    return Err(malformed());
                }
                self.bytes.insert(field.relocated(parent), bytes.clone());
            }
        }
        for (home, value) in
            crate::structural_storage::cells(&execution.structural_primitive_storage, source)
        {
            if let Some(target) = relocate(home) {
                self.primitives.insert(target, *value);
            }
        }
        for (home, bytes) in
            crate::structural_storage::cells(&execution.structural_byte_storage, source)
        {
            if let Some(target) = relocate(home) {
                self.byte_arrays.insert(target, bytes.clone());
            }
        }
        for (home, case) in crate::structural_storage::cells(&execution.structural_cases, source) {
            if let Some(target) = relocate(home) {
                self.cases.insert(target, *case);
            }
        }
        Ok(())
    }

    pub(crate) fn publish(
        self,
        execution: &mut TerminalExecution,
    ) -> Result<(), TerminalInterpretError> {
        // Validate every pinned home before publishing any part of the
        // transaction. None of these writes retires another owner's frame.
        for (home, _) in &self.owner_writes {
            execution.byte_owner_bytes(home)?;
        }
        for (home, bytes) in self.owner_writes {
            *execution.byte_owner_bytes_mut(&home)? = bytes;
        }
        execution.structural_scalar_fields.extend(self.scalars);
        execution.structural_byte_sequence_fields.extend(self.bytes);
        execution.structural_element_view_fields.extend(self.views);
        execution.reference_referents.extend(self.references);
        execution
            .structural_primitive_storage
            .extend(self.primitives);
        execution.structural_byte_storage.extend(self.byte_arrays);
        execution.structural_cases.extend(self.cases);
        Ok(())
    }
}
