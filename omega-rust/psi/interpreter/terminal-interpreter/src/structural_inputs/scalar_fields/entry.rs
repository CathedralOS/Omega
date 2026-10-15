//! Validate every bounded entry field before establishing invocation custody.

use crate::errors::malformed;
use crate::structural_inputs::scalar_fields::{
    PlaceId, StructuralScalarRuntimeField, StructuralTypeDeclaration, StructuralTypeId,
    TerminalInterpretError, TerminalMachine, TerminalScalarValue, TerminalStructuralValue,
};
use crate::values::StructuralRuntimePlace;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use terminal_psi::StructuralPathSegment;
use terminal_psi::{StructuralFieldType, StructuralTypeShape};

pub(crate) fn validate(
    machine: &TerminalMachine,
    types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    roots: &BTreeMap<PlaceId, TerminalStructuralValue>,
    values: &BTreeMap<StructuralScalarRuntimeField, TerminalScalarValue>,
    zero_initialized_arguments: &[u32],
) -> Result<(), TerminalInterpretError> {
    for (position, parameter) in machine.structural_parameters.iter().enumerate() {
        // These roots already have actual storage from the shared zero
        // materializer, which authenticated every instantiated restriction and
        // selected sum payload. This is not eligibility inferred from a type.
        if zero_initialized_arguments.contains(&(position as u32)) {
            continue;
        }
        let root = crate::custody::verified_structural_place(roots, parameter.place)?;
        validate_type(
            types,
            root.structural_type,
            &mut StructuralRuntimePlace::from(root),
            values,
            u32::try_from(position).map_err(|_| malformed())?,
            &mut Vec::new(),
        )?;
    }
    Ok(())
}

fn validate_type(
    types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
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
        return Err(malformed());
    }
    active.push(current);
    let declaration = types.get(&current).ok_or(malformed())?;
    match &declaration.shape {
        StructuralTypeShape::Record { fields, .. } => {
            for field in fields {
                if field.relevance.is_erased() {
                    continue;
                }
                match &field.field_type {
                    field_type
                        if terminal_semantics::scalar_field_requires_evidence(field_type) =>
                    {
                        let key = StructuralScalarRuntimeField::member(parent.clone(), field.id);
                        if !values
                            .get(&key)
                            .is_some_and(|value| value.satisfies_field(field_type))
                        {
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
                        validate_type(types, *child, parent, values, argument_index, active)?;
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
        terminal_psi::sum_or_mixed_shape_pattern!() | StructuralTypeShape::Reference { .. } => {
            return Err(malformed());
        }
        // The already supplied binding identity has no scalar payload. This
        // content walk neither creates the binding nor establishes authority.
        StructuralTypeShape::BindingCarrier
        | StructuralTypeShape::PrimitiveScalar(_)
        | StructuralTypeShape::ByteSequence(_)
        | StructuralTypeShape::ElementView { .. } => {}
    }
    active.pop();
    Ok(())
}

fn requires_contents(
    types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
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
            StructuralTypeShape::Reference { .. } => return true,
            StructuralTypeShape::BindingCarrier
            | StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::ByteSequence(_) => {
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
            // A borrowed view's element is referent-owned; traverse it so the
            // reachability answer tracks the viewed element type.
            StructuralTypeShape::ElementView { element, .. } => {
                pending.push(*element);
                continue;
            }
            StructuralTypeShape::Record {
                fields: members, ..
            } => fields.extend(members),
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
                StructuralFieldType::BoundedInteger(_) | StructuralFieldType::RefinedScalar(_) => {
                    return true;
                }
                StructuralFieldType::Structural(child) => pending.push(child),
                _ => {}
            }
        }
    }
    false
}
