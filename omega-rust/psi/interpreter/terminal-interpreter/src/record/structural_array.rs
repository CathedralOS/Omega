//! Completed array operands relocate through the same staged subtree transaction
//! as structural record fields. The destination is published only after every
//! element and custody transfer has been checked.
use super::*;
use crate::byte_sequences::ByteSequenceView;
use crate::errors::malformed;
use crate::values::StructuralByteSequenceRuntimeField;

#[derive(Default)]
pub(super) struct ConstructorStorage {
    scalars: BTreeMap<StructuralScalarRuntimeField, TerminalScalarValue>,
    bytes: BTreeMap<StructuralByteSequenceRuntimeField, ByteSequenceView>,
    views: BTreeMap<StructuralRuntimePlace, crate::element_views::ElementView>,
    references: BTreeMap<StructuralRuntimePlace, TerminalStructuralValue>,
    primitives: BTreeMap<StructuralRuntimePlace, TerminalScalarValue>,
    byte_arrays: BTreeMap<StructuralRuntimePlace, ByteSequenceView>,
    cases: BTreeMap<
        StructuralRuntimePlace,
        crate::structural_inputs::case_membership::StructuralCaseContents,
    >,
}

impl ConstructorStorage {
    pub(super) fn stage(
        &mut self,
        execution: &TerminalExecution,
        argument: &terminal_psi::StructuralArgument,
        child: &TerminalStructuralValue,
        destination: &StructuralRuntimePlace,
        transfers_custody: bool,
    ) -> Result<(), TerminalInterpretError> {
        let relocate = |home: &StructuralRuntimePlace| {
            if home.opaque_identity != child.opaque_identity || !home.path.starts_with(&child.path)
            {
                return None;
            }
            let mut path = destination.path.clone();
            path.extend_from_slice(&home.path[child.path.len()..]);
            Some(StructuralRuntimePlace {
                opaque_identity: destination.opaque_identity,
                path,
            })
        };
        if execution
            .program
            .structural_types
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
            .structural_types
            .get(&child.structural_type)
            .is_some_and(|row| {
                matches!(
                    row.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView {
                            access: None | Some(StructuralAccess::SharedBorrow)
                        }
                    )
                )
            })
        {
            let bytes = execution.shared_byte_descriptor(argument, child)?;
            self.byte_arrays
                .insert(destination.clone(), bytes.detached_descriptor());
        }
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

    pub(super) fn publish(self, execution: &mut TerminalExecution) {
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

impl TerminalExecution {
    pub(crate) fn execute_establish_structural_array(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<crate::execution::OperationFlow, TerminalInterpretError> {
        let OperationKind::EstablishStructuralArray { elements } = &operation.kind else {
            return Err(malformed());
        };
        let result = operation.result.structural().ok_or_else(malformed)?;
        let Some(StructuralTypeShape::FixedArray { element, length }) = self
            .program
            .structural_types
            .get(&result.structural_type)
            .map(|row| &row.shape)
        else {
            return Err(malformed());
        };
        let element = *element;
        if u64::try_from(elements.len()).ok() != Some(*length)
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || self
                .live_affine_frontier
                .iter()
                .any(|entry| entry.place == result.place)
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(result.place))
        {
            return Err(malformed());
        }
        let mut cursor = self.local_structural_identities.cursor();
        let value = TerminalStructuralValue {
            opaque_identity: self
                .local_structural_identities
                .allocate_staged(&mut cursor)?,
            structural_type: result.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        };
        let mut staged = ConstructorStorage::default();
        let mut frontier = self.live_affine_frontier.clone();
        let mut consumed = Vec::new();
        let mut claim_operands = Vec::new();
        for (index, argument) in elements.iter().enumerate() {
            let child = resolve_structural_arguments(
                &self.program.structural_types,
                &self.structural_values,
                std::slice::from_ref(argument),
            )?
            .pop()
            .ok_or_else(malformed)?;
            if argument.access != StructuralAccess::Owned
                || argument.place == result.place
                || child.structural_type != element
                || !child.qualifications.is_empty()
            {
                return Err(malformed());
            }
            let transfers_custody =
                self.stage_constructor_operand(argument, &mut frontier, &mut consumed)?;
            if transfers_custody && result.multiplicity == StructuralMultiplicity::Unrestricted {
                return Err(malformed());
            }
            let destination = StructuralRuntimePlace {
                opaque_identity: value.opaque_identity,
                path: vec![StructuralPathSegment::FixedIndex(
                    u64::try_from(index).map_err(|_| malformed())?,
                )],
            };
            claim_operands.push((argument, destination.path.clone()));
            staged.stage(self, argument, &child, &destination, transfers_custody)?;
        }
        let claims = self.stage_constructor_claims(result, &claim_operands, &mut frontier)?;
        self.commit_constructor_operands(frontier, &consumed, claims)?;
        self.local_structural_identities.commit_cursor(cursor);
        staged.publish(self);
        self.structural_values.insert(result.place, value);
        Ok(crate::execution::OperationFlow::Advance)
    }
}
