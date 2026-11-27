//! Project Build values into an interpreter entry and read its declared result.
#![forbid(unsafe_code)]

use build_model::values::BuildTimeValue;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Path, ClosedScalarType, IntegerValue, ScalarType,
    StructuralTypeId,
};
use terminal_interpreter::{
    TerminalBoundaryStorageValue, TerminalInstance, TerminalObservationRoot, TerminalProgram,
    TerminalScalarValue, TerminalStorageContents, TerminalStorageObservationValue,
    TerminalStructuralContents,
};
use terminal_psi::{StructuralFieldDeclaration, StructuralFieldType, StructuralTypeShape};

// Runtime widths come from the exact published program. This lookup never
// resolves a binder or closes an independently supplied type at a use site.
fn closed_types<'program>(
    program: &'program TerminalProgram<'_>,
) -> Result<&'program terminal_codec::ClosedTypeCatalog, String> {
    program.closed_types().map_err(|error| {
        format!("Terminal value requires its published closed type owner: {error:?}")
    })
}

pub fn input_contents(
    program: &TerminalProgram<'_>,
    argument_index: u32,
    structural_type: StructuralTypeId,
    value: &BuildTimeValue,
) -> Result<TerminalStructuralContents, String> {
    closed_types(program)?;
    let mut values = Vec::new();
    write_structural(
        program,
        structural_type,
        value,
        &mut Vec::new(),
        &mut values,
    )?;
    values.sort_unstable_by(|left, right| left.path.cmp(&right.path));
    Ok(TerminalStructuralContents {
        argument_index,
        values,
    })
}

fn label<'program>(
    program: &'program TerminalProgram<'_>,
    identity: terminal_psi::TerminalIdentityIndex,
) -> Result<&'program str, String> {
    program
        .module()
        .identities
        .text(identity)
        .ok_or_else(|| "Terminal vocabulary lost its declaration label".to_owned())
}

fn write_structural(
    program: &TerminalProgram<'_>,
    structural_type: StructuralTypeId,
    value: &BuildTimeValue,
    path: &mut Vec<Path>,
    values: &mut Vec<TerminalBoundaryStorageValue>,
) -> Result<(), String> {
    let declaration = program
        .structural_type(structural_type)
        .ok_or_else(|| "Terminal input lost its structural declaration".to_owned())?;
    match (&declaration.shape, value) {
        (StructuralTypeShape::ByteSequence(carrier), BuildTimeValue::Text(bytes))
            if carrier.is_shared_readable_view() =>
        {
            values.push(TerminalBoundaryStorageValue {
                path: path.clone(),
                contents: TerminalStorageContents::Bytes(bytes.as_slice().into()),
            });
            Ok(())
        }
        (
            StructuralTypeShape::Record { fields, .. },
            BuildTimeValue::Struct {
                fields: supplied, ..
            },
        ) => write_fields(program, fields, supplied, path, values),
        (StructuralTypeShape::Sum { cases }, BuildTimeValue::Case { variant, payload }) => {
            let variant = variant.rsplit("::").next().unwrap_or(variant);
            let mut matching = cases
                .iter()
                .filter(|case| program.module().identities.text(case.identity) == Some(variant));
            let selected = matching
                .next()
                .ok_or_else(|| format!("Terminal input has no `{variant}` case"))?;
            if matching.next().is_some() {
                return Err(format!("Terminal input has ambiguous `{variant}` cases"));
            }
            values.push(TerminalBoundaryStorageValue {
                path: path.clone(),
                contents: TerminalStorageContents::Case(selected.id),
            });
            path.push(Path::Case(selected.id));
            let result = write_fields(program, &selected.fields, payload, path, values);
            path.pop();
            result
        }
        (StructuralTypeShape::FixedArray { element, length }, BuildTimeValue::Array(elements)) => {
            let length = length
                .literal()
                .ok_or("Terminal input array extent is not closed")?;
            if usize::try_from(length).ok() != Some(elements.len()) {
                return Err("Terminal input array differs from its declared extent".into());
            }
            for (ordinal, element_value) in elements.iter().enumerate() {
                path.push(Path::FixedIndex(ordinal as u64));
                let result = write_structural(program, *element, element_value, path, values);
                path.pop();
                result?;
            }
            Ok(())
        }
        (
            StructuralTypeShape::ElementView {
                element,
                access: None | Some(terminal_psi::StructuralAccess::SharedBorrow),
            },
            BuildTimeValue::Array(elements),
        ) => {
            let mut contents = Vec::new();
            let mut element_path = vec![Path::FixedIndex(0)];
            for (ordinal, value) in elements.iter().enumerate() {
                element_path[0] = Path::FixedIndex(
                    u64::try_from(ordinal)
                        .map_err(|_| "Terminal sequence input exceeds its index namespace")?,
                );
                write_structural(program, *element, value, &mut element_path, &mut contents)?;
            }
            contents.sort_unstable_by(|left, right| left.path.cmp(&right.path));
            values.push(TerminalBoundaryStorageValue {
                path: path.clone(),
                contents: TerminalStorageContents::Elements {
                    length: elements.len(),
                    values: contents.into(),
                },
            });
            Ok(())
        }
        (StructuralTypeShape::PrimitiveScalar(scalar), _) => {
            write_scalar(program, *scalar, *scalar, value, path, values)
        }
        _ => Err("Terminal input is not a plain value of its exact formal shape".into()),
    }
}

