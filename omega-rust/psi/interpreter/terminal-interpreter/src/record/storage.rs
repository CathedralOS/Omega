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
use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
use std::collections::BTreeMap;
use terminal_psi::{StructuralPathSegment, StructuralTypeShape};

pub(crate) enum ConstructorValue<'execution> {
    Placed(TerminalStructuralValue),
    ScalarArray(&'execution crate::scalar_array::TerminalScalarArrayValue),
}

impl<'execution> ConstructorValue<'execution> {
    pub(crate) fn resolve(
        execution: &'execution TerminalExecution,
        argument: &terminal_psi::StructuralArgument,
    ) -> Result<Self, TerminalInterpretError> {
        if let Some(array) = execution.scalar_array_values.get(&argument.place) {
            if !argument.path.is_empty()
                || execution.structural_values.contains_key(&argument.place)
            {
                return Err(malformed());
            }
            return Ok(Self::ScalarArray(array));
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
            Self::ScalarArray(value) => value.structural_type,
        }
    }

    pub(crate) fn qualifications(&self) -> &[StructuralDomainId] {
        match self {
            Self::Placed(value) => &value.qualifications,
            Self::ScalarArray(_) => &[],
        }
    }

    /// Relocate the selected value's actual memberships with its storage.
    /// Packed scalar arrays carry no nominal membership roster.
    pub(crate) fn append_memberships(
        &self,
        prefix: &[StructuralPathSegment],
        destination: &mut Vec<terminal_psi::StructuralPathQualification>,
    ) {
        if let Self::Placed(value) = self {
            terminal_psi::StructuralPathQualification::append_under(
                &value.qualifications,
                &value.projected_qualifications,
                prefix,
                destination,
            );
        }
    }
}

#[derive(Default)]
pub(crate) struct ConstructorStorage {
    scalars: BTreeMap<StructuralScalarRuntimeField, TerminalScalarValue>,
    bytes: BTreeMap<StructuralByteSequenceRuntimeField, ByteSequenceView>,
    views: BTreeMap<StructuralRuntimePlace, crate::element_views::ElementView>,
    references: BTreeMap<StructuralRuntimePlace, crate::reference::ReferenceReferent>,
    primitives: BTreeMap<StructuralRuntimePlace, TerminalScalarValue>,
    byte_arrays: BTreeMap<StructuralRuntimePlace, ByteSequenceView>,
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
    ) -> Result<(), TerminalInterpretError> {
        let cell = StructuralByteSequenceRuntimeField {
            parent: parent.clone(),
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
            ConstructorValue::ScalarArray(array) => {
                if transfers_custody {
                    return Err(malformed());
                }
                self.stage_array(
                    execution,
                    array.structural_type,
                    &array.elements,
                    destination,
                )
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
        self.stage_subtree(
            execution,
            &StructuralRuntimePlace::from(child),
            destination,
            transfers_custody,
        )
    }

    /// Constructors and owned argument snapshots relocate every typed cell,
    /// not only scalar fields. A snapshot may not duplicate loan custody.
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
        for (home, descriptor) in &execution.structural_element_view_fields {
            if let Some(target) = relocate(home) {
                self.views.insert(target, descriptor.clone());
            }
        }
        for (home, referent) in &execution.reference_referents {
            if let Some(target) = relocate(home) {
                if !transfers_custody {
                    return Err(malformed());
                }
                self.references.insert(target, referent.clone());
            }
        }
        for (field, scalar) in &execution.structural_scalar_fields {
            if let Some(parent) = relocate(&field.parent) {
                self.scalars.insert(field.relocated(parent), *scalar);
            }
        }
        for (field, bytes) in &execution.structural_byte_sequence_fields {
            if let Some(parent) = relocate(&field.parent) {
                self.bytes.insert(
                    StructuralByteSequenceRuntimeField {
                        parent,
                        field: field.field,
                    },
                    bytes.clone(),
                );
            }
        }
        for (home, value) in &execution.structural_primitive_storage {
            if let Some(target) = relocate(home) {
                self.primitives.insert(target, *value);
            }
        }
        for (home, bytes) in &execution.structural_byte_storage {
            if let Some(target) = relocate(home) {
                self.byte_arrays.insert(target, bytes.clone());
            }
        }
        for (home, case) in &execution.structural_cases {
            if let Some(target) = relocate(home) {
                self.cases.insert(target, *case);
            }
        }
        Ok(())
    }

    pub(crate) fn publish(self, execution: &mut TerminalExecution) {
        execution.structural_scalar_fields.extend(self.scalars);
        execution.structural_byte_sequence_fields.extend(self.bytes);
        execution.structural_element_view_fields.extend(self.views);
        execution.reference_referents.extend(self.references);
        execution
            .structural_primitive_storage
            .extend(self.primitives);
        execution.structural_byte_storage.extend(self.byte_arrays);
        execution.structural_cases.extend(self.cases);
    }
}
