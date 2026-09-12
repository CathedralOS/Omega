//! Atomic record establishment over the interpreter's existing structural storage.
use super::*;
use terminal_psi::StructuralFieldType;

#[cfg(test)]
mod tests;

impl TerminalExecution {
    pub(super) fn establish_record(
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
                        StructuralScalarRuntimeField {
                            parent: StructuralRuntimePlace::from(&value),
                            field: binding.field,
                        },
                        scalar,
                    );
                }
                terminal_psi::RecordFieldValue::Structural(argument) => {
                    let child = self
                        .structural_values
                        .get(&argument.place)
                        .ok_or_else(invalid)?;
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
                                StructuralScalarRuntimeField {
                                    parent: StructuralRuntimePlace {
                                        opaque_identity: value.opaque_identity,
                                        path,
                                    },
                                    field: field.field,
                                },
                                *scalar,
                            );
                        }
                    }
                }
            }
        }
        for place in consumed {
            if let Some(child) = self.structural_values.remove(&place) {
                self.structural_scalar_fields.retain(|field, _| {
                    field.parent.opaque_identity != child.opaque_identity
                        || !field.parent.path.starts_with(&child.path)
                });
            }
            remove_affine_root(&mut self.live_affine_frontier, place);
        }
        self.structural_scalar_fields.extend(staged);
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
    /// Copy by-value records into independent activation storage. Borrowed inputs
    /// preserve their referents; absent host payload remains absent rather than
    /// manufacturing initialized zero fields.
    pub(super) fn copy_owned_record_arguments(
        &mut self,
        parameters: &[StructuralParameterDeclaration],
        values: &mut BTreeMap<semantic_vocabulary::PlaceId, TerminalStructuralValue>,
    ) -> Result<(), TerminalInterpretError> {
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
            let identity = self.local_structural_identities.allocate()?;
            let payload = self
                .structural_scalar_fields
                .iter()
                .filter(|(field, _)| {
                    field.parent.opaque_identity == source.opaque_identity
                        && field.parent.path.starts_with(&source.path)
                })
                .map(|(field, scalar)| {
                    (
                        StructuralScalarRuntimeField {
                            parent: StructuralRuntimePlace {
                                opaque_identity: identity,
                                path: field.parent.path[source.path.len()..].to_vec(),
                            },
                            field: field.field,
                        },
                        *scalar,
                    )
                })
                .collect::<Vec<_>>();
            self.structural_scalar_fields.extend(payload);
            value.opaque_identity = identity;
            value.path.clear();
        }
        Ok(())
    }

    pub(super) fn retire_unrestricted_records(&mut self) {
        let Some(machine) = self.machines.get(&self.current_machine) else {
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
                        | OperationKind::CallStructural { .. }
                        | OperationKind::CallStructuralWithScalarArguments { .. }
                )
            })
            .filter_map(|operation| operation.result.structural())
            .filter(|result| {
                result.multiplicity == StructuralMultiplicity::Unrestricted
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
                    && self.plain_record_type(result.structural_type)
            })
            .map(|result| result.place)
            .collect::<Vec<_>>();
        for place in places {
            self.structural_values.remove(&place);
        }
    }

    pub(super) fn plain_record_type(&self, root: StructuralTypeId) -> bool {
        let mut pending = vec![root];
        let mut visited = BTreeSet::new();
        while let Some(identity) = pending.pop() {
            if !visited.insert(identity) {
                continue;
            }
            let Some(StructuralTypeShape::Record { fields }) = self
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