fn write_fields(
    program: &TerminalProgram<'_>,
    fields: &[StructuralFieldDeclaration],
    supplied: &[(String, BuildTimeValue)],
    path: &mut Vec<Path>,
    values: &mut Vec<TerminalBoundaryStorageValue>,
) -> Result<(), String> {
    if fields.len() != supplied.len() {
        return Err("Terminal input differs from its complete field roster".into());
    }
    for field in fields {
        if field.relevance.is_erased() {
            return Err("Terminal input cannot manufacture erased evidence".into());
        }
        let name = label(program, field.identity)?;
        let mut matching = supplied.iter().filter(|(candidate, _)| candidate == name);
        let (_, value) = matching
            .next()
            .ok_or_else(|| format!("Terminal input is missing `{name}`"))?;
        if matching.next().is_some() {
            return Err(format!("Terminal input repeats `{name}`"));
        }
        path.push(Path::Field(field.id));
        let result = match &field.field_type {
            StructuralFieldType::Structural(child) => {
                write_structural(program, *child, value, path, values)
            }
            kind if kind
                .byte_sequence_carrier()
                .is_some_and(|carrier| carrier.is_shared_readable_view()) =>
            {
                let BuildTimeValue::Text(bytes) = value else {
                    return Err("Terminal byte field requires actual immutable contents".into());
                };
                if !kind.byte_predicates().holds_for(bytes) {
                    return Err("Terminal byte contents do not satisfy the field predicates".into());
                }
                values.push(TerminalBoundaryStorageValue {
                    path: path.clone(),
                    contents: TerminalStorageContents::Bytes(bytes.as_slice().into()),
                });
                Ok(())
            }
            kind => {
                let scalar = kind
                    .scalar_type()
                    .ok_or("Terminal input field is not plain scalar storage")?;
                let stored = field
                    .storage
                    .as_ref()
                    .and_then(|storage| storage.stored_scalar())
                    .unwrap_or(scalar);
                write_scalar(program, scalar, stored, value, path, values)
            }
        };
        path.pop();
        result?;
    }
    Ok(())
}

fn scalar_value(
    owner: &terminal_codec::ClosedTypeCatalog,
    scalar: ScalarType,
    value: &BuildTimeValue,
) -> Result<TerminalScalarValue, String> {
    let closed = owner.scalar(scalar).map_err(|error| {
        format!("Terminal input scalar is not in its published type owner: {error:?}")
    })?;
    Ok(match (scalar, closed, value) {
        (ScalarType::Boolean, ClosedScalarType::Boolean, BuildTimeValue::Bool(value)) => {
            TerminalScalarValue::Boolean(*value)
        }
        (
            ScalarType::IeeeFloat(_),
            ClosedScalarType::IeeeFloat(format),
            BuildTimeValue::Float(value),
        ) if value.format() == format => TerminalScalarValue::IeeeFloat(*value),
        (
            ScalarType::IeeeFloat(_),
            ClosedScalarType::IeeeFloat(format),
            BuildTimeValue::UndeterminedNaN(actual),
        ) if *actual == format => TerminalScalarValue::UndeterminedNaN(format),
        (
            ScalarType::Integer(raw),
            ClosedScalarType::Integer(integer),
            BuildTimeValue::Int(value),
        ) if integer.bits() <= 64 => {
            let value = if integer.sign() == semantic_vocabulary::IntegerSign::Signed {
                IntegerValue::Signed(i128::from(*value))
            } else {
                IntegerValue::Unsigned(u128::from(*value as u64))
            };
            if !raw.admits(value) {
                return Err("Terminal input integer exceeds its declared range".into());
            }
            TerminalScalarValue::Integer {
                scalar_type: raw,
                value,
            }
        }
        _ => return Err("Terminal input scalar differs from its declared logical type".into()),
    })
}

