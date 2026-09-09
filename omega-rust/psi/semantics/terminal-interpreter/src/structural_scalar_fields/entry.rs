//! Validate every bounded entry field before establishing invocation custody.

use super::{
    ExecutableMachine, PlaceId, StructuralScalarRuntimeField, StructuralTypeDeclaration,
    StructuralTypeId, TerminalInterpretError, TerminalScalarValue, TerminalStructuralValue,
};
use crate::{StructuralPathSegment, StructuralRuntimePlace};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use terminal_psi::{StructuralFieldType, StructuralTypeShape};

pub(super) fn validate(
    machine: &ExecutableMachine,
    types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    roots: &BTreeMap<PlaceId, TerminalStructuralValue>,
    values: &BTreeMap<StructuralScalarRuntimeField, TerminalScalarValue>,
) -> Result<(), TerminalInterpretError> {
    for (position, parameter) in machine.structural_parameters.iter().enumerate() {
        let root = roots.get(&parameter.place).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(parameter.place),
        )?;
        validate_type(
            types,
            root.structural_type,
            &mut StructuralRuntimePlace::from(root),
            values,
            u32::try_from(position)
                .map_err(|_| TerminalInterpretError::VerifiedOperationMalformed)?,
            &mut Vec::new(),
        )?;
    }
    Ok(())
}

fn validate_type(
    types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    current: StructuralTypeId,
    parent: &mut StructuralRuntimePlace,
    values: &BTreeMap<StructuralScalarRuntimeField, TerminalScalarValue>,
    argument_index: u32,
    active: &mut Vec<StructuralTypeId>,
) -> Result<(), TerminalInterpretError> {
    if !requires_contents(types, current) {
        return Ok(());
    }
    if active.contains(&current) {
        return Err(TerminalInterpretError::VerifiedOperationMalformed);
    }
    active.push(current);
    let declaration = types
        .get(&current)
        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
    match &declaration.shape {
        StructuralTypeShape::Record { fields } => {
            for field in fields {
                if field.relevance.is_erased() {
                    continue;
                }
                match field.field_type {
                    StructuralFieldType::BoundedInteger(bounds) => {
                        let key = StructuralScalarRuntimeField {
                            parent: parent.clone(),
                            field: field.id,
                        };
                        if !matches!(
                            values.get(&key),
                            Some(TerminalScalarValue::Integer { scalar_type, value })
                                if *scalar_type == bounds.integer_type()
                                    && bounds.contains(*value)
                        ) {
                            return Err(
                                TerminalInterpretError::StructuralScalarFieldArgumentInvalid {
                                    argument_index,
                                    field: field.id,
                                },
                            );
                        }
                    }
                    StructuralFieldType::Structural(child) => {
                        parent
                            .path
                            .push(StructuralPathSegment::Field(field.identity.clone()));
                        validate_type(types, child, parent, values, argument_index, active)?;
                        parent.path.pop();
                    }
                    _ => {}
                }
            }
        }
        StructuralTypeShape::FixedArray { element, length } => {
            // Visit one element at a time. Every bounded leaf needs a distinct
            // supplied path, so a huge array stops at its first missing field
            // without allocating or scanning its remaining elements.
            for position in 0..*length {
                parent
                    .path
                    .push(StructuralPathSegment::FixedIndex(position));
                validate_type(types, *element, parent, values, argument_index, active)?;
                parent.path.pop();
            }
        }
        // The host field carrier supplies no selected case discriminator.
        StructuralTypeShape::Sum { .. } | StructuralTypeShape::Mixed { .. } => {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::ByteSequence(_) => {}
    }
    active.pop();
    Ok(())
}

fn requires_contents(
    types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    root: StructuralTypeId,
) -> bool {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        let Some(declaration) = types.get(&current) else {
            return true;
        };
        let mut fields = Vec::new();
        match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::ByteSequence(_) => {
                continue;
            }
            StructuralTypeShape::FixedArray { element, length } => {
                // An empty array has no entry contents even when its element
                // type contains bounded fields. This also skips arrays of
                // empty arrays without walking their outer dimensions.
                if *length != 0 {
                    pending.push(*element);
                }
                continue;
            }
            StructuralTypeShape::Record { fields: members } => fields.extend(members),
            StructuralTypeShape::Sum { cases } => {
                fields.extend(cases.iter().flat_map(|case| &case.fields));
            }
            StructuralTypeShape::Mixed {
                fields: members,
                cases,
            } => {
                fields.extend(members);
                fields.extend(cases.iter().flat_map(|case| &case.fields));
            }
        }
        for field in fields
            .into_iter()
            .filter(|field| !field.relevance.is_erased())
        {
            match field.field_type {
                StructuralFieldType::BoundedInteger(_) => return true,
                StructuralFieldType::Structural(child) => pending.push(child),
                _ => {}
            }
        }
    }
    false
}
