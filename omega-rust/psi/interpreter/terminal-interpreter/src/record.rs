//! Atomic record establishment over the interpreter's existing structural storage.
use crate::custody::remove_affine_root;
use crate::errors::TerminalInterpretError;
use crate::execution::TerminalExecution;
use crate::scalar_operations::terminal_scalar_belongs_to_type;
use crate::values::StructuralRuntimePlace;
use crate::values::StructuralScalarRuntimeField;
use crate::values::{TerminalScalarValue, TerminalStructuralValue};
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use terminal_psi::OperationKind;
use terminal_psi::StructuralAccess;
use terminal_psi::StructuralAffineDiscard;
use terminal_psi::StructuralFieldType;
use terminal_psi::StructuralMultiplicity;
use terminal_psi::StructuralParameterDeclaration;
use terminal_psi::StructuralPathSegment;
use terminal_psi::StructuralTypeShape;

#[cfg(test)]
mod tests;

impl TerminalExecution {
    pub(crate) fn establish_record(
        &mut self,
        result: &terminal_psi::StructuralOperationResult,
        fields: &[terminal_psi::RecordFieldInitializer],
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        if !matches!(
            result.multiplicity,
            StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
        ) || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
        {
            return Err(invalid());
        }
        let declaration = self
            .program
            .structural_types
            .get(&result.structural_type)
            .ok_or_else(invalid)?;
        let StructuralTypeShape::Record {
            fields: declarations,
        } = &declaration.shape
        else {
            return Err(invalid());
        };
        if fields.len() != declarations.len() {
            return Err(invalid());
        }
        let value = TerminalStructuralValue {
            opaque_identity: self.local_structural_identities.allocate()?,
            structural_type: result.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        };
        let mut staged = BTreeMap::new();
        let mut staged_referents = BTreeMap::new();
        let mut staged_bytes = BTreeMap::new();
        let mut staged_views = BTreeMap::new();
        let mut consumed = BTreeSet::new();
        for (binding, declaration) in fields.iter().zip(declarations) {
            if binding.field != declaration.id || declaration.relevance.is_erased() {
                return Err(invalid());
            }
            match &binding.value {
                terminal_psi::RecordFieldValue::Scalar { value: operand, .. } => {
                    let scalar = self.values.get(operand).copied().ok_or_else(invalid)?;
                    if declaration.field_type.scalar_type() != Some(scalar.scalar_type())
                        || !terminal_scalar_belongs_to_type(scalar)
                    {
                        return Err(invalid());
                    }
                    staged.insert(
                        StructuralScalarRuntimeField::member(
                            StructuralRuntimePlace::from(&value),
                            binding.field,
                        ),
                        scalar,
                    );
                }
                terminal_psi::RecordFieldValue::Structural(argument) => {
                    let child = self
                        .structural_values
                        .get(&argument.place)
                        .ok_or_else(invalid)?;
                    if matches!(
                        declaration.field_type.byte_sequence_carrier(),
                        Some(terminal_psi::ByteSequenceCarrier::BorrowedView {
                            access: None | Some(StructuralAccess::SharedBorrow),
                        })
                    ) {
                        let bytes = self
                            .byte_sequence_values
                            .get(&argument.place)
                            .ok_or_else(invalid)?
                            .immutable()?;
                        if argument.access != StructuralAccess::Owned
                            || !argument.path.is_empty()
                            || !self
                                .program
                                .structural_types
                                .get(&child.structural_type)
                                .is_some_and(|row| {
                                    matches!(
                                        row.shape,
                                        StructuralTypeShape::ByteSequence(
                                            terminal_psi::ByteSequenceCarrier::BorrowedView {
                                                access: None | Some(StructuralAccess::SharedBorrow),
                                            }
                                        )
                                    )
                                })
                            || !terminal_semantics::byte_literal_qualifications_hold_in(
                                self.program.catalogs.structural_domains.iter(),
                                child.structural_type,
                                &child.qualifications,
                                bytes.bytes(),
                            )
                            || !declaration
                                .field_type
                                .byte_predicates()
                                .holds_for(bytes.bytes())
                        {
                            return Err(invalid());
                        }
                        let affine = self
                            .live_affine_frontier
                            .iter()
                            .any(|entry| entry.place == argument.place && entry.path.is_empty());
                        if affine
                            && (!consumed.insert(argument.place)
                                || result.multiplicity == StructuralMultiplicity::Unrestricted)
                        {
                            return Err(invalid());
                        }
                        staged_bytes.insert(
                            crate::values::StructuralByteSequenceRuntimeField {
                                parent: StructuralRuntimePlace::from(&value),
                                field: binding.field,
                            },
                            bytes.clone(),
                        );
                        continue;
                    }
                    if let Some(
                        carrier @ terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                    ) = declaration.field_type.byte_sequence_carrier()
                    {
                        let predicates = terminal_semantics::byte_qualification_predicates_in(
                            self.program.catalogs.structural_domains.iter(),
                            child.structural_type,
                            &child.qualifications,
                        )
                        .ok_or_else(invalid)?;
                        let bytes = self
                            .byte_sequence_values
                            .get(&argument.place)
                            .ok_or_else(invalid)?
                            .immutable()?;
                        if argument.access != StructuralAccess::Owned
                            || !argument.path.is_empty()
                            || !child.path.is_empty()
                            || !self
                                .program
                                .structural_types
                                .get(&child.structural_type)
                                .is_some_and(|declaration| {
                                    declaration.shape == StructuralTypeShape::ByteSequence(carrier)
                                })
                            || !predicates.implies(declaration.field_type.byte_predicates())
                            || u64::try_from(bytes.len()).map_or(true, |length| length > capacity)
                            || self
                                .live_affine_frontier
                                .iter()
                                .any(|entry| entry.place == argument.place)
                        {
                            return Err(invalid());
                        }
                        staged_bytes.insert(
                            crate::values::StructuralByteSequenceRuntimeField {
                                parent: StructuralRuntimePlace::from(&value),
                                field: binding.field,
                            },
                            bytes.clone(),
                        );
                        continue;
                    }
                    if argument.access != StructuralAccess::Owned
                        || !argument.path.is_empty()
                        || !child.qualifications.is_empty()
                        || declaration.field_type
                            != StructuralFieldType::Structural(child.structural_type)
                    {
                        return Err(invalid());
                    }
                    let affine = self
                        .live_affine_frontier
                        .iter()
                        .any(|entry| entry.place == argument.place && entry.path.is_empty());
                    if affine
                        && (!consumed.insert(argument.place)
                            || result.multiplicity == StructuralMultiplicity::Unrestricted)
                    {
                        return Err(invalid());
                    }
                    let mut member_path = value.path.clone();
                    member_path.push(StructuralPathSegment::Field(declaration.identity.clone()));
                    if self
                        .program
                        .structural_types
                        .get(&child.structural_type)
                        .is_some_and(|row| {
                            matches!(row.shape, StructuralTypeShape::ElementView { .. })
                        })
                    {
                        let descriptor = self.element_view_argument(argument, child)?;
                        let descriptor = descriptor.clone();
                        staged_views.insert(
                            StructuralRuntimePlace {
                                opaque_identity: value.opaque_identity,
                                path: member_path.clone(),
                            },
                            descriptor,
                        );
                    }
                    for (home, descriptor) in &self.structural_element_view_fields {
                        if home.opaque_identity == child.opaque_identity
                            && home.path.starts_with(&child.path)
                        {
                            let mut path = member_path.clone();
                            path.extend_from_slice(&home.path[child.path.len()..]);
                            staged_views.insert(
                                StructuralRuntimePlace {
                                    opaque_identity: value.opaque_identity,
                                    path,
                                },
                                descriptor.clone(),
                            );
                        }
                    }
                    // Move permission descriptors into the new owner's field
                    // coordinates, retaining their original backing unchanged.
                    // Stage all fields before consuming any child so a bad
                    // later operand cannot partially move a loan.
                    for (carrier, referent) in &self.reference_referents {
                        if carrier.opaque_identity == child.opaque_identity
                            && carrier.path.starts_with(&child.path)
                        {
                            if !affine {
                                return Err(invalid());
                            }
                            let mut path =
                                vec![StructuralPathSegment::Field(declaration.identity.clone())];
                            path.extend_from_slice(&carrier.path[child.path.len()..]);
                            staged_referents.insert(
                                StructuralRuntimePlace {
                                    opaque_identity: value.opaque_identity,
                                    path,
                                },
                                referent.clone(),
                            );
                        }
                    }
                    // Copy exact completed leaf storage into the parent's nested home. The
                    // child retains its independent identity until the entire roster passes.
                    for (field, scalar) in &self.structural_scalar_fields {
                        if field.parent.opaque_identity == child.opaque_identity
                            && field.parent.path.starts_with(&child.path)
                        {
                            let mut path =
                                vec![StructuralPathSegment::Field(declaration.identity.clone())];
                            path.extend_from_slice(&field.parent.path[child.path.len()..]);
                            staged.insert(
                                field.relocated(StructuralRuntimePlace {
                                    opaque_identity: value.opaque_identity,
                                    path,
                                }),
                                *scalar,
                            );
                        }
                    }
                    for (field, bytes) in &self.structural_byte_sequence_fields {
                        if field.parent.opaque_identity == child.opaque_identity
                            && field.parent.path.starts_with(&child.path)
                        {
                            let mut path =
                                vec![StructuralPathSegment::Field(declaration.identity.clone())];
                            path.extend_from_slice(&field.parent.path[child.path.len()..]);
                            staged_bytes.insert(
                                crate::values::StructuralByteSequenceRuntimeField {
                                    parent: StructuralRuntimePlace {
                                        opaque_identity: value.opaque_identity,
                                        path,
                                    },
                                    field: field.field,
                                },
                                bytes.clone(),
                            );
                        }
                    }
                }
            }
        }
        for place in consumed {
            if let Some(child) = self.structural_values.remove(&place) {
                self.structural_element_view_fields.retain(|home, _| {
                    home.opaque_identity != child.opaque_identity
                        || !home.path.starts_with(&child.path)
                });
                self.reference_referents.retain(|carrier, _| {
                    carrier.opaque_identity != child.opaque_identity
                        || !carrier.path.starts_with(&child.path)
                });
                self.structural_scalar_fields.retain(|field, _| {
                    field.parent.opaque_identity != child.opaque_identity
                        || !field.parent.path.starts_with(&child.path)
                });
                self.structural_byte_sequence_fields.retain(|field, _| {
                    field.parent.opaque_identity != child.opaque_identity
                        || !field.parent.path.starts_with(&child.path)
                });
            }
            remove_affine_root(&mut self.live_affine_frontier, place);
        }
        self.structural_scalar_fields.extend(staged);
        self.structural_byte_sequence_fields.extend(staged_bytes);
        self.structural_element_view_fields.extend(staged_views);
        self.reference_referents.extend(staged_referents);
        self.structural_values.insert(result.place, value);
        if result.multiplicity == StructuralMultiplicity::Affine {
            self.live_affine_frontier.insert(StructuralAffineDiscard {
                place: result.place,
                path: Vec::new(),
                structural_type: result.structural_type,
            });
        }
        Ok(())
    }
    /// Stage independent payloads for each owned unrestricted occurrence. The
    /// supplied descriptors and allocation cursor belong to the caller's
    /// uncommitted transaction; source fields remain untouched even in swaps.
    pub(crate) fn stage_owned_record_arguments(
        &self,
        parameters: &[StructuralParameterDeclaration],
        values: &mut BTreeMap<semantic_vocabulary::PlaceId, TerminalStructuralValue>,
        cursor: &mut Option<u64>,
    ) -> Result<Vec<(StructuralScalarRuntimeField, TerminalScalarValue)>, TerminalInterpretError>
    {
        let mut payload = Vec::new();
        for parameter in parameters {
            if parameter.access != StructuralAccess::Owned
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
                || !self.plain_record_type(parameter.structural_type)
            {
                continue;
            }
            let value = values.get_mut(&parameter.place).ok_or(
                TerminalInterpretError::VerifiedStructuralPlaceMissing(parameter.place),
            )?;
            let source = StructuralRuntimePlace::from(&*value);
            let identity = self.local_structural_identities.allocate_staged(cursor)?;
            payload.extend(
                self.structural_scalar_fields
                    .iter()
                    .filter(|(field, _)| {
                        field.parent.opaque_identity == source.opaque_identity
                            && field.parent.path.starts_with(&source.path)
                    })
                    .map(|(field, scalar)| {
                        (
                            field.relocated(StructuralRuntimePlace {
                                opaque_identity: identity,
                                path: field.parent.path[source.path.len()..].to_vec(),
                            }),
                            *scalar,
                        )
                    }),
            );
            value.opaque_identity = identity;
            value.path.clear();
        }
        Ok(payload)
    }