/// Convert a returned scalar to the compiler's value vocabulary.
pub fn scalar_result(
    program: &TerminalProgram<'_>,
    value: TerminalScalarValue,
) -> Result<BuildTimeValue, String> {
    let owner = closed_types(program)?;
    match value {
        TerminalScalarValue::Boolean(value) => Ok(BuildTimeValue::Bool(value)),
        TerminalScalarValue::IeeeFloat(value) => Ok(BuildTimeValue::Float(value)),
        TerminalScalarValue::UndeterminedNaN(format) => Ok(BuildTimeValue::UndeterminedNaN(format)),
        TerminalScalarValue::Integer { value, scalar_type } => {
            let integer = owner.integer(scalar_type).map_err(|error| {
                format!("Terminal result scalar is not in its published type owner: {error:?}")
            })?;
            if integer.bits() > 64 {
                return Err(
                    "Terminal result scalar is outside the compiler-owned vocabulary".into(),
                );
            }
            integer_value(value)
        }
        _ => Err("Terminal result scalar is outside the compiler-owned vocabulary".into()),
    }
}

fn integer_value(value: IntegerValue) -> Result<BuildTimeValue, String> {
    Ok(BuildTimeValue::Int(match value {
        IntegerValue::Signed(value) => {
            i64::try_from(value).map_err(|_| "Terminal result signed integer exceeds i64")?
        }
        IntegerValue::Unsigned(value) => {
            u64::try_from(value).map_err(|_| "Terminal result unsigned integer exceeds u64")? as i64
        }
    }))
}

fn write_scalar(
    program: &TerminalProgram<'_>,
    scalar: ScalarType,
    stored_scalar: ScalarType,
    value: &BuildTimeValue,
    path: &[Path],
    values: &mut Vec<TerminalBoundaryStorageValue>,
) -> Result<(), String> {
    let owner = closed_types(program)?;
    owner.scalar(stored_scalar).map_err(|error| {
        format!("Terminal stored scalar is not in its published type owner: {error:?}")
    })?;
    let value = scalar_value(owner, scalar, value)?;
    values.push(TerminalBoundaryStorageValue {
        path: path.to_vec(),
        contents: TerminalStorageContents::Scalar {
            value,
            stored_scalar,
        },
    });
    Ok(())
}

pub fn result_value(
    instance: &TerminalInstance<'_, '_>,
    structural_type: StructuralTypeId,
    opaque_inputs: &[(
        &terminal_interpreter::TerminalStructuralValue,
        &BuildTimeValue,
    )],
) -> Result<BuildTimeValue, String> {
    closed_types(instance.program())?;
    if opaque_inputs.windows(2).any(|pair| {
        (pair[0].0.opaque_identity, pair[0].0.structural_type)
            >= (pair[1].0.opaque_identity, pair[1].0.structural_type)
    }) {
        return Err("opaque result bindings are not in unique input identity order".into());
    }
    read_structural(
        instance,
        TerminalObservationRoot::Result,
        structural_type,
        &mut Vec::new(),
        opaque_inputs,
    )
}

/// Read a field from a typed borrowed argument at the interpreter state its caller requires.
pub fn field_value(
    instance: &TerminalInstance<'_, '_>,
    root: TerminalObservationRoot,
    field: &StructuralFieldDeclaration,
) -> Result<BuildTimeValue, String> {
    closed_types(instance.program())?;
    if field.relevance.is_erased() {
        return Err("cannot project an erased field".to_owned());
    }
    let mut path = vec![Path::Field(field.id)];
    match field.field_type {
        StructuralFieldType::Structural(child) => {
            read_structural(instance, root, child, &mut path, &[])
        }
        _ => read_leaf(instance, root, &path),
    }
}

