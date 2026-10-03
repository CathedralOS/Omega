//! Non-observing bounded byte replacement with invocation-independent backing.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, PlaceId, StructuralFieldId};
use terminal_psi::{
    ByteSequenceCarrier, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralArgument, StructuralMultiplicity, StructuralPathSegment, StructuralTypeShape,
};

use crate::custody::{project_structural_value, resolve_structural_arguments};
use crate::errors::TerminalInterpretError;
use crate::execution::TerminalExecution;
use crate::values::StructuralByteSequenceRuntimeField;
use crate::values::StructuralRuntimePlace;
use crate::values::TerminalScalarValue;

impl TerminalExecution {
    /// Observe the current live bytes at an original structural referent and
    /// carrier path. Unused capacity is not part of the returned sequence.
    pub fn structural_byte_sequence_field(
        &self,
        opaque_identity: u64,
        path: &[StructuralPathSegment],
        field: StructuralFieldId,
    ) -> Option<&[u8]> {
        self.structural_byte_sequence_fields
            .get(&StructuralByteSequenceRuntimeField {
                parent: StructuralRuntimePlace {
                    opaque_identity,
                    path: path.to_vec(),
                },
                field,
            })
            .map(|view| view.bytes())
    }

    /// Dispatch charges fuel first. Validate and snapshot the complete source
    /// before replacing either live bytes or length; no failure partially writes.
    pub(crate) fn execute_structural_byte_sequence_field_store(
        &mut self,
        operation: &Operation,
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let OperationKind::StructuralByteSequenceFieldStore {
            destination,
            path,
            field,
            source,
            length,
            ..
        } = &operation.kind
        else {
            return Err(invalid());
        };
        if operation.result != OperationResult::Unit || destination == source {
            return Err(invalid());
        }
        let (destination_field, capacity) =
            self.resolve_structural_byte_sequence_field(*destination, path, *field, true)?;
        if self
            .live_claims
            .values()
            .any(|claim| claim.place == Some(*source))
        {
            return Err(invalid());
        }
        let TerminalScalarValue::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(length),
        } = self.values.get(length).ok_or_else(invalid)?
        else {
            return Err(invalid());
        };
        if *scalar_type != IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?
            || *length > u128::from(capacity)
        {
            return Err(invalid());
        }
        let source_value = self.structural_values.get(source).ok_or_else(invalid)?;
        if !source_value.path.is_empty()
            || !source_value.qualifications.is_empty()
            || !matches!(self.program.structural_types.get(&source_value.structural_type), Some(declaration)
                            if declaration.shape.is_borrowed_byte_view()
                                || matches!(declaration.shape, StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. })))
        {
            return Err(invalid());
        }
        let bytes = self
            .byte_sequence_values
            .get(source)
            .ok_or_else(invalid)?
            .immutable()?;
        if bytes.len() as u128 != *length {
            return Err(invalid());
        }
        // Sharing immutable backing copies the semantic value. An indexed
        // destination mutation detaches it before changing any byte.
        self.structural_byte_sequence_fields
            .insert(destination_field, bytes.clone());
        Ok(())
    }

    pub(crate) fn resolve_structural_byte_sequence_field(
        &self,
        root: PlaceId,
        path: &[StructuralPathSegment],
        field: StructuralFieldId,
        requires_mutation: bool,
    ) -> Result<(StructuralByteSequenceRuntimeField, u64), TerminalInterpretError> {
        let (cell, carrier) =
            self.resolve_structural_byte_field_carrier(root, path, field, requires_mutation)?;
        let ByteSequenceCarrier::BoundedOwned { capacity } = carrier else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        Ok((cell, capacity))
    }

    /// Dispatch charges fuel first. The verifier admitted only a literal
    /// source, whose immutable bytes the field now shares: the descriptor is
    /// replaced whole and the displaced view's backing is left untouched.
    pub(crate) fn execute_structural_byte_view_field_store(
        &mut self,
        operation: &Operation,
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let OperationKind::StructuralByteViewFieldStore {
            destination,
            path,
            field,
            source,
            ..
        } = &operation.kind
        else {
            return Err(invalid());
        };
        if operation.result != OperationResult::Unit || destination == source {
            return Err(invalid());
        }
        let (destination_field, carrier) =
            self.resolve_structural_byte_field_carrier(*destination, path, *field, true)?;
        if carrier
            != (ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            })
        {
            return Err(invalid());
        }
        let source_value = self.structural_values.get(source).ok_or_else(invalid)?;
        if !source_value.path.is_empty()
            || !matches!(self.program.structural_types.get(&source_value.structural_type),
                Some(declaration) if declaration.shape.is_borrowed_byte_view())
        {
            return Err(invalid());
        }
        let view = self
            .byte_sequence_values
            .get(source)
            .ok_or_else(invalid)?
            .immutable()?
            .clone();
        self.structural_byte_sequence_fields
            .insert(destination_field, view);
        Ok(())
    }

    /// Resolve one byte field's runtime cell and its declared carrier below
    /// the current home of `root`. Callers decide which carriers they admit.
    pub(crate) fn resolve_structural_byte_field_carrier(
        &self,
        root: PlaceId,
        path: &[StructuralPathSegment],
        field: StructuralFieldId,
        requires_mutation: bool,
    ) -> Result<(StructuralByteSequenceRuntimeField, ByteSequenceCarrier), TerminalInterpretError>
    {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let machine = self
            .program
            .machines
            .get(&self.current_machine)
            .ok_or_else(invalid)?;
        if self
            .live_claims
            .values()
            .any(|claim| claim.place == Some(root))
        {
            return Err(invalid());
        }
        let (access, parameter_root) = if let Some(parameter) = machine
            .structural_parameters
            .iter()
            .chain(
                machine
                    .blocks
                    .values()
                    .flat_map(|block| &block.structural_parameters),
            )
            .find(|parameter| parameter.place == root)
        {
            if !matches!(
                parameter.multiplicity,
                StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
            ) || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                return Err(invalid());
            }
            (parameter.access, true)
        } else {
            // Admission already verified the exact producer and its declaration
            // custody. Execution must find that home's current instance; it
            // cannot recover a value from a future or retired producer.
            let declaration = machine
                .structural_places
                .iter()
                .find(|place| place.id == root)
                .ok_or_else(invalid)?;
            let semantic_vocabulary::StructuralPlaceKind::OperationResult {
                structural_type, ..
            } = declaration.kind
            else {
                return Err(invalid());
            };
            if !self
                .structural_values
                .get(&root)
                .is_some_and(|value| value.structural_type == structural_type)
            {
                return Err(invalid());
            }
            (StructuralAccess::Owned, false)
        };
        let parent = if let Some(view) = self.element_view_values.get(&root) {
            if parameter_root && requires_mutation && access != StructuralAccess::MutableBorrow {
                return Err(invalid());
            }
            let descriptor_type = self.live_structural_type(root)?;
            let StructuralTypeShape::ElementView { element } = self
                .program
                .structural_types
                .get(&descriptor_type)
                .ok_or_else(invalid)?
                .shape
            else {
                return Err(invalid());
            };
            let (collection, selected_path) =
                view.structural_projection(path, requires_mutation)?;
            let StructuralTypeShape::FixedArray {
                element: backing_element,
                ..
            } = self
                .program
                .structural_types
                .get(&collection.structural_type)
                .ok_or_else(invalid)?
                .shape
            else {
                return Err(invalid());
            };
            if element != backing_element {
                return Err(invalid());
            }
            project_structural_value(&self.program.structural_types, &collection, &selected_path)?
        } else {
            // Authority belongs to the current binding, including an arrival
            // parameter. The static projection resolver checks every selected hop;
            // the operation-wide selector pass already checked dynamic bounds.
            if requires_mutation
                && !matches!(
                    access,
                    StructuralAccess::Owned
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                )
            {
                return Err(invalid());
            }
            resolve_structural_arguments(
                &self.program.structural_types,
                &self.structural_values,
                &[StructuralArgument {
                    place: root,
                    path: path.to_vec(),
                    access,
                }],
            )?
            .pop()
            .ok_or_else(invalid)?
        };
        let declaration = self
            .program
            .structural_types
            .get(&parent.structural_type)
            .ok_or_else(invalid)?;
        let StructuralTypeShape::Record { fields } = &declaration.shape else {
            return Err(invalid());
        };
        let field_declaration = fields
            .iter()
            .find(|candidate| candidate.id == field && !candidate.relevance.is_erased())
            .ok_or_else(invalid)?;
        let carrier = field_declaration
            .field_type
            .byte_sequence_carrier()
            .ok_or_else(invalid)?;
        Ok((
            StructuralByteSequenceRuntimeField {
                parent: StructuralRuntimePlace::from(&parent),
                field,
            },
            carrier,
        ))
    }
}
