//! Exchange the calling vocabulary with retained Terminal code.
//!
//! Descriptor labels decode compiler-owned vocabulary; they never select a
//! callable or establish a nominal qualification. Entry selection and storage
//! formation remain the interpreter's artifact-bound admission operations.

use build_time_evaluation::BuildTimeValue;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Path, IntegerValue, ScalarType, StructuralTypeId,
};
use terminal_interpreter::{
    TerminalBoundaryStorageValue, TerminalInstance, TerminalProgram, TerminalScalarValue,
    TerminalStorageContents, TerminalStorageObservationValue, TerminalStructuralContents,
};
use terminal_psi::{StructuralFieldDeclaration, StructuralFieldType, StructuralTypeShape};

/// Project a plain vocabulary argument into the exact formal's declaration
/// namespace. The SDK subsequently checks completeness, stored widths,
/// predicates, erased obligations and custody before publishing any storage.
pub(super) fn input_contents(
    program: &TerminalProgram<'_>,
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
        argument_index: 0,
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
        .ok_or_else(|| "calling vocabulary lost its declaration label".to_owned())
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
        .ok_or_else(|| "calling input lost its structural declaration".to_owned())?;
    match (&declaration.shape, value) {
        (
            StructuralTypeShape::Record { fields, .. },
            BuildTimeValue::Struct {
                fields: supplied, ..
            },
        ) => write_fields(program, fields, supplied, path, values),
        (StructuralTypeShape::Sum { cases }, BuildTimeValue::Case { variant, payload }) => {
            let mut matching = cases.iter().filter(|case| {
                program.module().identities.text(case.identity) == Some(variant.as_str())
            });
            let selected = matching
                .next()
                .ok_or_else(|| format!("calling input has no `{variant}` case"))?;
            if matching.next().is_some() {
                return Err(format!("calling input has ambiguous `{variant}` cases"));
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
                .ok_or("calling input array extent is not closed")?;
            if usize::try_from(length).ok() != Some(elements.len()) {
                return Err("calling input array differs from its declared extent".into());
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
        _ => Err("calling input is not a plain value of its exact formal shape".into()),
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
        return Err("calling input differs from its complete field roster".into());
    }
    for field in fields {
        if field.relevance.is_erased() {
            return Err("calling input cannot manufacture erased evidence".into());
        }
        let name = label(program, field.identity)?;
        let mut matching = supplied.iter().filter(|(candidate, _)| candidate == name);
        let (_, value) = matching
            .next()
            .ok_or_else(|| format!("calling input is missing `{name}`"))?;
        if matching.next().is_some() {
            return Err(format!("calling input repeats `{name}`"));
        }
        path.push(Path::Field(field.id));
        let result = match &field.field_type {
            StructuralFieldType::Structural(child) => {
                write_structural(program, *child, value, path, values)
            }
            kind => {
                let scalar = kind
                    .scalar_type()
                    .ok_or("calling input field is not plain scalar storage")?;
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

fn write_scalar(
    scalar: ScalarType,
    stored_scalar: ScalarType,
    value: &BuildTimeValue,
    path: &[Path],
    values: &mut Vec<TerminalBoundaryStorageValue>,
) -> Result<(), String> {
    let value = match (scalar, value) {
        (ScalarType::Boolean, BuildTimeValue::Bool(value)) => TerminalScalarValue::Boolean(*value),
        (ScalarType::Integer(integer), BuildTimeValue::Int(value)) if integer.bits() <= 64 => {
            TerminalScalarValue::Integer {
                scalar_type: integer,
                value: if integer.sign() == semantic_vocabulary::IntegerSign::Signed {
                    IntegerValue::Signed(i128::from(*value))
                } else {
                    // The existing calling vocabulary transports u64 bit
                    // patterns through BuildTimeValue::Int, including IDs.
                    IntegerValue::Unsigned(u128::from(*value as u64))
                },
            }
        }
        _ => return Err("calling input scalar differs from its declared logical type".into()),
    };
    values.push(TerminalBoundaryStorageValue {
        path: path.to_vec(),
        contents: TerminalStorageContents::Scalar {
            value,
            stored_scalar,
        },
    });
    Ok(())
}

/// Decode the actual completed result. Paths and tags come from this
/// invocation's canonical catalog; no template or checked tree is consulted.
pub(super) fn result_value(
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
        .ok_or("calling result lost its structural declaration")?;
    match &declaration.shape {
        StructuralTypeShape::Record { fields, .. } => Ok(BuildTimeValue::Struct {
            type_name: label(program, declaration.identity)?.to_owned(),
            fields: read_fields(instance, fields, path)?,
        }),
        StructuralTypeShape::Sum { cases } => {
            let observed = instance
                .observe_result(path)
                .map_err(|error| format!("calling result tag: {error:?}"))?;
            let TerminalStorageObservationValue::Storage(TerminalStorageContents::Case(selected)) =
                observed
            else {
                return Err("calling result has no active case".into());
            };
            let case = cases
                .iter()
                .find(|case| case.id == selected)
                .ok_or("calling result case is outside its declaration")?;
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
                .ok_or("calling result array extent is not closed")?;
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
        _ => Err("calling result has no plain vocabulary representation".into()),
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
            return Err("calling result cannot publish erased evidence".into());
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

fn read_leaf(instance: &TerminalInstance<'_, '_>, path: &[Path]) -> Result<BuildTimeValue, String> {
    match instance
        .observe_result(path)
        .map_err(|error| format!("calling result contents: {error:?}"))?
    {
        TerminalStorageObservationValue::Bytes(bytes) => {
            let mut copied = vec![0; bytes.len()];
            bytes
                .copy_to(&mut copied)
                .map_err(|error| format!("calling result bytes: {error:?}"))?;
            Ok(BuildTimeValue::Text(copied))
        }
        TerminalStorageObservationValue::Storage(TerminalStorageContents::Scalar {
            value: TerminalScalarValue::Boolean(value),
            ..
        }) => Ok(BuildTimeValue::Bool(value)),
        TerminalStorageObservationValue::Storage(TerminalStorageContents::Scalar {
            value: TerminalScalarValue::Integer { value, scalar_type },
            ..
        }) if scalar_type.bits() <= 64 => Ok(BuildTimeValue::Int(match value {
            IntegerValue::Signed(value) => {
                i64::try_from(value).map_err(|_| "calling result signed integer exceeds i64")?
            }
            IntegerValue::Unsigned(value) => u64::try_from(value)
                .map_err(|_| "calling result unsigned integer exceeds u64")?
                as i64,
        })),
        _ => Err("calling result leaf is outside the compiler-owned vocabulary".into()),
    }
}