fn read_structural(
    instance: &TerminalInstance<'_, '_>,
    root: TerminalObservationRoot,
    structural_type: StructuralTypeId,
    path: &mut Vec<Path>,
    opaque_inputs: &[(
        &terminal_interpreter::TerminalStructuralValue,
        &BuildTimeValue,
    )],
) -> Result<BuildTimeValue, String> {
    let program = instance.program();
    let declaration = program
        .structural_type(structural_type)
        .ok_or("Terminal result lost its structural declaration")?;
    match &declaration.shape {
        StructuralTypeShape::TypeParameter => {
            Err("generic result needs a closed application before build-time observation".into())
        }
        StructuralTypeShape::Opaque => {
            let observed = instance
                .observe_storage(root, path)
                .map_err(|error| format!("Terminal opaque result: {error:?}"))?;
            let TerminalStorageObservationValue::Opaque(observed) = observed else {
                return Err("opaque result has no supplying host value".into());
            };
            let position = opaque_inputs
                .binary_search_by_key(
                    &(observed.opaque_identity, observed.structural_type),
                    |(input, _)| (input.opaque_identity, input.structural_type),
                )
                .map_err(|_| "opaque result has no actual supplying context")?;
            let (input, value) = opaque_inputs[position];
            if input != &observed {
                return Err("opaque result changed its retained context custody".into());
            }
            Ok(value.clone())
        }
        StructuralTypeShape::Record { fields, .. } => Ok(BuildTimeValue::Struct {
            type_name: label(program, declaration.identity)?.to_owned(),
            fields: read_fields(instance, root, fields, path, opaque_inputs)?,
        }),
        StructuralTypeShape::Sum { cases } => {
            let observed = instance
                .observe_storage(root, path)
                .map_err(|error| format!("Terminal result tag: {error:?}"))?;
            let TerminalStorageObservationValue::Storage(TerminalStorageContents::Case(selected)) =
                observed
            else {
                return Err("Terminal result has no active case".into());
            };
            let case = cases
                .iter()
                .find(|case| case.id == selected)
                .ok_or("Terminal result case is outside its declaration")?;
            path.push(Path::Case(selected));
            let payload = read_fields(instance, root, &case.fields, path, opaque_inputs);
            path.pop();
            Ok(BuildTimeValue::Case {
                variant: label(program, case.identity)?.to_owned(),
                payload: payload?,
            })
        }
        StructuralTypeShape::ElementView { element, .. } => {
            let length = instance
                .observe_sequence_length(root, path)
                .map_err(|error| format!("Terminal result sequence backing: {error:?}"))?;
            let count = usize::try_from(length)
                .map_err(|_| "Terminal result sequence extent exceeds usize")?;
            let mut elements = Vec::new();
            elements
                .try_reserve_exact(count)
                .map_err(|reason| format!("Terminal result sequence snapshot: {reason}"))?;
            for ordinal in 0..length {
                path.push(Path::FixedIndex(ordinal));
                let result = read_structural(instance, root, *element, path, opaque_inputs);
                path.pop();
                elements
                    .push(result.map_err(|reason| {
                        format!("Terminal result element {ordinal}: {reason}")
                    })?);
            }
            Ok(BuildTimeValue::Array(elements))
        }
        StructuralTypeShape::FixedArray { element, length } => {
            let length = length
                .literal()
                .ok_or("Terminal result array extent is not closed")?;
            let mut elements = Vec::new();
            for ordinal in 0..length {
                path.push(Path::FixedIndex(ordinal));
                let result = read_structural(instance, root, *element, path, opaque_inputs);
                path.pop();
                elements
                    .push(result.map_err(|reason| {
                        format!("Terminal result element {ordinal}: {reason}")
                    })?);
            }
            Ok(BuildTimeValue::Array(elements))
        }
        StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::ByteSequence(_) => {
            read_leaf(instance, root, path)
        }
        _ => Err("Terminal result has no plain vocabulary representation".into()),
    }
}

