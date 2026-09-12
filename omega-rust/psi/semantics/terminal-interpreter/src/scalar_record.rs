//! Record constructors publish complete payloads after operand validation.
//! Runtime identity belongs to an activation, not a reusable Terminal PlaceId;
//! nested calls and repeated constructions must never alias earlier records.

use terminal_psi::{
    Operation, ScalarRecordFieldValue, StructuralAccess, StructuralAffineDiscard,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeShape,
};

use super::{
    StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalExecution,
    TerminalInterpretError, TerminalStructuralValue, terminal_scalar_belongs_to_type,
};

#[cfg(test)]
mod tests;

impl TerminalExecution {
    /// Plain copied payloads have no affine disposal obligation. Retire their
    /// activation bindings before checking the exact remaining cleanup frontier.
    pub(super) fn retire_unrestricted_scalar_records(&mut self) {
        let Some(machine) = self.machines.get(&self.current_machine) else {
            return;
        };
        for result in machine.blocks.values().flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind,
            terminal_psi::OperationKind::EstablishScalarRecord { .. }
                | terminal_psi::OperationKind::CallStructural { .. }
                | terminal_psi::OperationKind::CallStructuralWithScalarArguments { .. }))
        .filter_map(|operation| operation.result.structural())
        .filter(|result| result.multiplicity == StructuralMultiplicity::Unrestricted
            && result.qualifications.is_empty() && result.projected_qualifications.is_empty()
            && result.claims.is_empty()
            && self.structural_types.get(&result.structural_type).is_some_and(|declaration|
                matches!(&declaration.shape, StructuralTypeShape::Record { fields }
                    if fields.iter().all(|field| !field.relevance.is_erased()
                        && matches!(field.field_type, StructuralFieldType::Scalar(_) | StructuralFieldType::IeeeFloat(_)))))) {
            self.structural_values.remove(&result.place);
        }
    }

    /// By-value unrestricted records copy their payload; only borrowed arguments
    /// retain the caller's referent. Missing host fields stay missing in the copy
    /// and still fail if read, rather than becoming fabricated zero values.
    pub(super) fn copy_owned_scalar_record_arguments(
        &mut self,
        parameters: &[StructuralParameterDeclaration],
        values: &mut std::collections::BTreeMap<
            semantic_vocabulary::PlaceId,
            TerminalStructuralValue,
        >,
    ) -> Result<(), TerminalInterpretError> {
        for parameter in parameters {
            if parameter.access != StructuralAccess::Owned
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                continue;
            }
            let Some(StructuralTypeShape::Record { fields }) = self
                .structural_types
                .get(&parameter.structural_type)
                .map(|declaration| &declaration.shape)
            else {
                continue;
            };
            if fields.iter().any(|field| {
                field.relevance.is_erased()
                    || !matches!(
                        field.field_type,
                        StructuralFieldType::Scalar(_) | StructuralFieldType::IeeeFloat(_)
                    )
            }) {
                continue;
            }
            let value = values.get_mut(&parameter.place).ok_or(
                TerminalInterpretError::VerifiedStructuralPlaceMissing(parameter.place),
            )?;
            let source = StructuralRuntimePlace::from(&*value);
            let identity = self.primitive_local_identities.allocate()?;
            let destination = TerminalStructuralValue {
                opaque_identity: identity,
                structural_type: value.structural_type,
                qualifications: value.qualifications.clone(),
                path: Vec::new(),
            };
            let parent = StructuralRuntimePlace::from(&destination);
            for field in fields {
                if let Some(scalar) = self
                    .structural_scalar_fields
                    .get(&StructuralScalarRuntimeField {
                        parent: source.clone(),
                        field: field.id,
                    })
                    .copied()
                {
                    self.structural_scalar_fields.insert(
                        StructuralScalarRuntimeField {
                            parent: parent.clone(),
                            field: field.id,
                        },
                        scalar,
                    );
                }
            }
            *value = destination;
        }
        Ok(())
    }

    pub(super) fn execute_scalar_record_establishment(
        &mut self,
        operation: &Operation,
        fields: &[ScalarRecordFieldValue],
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let result = operation.result.structural().ok_or_else(invalid)?;
        if !matches!(
            result.multiplicity,
            StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
        ) || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
        {
            return Err(invalid());
        }
        let Some(StructuralTypeShape::Record {
            fields: declarations,
        }) = self
            .structural_types
            .get(&result.structural_type)
            .map(|declaration| &declaration.shape)
        else {
            return Err(invalid());
        };
        if declarations.len() != fields.len() {
            return Err(invalid());
        }
        let mut payload = Vec::with_capacity(fields.len());
        for (declaration, binding) in declarations.iter().zip(fields) {
            let value = self
                .values
                .get(&binding.value)
                .copied()
                .ok_or(TerminalInterpretError::VerifiedValueMissing(binding.value))?;
            if declaration.id != binding.field
                || declaration.relevance.is_erased()
                || !matches!(
                    declaration.field_type,
                    StructuralFieldType::Scalar(_) | StructuralFieldType::IeeeFloat(_)
                )
                || declaration.field_type.scalar_type() != Some(value.scalar_type())
                || !terminal_scalar_belongs_to_type(value)
            {
                return Err(invalid());
            }
            payload.push((binding.field, value));
        }
        // The same allocator excludes host referents and all prior activation
        // locals, regardless of whether their storage is scalar or structural.
        let identity = self.primitive_local_identities.allocate()?;
        let value = TerminalStructuralValue {
            opaque_identity: identity,
            structural_type: result.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        };
        let parent = StructuralRuntimePlace::from(&value);
        for (field, scalar) in payload {
            self.structural_scalar_fields.insert(
                StructuralScalarRuntimeField {
                    parent: parent.clone(),
                    field,
                },
                scalar,
            );
        }
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
}
