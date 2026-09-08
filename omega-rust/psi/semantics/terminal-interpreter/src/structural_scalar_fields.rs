use std::collections::BTreeMap;

use semantic_vocabulary::{PlaceId, StructuralFieldId, StructuralTypeId};
use terminal_psi::{OperationKind, StructuralPathSegment, StructuralTypeDeclaration};

use crate::{
    ExecutableMachine, StructuralRuntimePlace, StructuralScalarRuntimeField,
    TerminalInterpretError, TerminalScalarValue, TerminalStructuralBooleanFieldValue,
    TerminalStructuralValue, direct_scalar_field_type, resolve_structural_path_type,
};

#[cfg(test)]
mod tests;

/// Explicit host contents of one relevant scalar field of a structural entry input.
/// `argument_index` is the dense structural position; `path` resolves from that
/// argument's type to the containing record. Field identity is independent of layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalStructuralScalarFieldValue {
    pub argument_index: u32,
    pub path: Vec<StructuralPathSegment>,
    pub field: StructuralFieldId,
    pub value: TerminalScalarValue,
}

impl From<&TerminalStructuralBooleanFieldValue> for TerminalStructuralScalarFieldValue {
    fn from(field: &TerminalStructuralBooleanFieldValue) -> Self {
        Self {
            argument_index: field.argument_index,
            path: field.path.clone(),
            field: field.field,
            value: TerminalScalarValue::Boolean(field.value),
        }
    }
}

pub(crate) fn bind(
    machine: &ExecutableMachine,
    structural_types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    structural_values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    arguments: &[TerminalStructuralScalarFieldValue],
) -> Result<BTreeMap<StructuralScalarRuntimeField, TerminalScalarValue>, TerminalInterpretError> {
    let mut values = BTreeMap::new();
    for argument in arguments {
        let invalid = || TerminalInterpretError::StructuralScalarFieldArgumentInvalid {
            argument_index: argument.argument_index,
            field: argument.field,
        };
        let parameter = machine
            .structural_parameters
            .get(argument.argument_index as usize)
            .ok_or_else(invalid)?;
        let root = structural_values.get(&parameter.place).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(parameter.place),
        )?;
        let parent_type =
            resolve_structural_path_type(structural_types, root.structural_type, &argument.path)
                .map_err(|_| invalid())?;
        if direct_scalar_field_type(structural_types, parent_type, argument.field)
            != Some(argument.value.scalar_type())
            || matches!(argument.value, TerminalScalarValue::Integer { scalar_type, value } if !scalar_type.admits(value))
        {
            return Err(invalid());
        }
        let mut parent = StructuralRuntimePlace::from(root);
        parent.path.extend_from_slice(&argument.path);
        // Key by runtime referent so aliases cannot provide conflicting contents.
        if values
            .insert(
                StructuralScalarRuntimeField {
                    parent,
                    field: argument.field,
                },
                argument.value,
            )
            .is_some()
        {
            return Err(invalid());
        }
    }
    for operation in machine.blocks.values().flat_map(|block| &block.operations) {
        // Preserve the Boolean entry contract. Integer fields can be initialized
        // by earlier stores; their existing runtime read checks missing contents.
        let (source, field) = match operation.kind {
            OperationKind::BooleanStructuralField { source, field } => (source, field),
            _ => continue,
        };
        if !machine
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == source)
        {
            continue;
        }
        let root = structural_values.get(&source).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(source),
        )?;
        if !values.contains_key(&StructuralScalarRuntimeField {
            parent: StructuralRuntimePlace::from(root),
            field,
        }) {
            return Err(TerminalInterpretError::StructuralBooleanFieldMissing { source, field });
        }
    }
    Ok(values)
}