fn read_fields(
    instance: &TerminalInstance<'_, '_>,
    root: TerminalObservationRoot,
    fields: &[StructuralFieldDeclaration],
    path: &mut Vec<Path>,
    opaque_inputs: &[(
        &terminal_interpreter::TerminalStructuralValue,
        &BuildTimeValue,
    )],
) -> Result<Vec<(String, BuildTimeValue)>, String> {
    let mut values = Vec::with_capacity(fields.len());
    for field in fields {
        if field.relevance.is_erased() {
            return Err("Terminal result cannot publish erased evidence".into());
        }
        path.push(Path::Field(field.id));
        let value = match field.field_type {
            StructuralFieldType::Structural(child) => {
                read_structural(instance, root, child, path, opaque_inputs)
            }
            _ => read_leaf(instance, root, path),
        };
        path.pop();
        let name = label(instance.program(), field.identity)?.to_owned();
        let value = value.map_err(|reason| format!("Terminal result field `{name}`: {reason}"))?;
        values.push((name, value));
    }
    Ok(values)
}

fn read_leaf(
    instance: &TerminalInstance<'_, '_>,
    root: TerminalObservationRoot,
    path: &[Path],
) -> Result<BuildTimeValue, String> {
    closed_types(instance.program())?;
    match instance
        .observe_storage(root, path)
        .map_err(|error| format!("Terminal result contents: {error:?}"))?
    {
        TerminalStorageObservationValue::Bytes(bytes) => {
            let mut copied = vec![0; bytes.len()];
            bytes
                .copy_to(&mut copied)
                .map_err(|error| format!("Terminal result bytes: {error:?}"))?;
            Ok(BuildTimeValue::Text(copied))
        }
        TerminalStorageObservationValue::Storage(TerminalStorageContents::Scalar {
            value, ..
        }) => scalar_result(instance.program(), value),
        _ => Err("Terminal result leaf is outside the compiler-owned vocabulary".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerSign, IntegerType};
    use terminal_psi::{
        StructuralMultiplicity, StructuralTypeDeclaration, TerminalIdentityTableBuilder,
        TerminalModule,
    };

    fn integer_formal(
        sign: IntegerSign,
        bits: u16,
    ) -> (TerminalProgram<'static>, StructuralTypeId) {
        let integer = IntegerType::new(sign, bits).unwrap();
        let mut identities = TerminalIdentityTableBuilder::default();
        let identity = identities.intern("integer_input");
        let id = StructuralTypeId::new(1).unwrap();
        let mut module = TerminalModule::declarations();
        module.identities = identities.build();
        module
            .types
            .structural_types
            .push(StructuralTypeDeclaration {
                declaration_identity: Default::default(),
                quotient_carrier: None,
                id,
                identity,
                owns_linear_claim: false,
                multiplicity: StructuralMultiplicity::Unrestricted,
                no_code_affine_cleanup: false,
                descriptor_cleanup: false,
                nominal_cleanup: None,
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)),
                scalar_restriction: None,
                requires: Vec::new(),
            });
        let closed = terminal_codec::ClosedTerminalModule::new(module).unwrap();
        (TerminalProgram::load(closed.semantic_bytes()).unwrap(), id)
    }

    #[test]
    fn projected_integer_inputs_respect_the_published_width() {
        use IntegerSign::{Signed, Unsigned};
        for (sign, low, high, under, over) in
            [(Signed, -128, 127, -129, 128), (Unsigned, 0, 255, -1, 256)]
        {
            let (program, id) = integer_formal(sign, 8);
            for input in [low, high] {
                input_contents(&program, 0, id, &BuildTimeValue::Int(input)).unwrap();
            }
            for input in [under, over] {
                assert!(
                    input_contents(&program, 0, id, &BuildTimeValue::Int(input)).is_err(),
                    "{sign:?} 8-bit formal accepted {input}"
                );
            }
        }
        // Build integers carry the full unsigned 64-bit result in an i64.
        let (program, id) = integer_formal(Unsigned, 64);
        let contents = input_contents(&program, 0, id, &BuildTimeValue::Int(-1)).unwrap();
        assert!(
            matches!(&contents.values[..], [TerminalBoundaryStorageValue {
            contents: TerminalStorageContents::Scalar {
                value: TerminalScalarValue::Integer { value: IntegerValue::Unsigned(bits), .. }, ..
            }, ..
        }] if *bits == u128::from(u64::MAX))
        );
    }
}
