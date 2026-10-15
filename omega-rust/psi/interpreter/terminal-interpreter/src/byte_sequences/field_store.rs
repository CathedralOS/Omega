//! Non-observing bounded byte replacement with invocation-independent backing.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, PlaceId, StructuralFieldId};
use terminal_psi::{
    ByteSequenceCarrier, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralPathSegment, StructuralTypeShape,
};

use crate::byte_sequences::binding::ByteSequenceBinding;
use crate::custody::project_structural_value;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::StructuralByteSequenceRuntimeField;
use crate::values::StructuralRuntimePlace;
use crate::values::TerminalScalarValue;

impl TerminalExecution<'_> {
    /// Dispatch charges fuel first. Validate and snapshot the complete source
    /// before replacing either live bytes or length; no failure partially writes.
    pub(crate) fn execute_structural_byte_sequence_field_store(
        &mut self,
        operation: &Operation,
    ) -> Result<(), TerminalInterpretError> {
        let OperationKind::StructuralByteSequenceFieldStore {
            destination,
            path,
            field,
            source,
            length,
            ..
        } = &operation.kind
        else {
            return Err(malformed());
        };
        if operation.result != OperationResult::Unit || destination == source {
            return Err(malformed());
        }
        let (destination_field, capacity) =
            self.resolve_structural_byte_sequence_field(*destination, path, *field, true)?;
        if self
            .live_claims
            .values()
            .any(|claim| claim.place == Some(*source))
        {
            return Err(malformed());
        }
        let TerminalScalarValue::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(length),
        } = self.values.get(length).ok_or_else(malformed)?
        else {
            return Err(malformed());
        };
        if *scalar_type != IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| malformed())?
            || *length > u128::from(capacity)
        {
            return Err(malformed());
        }
        let source_value = self.structural_values.get(source).ok_or_else(malformed)?;
        if !source_value.projected_qualifications.is_empty()
            || !matches!(self.program.structural_types().get(&source_value.structural_type), Some(declaration)
                            if declaration.shape.is_borrowed_byte_view()
                                || declaration.shape.is_bounded_owned_byte_sequence())
        {
            return Err(malformed());
        }
        let bytes = self
            .byte_sequence_values
            .get(source)
            .ok_or_else(malformed)?
            .immutable()?;
        // A qualified value is still a readable byte carrier. Its immutable
        // binding retains the captured contents and exact descriptor home;
        // qualifiers neither change those bytes nor require an empty backing
        // path for a descriptor borrowed from a field.
        if !bytes.matches_descriptor_home(source_value) || bytes.len() as u128 != *length {
            return Err(malformed());
        }
        // Sharing immutable backing copies the semantic value. An indexed
        // destination mutation detaches it before changing any byte.
        self.structural_byte_sequence_fields.insert(
            destination_field,
            ByteSequenceBinding::Immutable(bytes.clone()),
        );
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
            return Err(malformed());
        };
        Ok((cell, capacity))
    }

    /// A readable byte field: a bounded owner with its capacity, or a shared
    /// view field whose descriptor bounds nothing beyond its own extent.
    pub(crate) fn readable_byte_field(
        &self,
        root: semantic_vocabulary::PlaceId,
        path: &[terminal_psi::StructuralPathSegment],
        field: semantic_vocabulary::StructuralFieldId,
    ) -> Result<
        (
            crate::values::StructuralByteSequenceRuntimeField,
            Option<u64>,
        ),
        TerminalInterpretError,
    > {
        let (cell, carrier) =
            self.resolve_structural_byte_field_carrier(root, path, field, false)?;
        match carrier {
            terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity } => {
                Ok((cell, Some(capacity)))
            }
            terminal_psi::ByteSequenceCarrier::BorrowedView {
                access:
                    None
                    | Some(
                        terminal_psi::StructuralAccess::SharedBorrow
                        | terminal_psi::StructuralAccess::MutableBorrow,
                    ),
            } => Ok((cell, None)),
            terminal_psi::ByteSequenceCarrier::BoundedView { access, capacity }
                if access != Some(terminal_psi::StructuralAccess::WriteOnlyBorrow) =>
            {
                Ok((cell, Some(capacity)))
            }
            _ => Err(malformed()),
        }
    }

    /// Dispatch charges fuel first. The verifier admitted a readable shared
    /// descriptor source, whose immutable bytes the slot now shares: the
    /// descriptor is replaced whole and the displaced view's backing is left
    /// untouched. A field slot lives below its record's current home; the
    /// whole slot of a declared descriptor local is that home's own binding.
    pub(crate) fn execute_structural_byte_view_field_store(
        &mut self,
        operation: &Operation,
    ) -> Result<(), TerminalInterpretError> {
        let OperationKind::StructuralByteViewFieldStore {
            destination,
            path,
            field,
            source,
            ..
        } = &operation.kind
        else {
            return Err(malformed());
        };
        if operation.result != OperationResult::Unit || destination == source {
            return Err(malformed());
        }
        let shared_view = ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        };
        let destination_field = match field {
            Some(field) => {
                let (destination_field, carrier) =
                    self.resolve_structural_byte_field_carrier(*destination, path, *field, true)?;
                if carrier != shared_view {
                    return Err(malformed());
                }
                Some(destination_field)
            }
            None => {
                let home = self
                    .structural_values
                    .get(destination)
                    .ok_or_else(malformed)?;
                if !path.is_empty()
                    || !home.path.is_empty()
                    || !matches!(self.program.structural_types().get(&home.structural_type),
                    Some(declaration) if declaration.shape.is_shared_readable_byte_view())
                {
                    return Err(malformed());
                }
                None
            }
        };
        let source_value = self.structural_values.get(source).ok_or_else(malformed)?;
        if !source_value.path.is_empty()
            || !matches!(self.program.structural_types().get(&source_value.structural_type),
                Some(declaration) if declaration.shape.is_borrowed_byte_view())
        {
            return Err(malformed());
        }
        let view = self
            .byte_sequence_values
            .get(source)
            .ok_or_else(malformed)?
            .immutable()?
            .clone();
        if !view.matches_descriptor_home(source_value) {
            return Err(malformed());
        }
        match destination_field {
            Some(destination_field) => {
                self.structural_byte_sequence_fields
                    .insert(destination_field, ByteSequenceBinding::Immutable(view));
            }
            None => {
                // Replacing a descriptor transfers its observed window, not
                // its old storage identity. Rehome only the descriptor; its
                // backing and captured extent remain those of the source.
                let home = self
                    .structural_values
                    .get(destination)
                    .ok_or_else(malformed)?;
                self.byte_sequence_values.insert(
                    *destination,
                    ByteSequenceBinding::Immutable(view.with_descriptor_home(home.clone())),
                );
            }
        }
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
        let machine = self
            .program
            .machines()
            .get(&self.current_machine)
            .ok_or_else(malformed)?;
        if self
            .live_claims
            .values()
            .any(|claim| claim.place == Some(root))
        {
            return Err(malformed());
        }
        let (access, parameter_root) = if let Some(parameter) = machine.structural_parameter(root) {
            // The verified parameter retains its predicate roster. Selecting
            // its current field changes neither custody nor qualification.
            (parameter.access, true)
        } else {
            // Admission already verified the exact producer and its declaration
            // custody. Execution must find that home's current instance; it
            // cannot recover a value from a future or retired producer.
            let declaration = machine.structural_place(root).ok_or_else(malformed)?;
            let semantic_vocabulary::StructuralPlaceKind::OperationResult {
                structural_type, ..
            } = declaration.kind
            else {
                return Err(malformed());
            };
            if !self
                .structural_values
                .get(&root)
                .is_some_and(|value| value.structural_type == structural_type)
            {
                return Err(malformed());
            }
            (StructuralAccess::Owned, false)
        };
        // A final Case selects a payload namespace, not a child allocation.
        // Resolve its sum first, then authenticate the stored tag at that exact
        // backing before looking up its declaration-local field.
        let (carrier_path, selected_case) = match path.split_last() {
            Some((StructuralPathSegment::Case(case), prefix)) => (prefix, Some(*case)),
            _ => (path, None),
        };
        let parent = if let Some(view) = self.element_view_values.get(&root) {
            if parameter_root && requires_mutation && access != StructuralAccess::MutableBorrow {
                return Err(malformed());
            }
            let descriptor_type = self.live_structural_type(root)?;
            let StructuralTypeShape::ElementView { element, .. } = self
                .program
                .structural_types()
                .get(&descriptor_type)
                .ok_or_else(malformed)?
                .shape
            else {
                return Err(malformed());
            };
            let (collection, selected_path) = view.structural_projection(
                &self.program.structural_types(),
                carrier_path,
                requires_mutation,
            )?;
            let StructuralTypeShape::FixedArray {
                element: backing_element,
                ..
            } = self
                .program
                .structural_types()
                .get(&collection.structural_type)
                .ok_or_else(malformed)?
                .shape
            else {
                return Err(malformed());
            };
            if element != backing_element {
                return Err(malformed());
            }
            project_structural_value(
                &self.program.structural_types(),
                &collection,
                &selected_path,
            )?
        } else {
            // Authority belongs to the current binding, including an arrival
            // parameter. The static projection resolver checks every selected hop;
            // the operation-wide selector pass already checked dynamic bounds.
            let requested = if requires_mutation {
                StructuralAccess::WriteOnlyBorrow
            } else {
                StructuralAccess::SharedBorrow
            };
            if !crate::reference::access_covers(access, requested) {
                return Err(malformed());
            }
            // A reference hop selects captured referent storage. Appending
            // Referent as a physical field path would instead write beneath
            // the descriptor and leave the caller's bytes unchanged.
            self.resolve_live_projection(root, carrier_path, requested)?
        };
        let (fields, namespace) = self.active_field_declarations(
            parent.structural_type,
            &StructuralRuntimePlace::from(&parent),
            selected_case,
        )?;
        let field_declaration =
            terminal_psi::live_field_by_id(fields, field).ok_or_else(malformed)?;
        let carrier = field_declaration
            .field_type
            .byte_sequence_carrier()
            .ok_or_else(malformed)?;
        Ok((
            StructuralByteSequenceRuntimeField {
                parent: StructuralRuntimePlace::from(&parent),
                namespace,
                field,
            },
            carrier,
        ))
    }
}