    pub(crate) fn retire_unrestricted_aggregates(&mut self) {
        let Some(machine) = self.program.machines.get(&self.current_machine) else {
            return;
        };
        let places = machine
            .blocks
            .values()
            .flat_map(|block| &block.operations)
            .filter(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::EstablishRecord { .. }
                        | OperationKind::EstablishStructuralCase { .. }
                        | OperationKind::EstablishZeroedStructural { .. }
                        | OperationKind::EstablishStructuralLocal { .. }
                        | OperationKind::MoveStructuralField { .. }
                        | OperationKind::CallStructural { .. }
                        | OperationKind::CallStructuralWithScalarArguments { .. }
                        | OperationKind::StructuralLeafCopy { .. }
                        | OperationKind::StructuralCaseLeafCopy { .. }
                )
            })
            .filter_map(|operation| {
                let result = operation.result.structural()?;
                // A zeroed structural covers record members that are
                // themselves fixed arrays: its own leaf predicate is the
                // plain-type test.
                let plain = match operation.kind {
                    OperationKind::EstablishZeroedStructural { .. } => {
                        terminal_semantics::zeroed_structural_shape(
                            self.program.structural_types.values(),
                            result.structural_type,
                        )
                    }
                    _ => {
                        self.plain_record_type(result.structural_type)
                            || self.plain_case_type(result.structural_type)
                    }
                };
                (result.multiplicity == StructuralMultiplicity::Unrestricted
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
                    && plain)
                    .then_some(result.place)
            })
            .collect::<Vec<_>>();
        for place in places {
            self.structural_values.remove(&place);
        }
    }

    pub(crate) fn plain_record_type(&self, root: StructuralTypeId) -> bool {
        let mut pending = vec![root];
        let mut visited = BTreeSet::new();
        while let Some(identity) = pending.pop() {
            if !visited.insert(identity) {
                continue;
            }
            let Some(StructuralTypeShape::Record { fields }) = self
                .program
                .structural_types
                .get(&identity)
                .map(|declaration| &declaration.shape)
            else {
                return false;
            };
            for field in fields {
                if field.relevance.is_erased() {
                    return false;
                }
                match field.field_type {
                    StructuralFieldType::Structural(child) => pending.push(child),
                    _ if field.field_type.scalar_type().is_some() => {}
                    _ => return false,
                }
            }
        }
        true
    }
}
