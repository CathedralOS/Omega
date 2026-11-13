/*
Marshal `BuildTimeValue` arguments and results across the Terminal interpreter
boundary (`input_contents`, `scalar_argument`, `result_value`).

Build-time evaluation and Build calling-policy execution both run admitted
Terminal entries on compiler-materialized values, so they share this single
projection rather than each keeping a private copy.

`input_contents` projects a value into the formal parameter's
`StructuralTypeId` namespace (`TerminalStructuralContents`), sorting paths
canonically so the interpreter can verify field completeness, stored scalar
widths, predicates, erased-field exclusions, and memory custody before
materializing storage. When matching sum variants (`BuildTimeValue::Case`), we
strip any leading `Type::` prefix from the compiler-owned descriptor label
because the formal `StructuralTypeId` already owns the case namespace. When
writing unsigned integer scalars (`<= 64` bits), we reinterpret the `i64`
carrier bit-pattern as `u64` because `BuildTimeValue::Int` transports `u64` IDs
and bitmasks in two's-complement form.

`result_value` observes the completed structural return value from the
`TerminalInstance` using only the instance's own canonical structural catalog;
`scalar_result` converts a returned scalar the same way a scalar leaf is read.
*/

use crate::BuildTimeValue;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Path, IntegerValue, ScalarType, StructuralTypeId,
};
use terminal_interpreter::{
    TerminalBoundaryStorageValue, TerminalInstance, TerminalProgram, TerminalScalarValue,
    TerminalStorageContents, TerminalStorageObservationValue, TerminalStructuralContents,
};
use terminal_psi::{StructuralFieldDeclaration, StructuralFieldType, StructuralTypeShape};

pub fn input_contents(
    program: &TerminalProgram<'_>,
    argument_index: u32,
    structural_type: StructuralTypeId,
    value: &BuildTimeValue,
) -> Result<TerminalStructuralContents, String> {
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
        (StructuralTypeShape::PrimitiveScalar(scalar), _) => {
            write_scalar(*scalar, *scalar, value, path, values)
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
                write_scalar(scalar, stored, value, path, values)
            }
        };
        path.pop();
        result?;
    }
    Ok(())
}

/// Convert one scalar argument to the entry parameter's declared scalar type.
pub fn scalar_argument(
    scalar: ScalarType,
    value: &BuildTimeValue,
) -> Result<TerminalScalarValue, String> {
    scalar_value(scalar, value)
}

fn scalar_value(scalar: ScalarType, value: &BuildTimeValue) -> Result<TerminalScalarValue, String> {
    Ok(match (scalar, value) {
        (ScalarType::Boolean, BuildTimeValue::Bool(value)) => TerminalScalarValue::Boolean(*value),
        (ScalarType::Integer(integer), BuildTimeValue::Int(value)) if integer.bits() <= 64 => {
            TerminalScalarValue::Integer {
                scalar_type: integer,
                value: if integer.sign() == semantic_vocabulary::IntegerSign::Signed {
                    IntegerValue::Signed(i128::from(*value))
                } else {
                    IntegerValue::Unsigned(u128::from(*value as u64))
                },
            }
        }
        _ => return Err("Terminal input scalar differs from its declared logical type".into()),
    })
}

/// Convert a returned scalar to the compiler's value vocabulary.
pub fn scalar_result(value: TerminalScalarValue) -> Result<BuildTimeValue, String> {
    match value {
        TerminalScalarValue::Boolean(value) => Ok(BuildTimeValue::Bool(value)),
        TerminalScalarValue::Integer { value, scalar_type } if scalar_type.bits() <= 64 => {
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
    scalar: ScalarType,
    stored_scalar: ScalarType,
    value: &BuildTimeValue,
    path: &[Path],
    values: &mut Vec<TerminalBoundaryStorageValue>,
) -> Result<(), String> {
    let value = scalar_value(scalar, value)?;
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
) -> Result<BuildTimeValue, String> {
    read_structural(instance, structural_type, &mut Vec::new())
}

fn read_structural(
    instance: &TerminalInstance<'_, '_>,
    structural_type: StructuralTypeId,
    path: &mut Vec<Path>,
) -> Result<BuildTimeValue, String> {
    let program = instance.program();
    let declaration = program
        .structural_type(structural_type)
        .ok_or("Terminal result lost its structural declaration")?;
    match &declaration.shape {
        StructuralTypeShape::Record { fields, .. } => Ok(BuildTimeValue::Struct {
            type_name: label(program, declaration.identity)?.to_owned(),
            fields: read_fields(instance, fields, path)?,
        }),
        StructuralTypeShape::Sum { cases } => {
            let observed = instance
                .observe_result(path)
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
            let payload = read_fields(instance, &case.fields, path);
            path.pop();
            Ok(BuildTimeValue::Case {
                variant: label(program, case.identity)?.to_owned(),
                payload: payload?,
            })
        }
        StructuralTypeShape::FixedArray { element, length } => {
            let length = length
                .literal()
                .ok_or("Terminal result array extent is not closed")?;
            let mut elements = Vec::new();
            for ordinal in 0..length {
                path.push(Path::FixedIndex(ordinal));
                let result = read_structural(instance, *element, path);
                path.pop();
                elements.push(result?);
            }
            Ok(BuildTimeValue::Array(elements))
        }
        StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::ByteSequence(_) => {
            read_leaf(instance, path)
        }
        _ => Err("Terminal result has no plain vocabulary representation".into()),
    }
}

fn read_fields(
    instance: &TerminalInstance<'_, '_>,
    fields: &[StructuralFieldDeclaration],
    path: &mut Vec<Path>,
) -> Result<Vec<(String, BuildTimeValue)>, String> {
    let mut values = Vec::with_capacity(fields.len());
    for field in fields {
        if field.relevance.is_erased() {
            return Err("Terminal result cannot publish erased evidence".into());
        }
        path.push(Path::Field(field.id));
        let value = match field.field_type {
            StructuralFieldType::Structural(child) => read_structural(instance, child, path),
            _ => read_leaf(instance, path),
        };
        path.pop();
        values.push((
            label(instance.program(), field.identity)?.to_owned(),
            value?,
        ));
    }
    Ok(values)
}

pub fn read_leaf(
    instance: &TerminalInstance<'_, '_>,
    path: &[Path],
) -> Result<BuildTimeValue, String> {
    match instance
        .observe_result(path)
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
            value: TerminalScalarValue::Boolean(value),
            ..
        }) => Ok(BuildTimeValue::Bool(value)),
        TerminalStorageObservationValue::Storage(TerminalStorageContents::Scalar {
            value: TerminalScalarValue::Integer { value, scalar_type },
            ..
        }) if scalar_type.bits() <= 64 => integer_value(value),
        _ => Err("Terminal result leaf is outside the compiler-owned vocabulary".into()),
    }
}
