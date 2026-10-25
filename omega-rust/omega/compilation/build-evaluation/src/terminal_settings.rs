//! Decode settings from the original Build backing after normal return.
//!
//! The admitted initial value supplies result-descriptor labels, not nominal
//! authority. Runtime paths and selected cases come from this instance's own
//! canonical catalog. The existing Build admission still validates the source
//! vocabulary, and its ordinary result validator owns configuration rules.
//! Host facets and product selections are not settings and are never copied.

use build_time_evaluation::BuildTimeValue;
use package_compilation::build_execution::build_value::{
    BuildValueAdmission, EvaluatedBuildSettings,
};
use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, IntegerValue, StructuralTypeId};
use terminal_interpreter::{
    TerminalEntryStorageValue, TerminalInstance, TerminalScalarValue, TerminalStorageContents,
};
use terminal_psi::{StructuralFieldDeclaration, StructuralFieldType, StructuralTypeShape};

/// Read the same settings consumed by checked-tree Build execution. The caller
/// retains the admission and initial value of this activation; this operation
/// neither runs Build nor creates an execution/accounting receipt.
pub fn extract(
    instance: &TerminalInstance<'_, '_>,
    admission: BuildValueAdmission,
    initial: &BuildTimeValue,
) -> Result<
    (
        EvaluatedBuildSettings,
        optimization_core::OptimizationReportRequest,
    ),
    String,
> {
    let BuildTimeValue::Struct {
        type_name,
        fields: initial_fields,
    } = initial
    else {
        return Err("admitted Build input is not a record".to_owned());
    };
    let entry = instance
        .program()
        .entry()
        .map_err(|error| format!("Build entry: {error:?}"))?;
    let [parameter] = entry.structural_parameters else {
        return Err("Build settings require one original structural argument".to_owned());
    };
    let declaration = instance
        .program()
        .structural_type(parameter.structural_type)
        .ok_or_else(|| "Build entry lost its structural declaration".to_owned())?;
    let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
        return Err("Build entry backing is not a record".to_owned());
    };
    let mut values = Vec::new();
    for (name, template) in initial_fields {
        if !matches!(
            name.as_str(),
            "subsystem"
                | "identifier"
                | "freestanding"
                | "optimizations"
                | "x86_deployment_features"
                | "privileged_services"
                | "elf_interpreter"
        ) {
            continue;
        }
        let mut selected = None;
        for field in fields {
            if label(instance, field.identity)? != name {
                continue;
            }
            if selected.replace(field).is_some() {
                return Err(format!("Build backing has ambiguous `{name}` fields"));
            }
        }
        let field =
            selected.ok_or_else(|| format!("Build backing has no admitted `{name}` field"))?;
        let mut path = vec![Path::Field(field.id)];
        let value = read_field(instance, &mut path, field, Some(template))?;
        values.push((name.clone(), value));
    }
    admission.extract(&BuildTimeValue::Struct {
        type_name: type_name.clone(),
        fields: values,
    })
}

fn label<'instance>(
    instance: &'instance TerminalInstance<'_, '_>,
    identity: terminal_psi::TerminalIdentityIndex,
) -> Result<&'instance str, String> {
    instance
        .program()
        .module()
        .identities
        .text(identity)
        .ok_or_else(|| "Build result declaration has no descriptor label".to_owned())
}

fn read_field(
    instance: &TerminalInstance<'_, '_>,
    path: &mut Vec<Path>,
    field: &StructuralFieldDeclaration,
    template: Option<&BuildTimeValue>,
) -> Result<BuildTimeValue, String> {
    if field.relevance.is_erased() {
        return Err("Build settings cannot copy an erased field".to_owned());
    }
    if let StructuralFieldType::Structural(structural_type) = field.field_type {
        return read_structural(instance, path, structural_type, template);
    }
    read_leaf(instance, path)
}

fn read_structural(
    instance: &TerminalInstance<'_, '_>,
    path: &mut Vec<Path>,
    structural_type: StructuralTypeId,
    template: Option<&BuildTimeValue>,
) -> Result<BuildTimeValue, String> {
    let declaration = instance
        .program()
        .structural_type(structural_type)
        .ok_or_else(|| "Build result lost its structural declaration".to_owned())?;
    match &declaration.shape {
        StructuralTypeShape::Record { fields, .. } => {
            let (type_name, templates) = match template {
                Some(BuildTimeValue::Struct { type_name, fields }) => {
                    (type_name.as_str(), fields.as_slice())
                }
                _ => (label(instance, declaration.identity)?, &[][..]),
            };
            Ok(BuildTimeValue::Struct {
                type_name: type_name.to_owned(),
                fields: read_fields(instance, path, fields, templates)?,
            })
        }
        StructuralTypeShape::Sum { cases } => {
            let observed = instance
                .observe_borrowed_entry(0, path)
                .map_err(|error| format!("Build case readback: {error:?}"))?;
            let TerminalEntryStorageValue::Storage(TerminalStorageContents::Case(selected)) = observed
            else {
                return Err("Build sum readback did not retain its selected case".to_owned());
            };
            let case = cases
                .iter()
                .find(|case| case.id == selected)
                .ok_or_else(|| "Build result case is outside its nominal declaration".to_owned())?;
            path.push(Path::Case(selected));
            let payload = read_fields(instance, path, &case.fields, &[]);
            path.pop();
            Ok(BuildTimeValue::Case {
                variant: label(instance, case.identity)?.to_owned(),
                payload: payload?,
            })
        }
        StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::ByteSequence(_) => {
            read_leaf(instance, path)
        }
        _ => Err("Build setting has no plain result representation; references and host facets cannot be published as settings".to_owned()),
    }
}

fn read_fields(
    instance: &TerminalInstance<'_, '_>,
    path: &mut Vec<Path>,
    fields: &[StructuralFieldDeclaration],
    templates: &[(String, BuildTimeValue)],
) -> Result<Vec<(String, BuildTimeValue)>, String> {
    let mut values = Vec::with_capacity(fields.len());
    for field in fields {
        let name = label(instance, field.identity)?;
        let template = templates
            .iter()
            .find(|(candidate, _)| candidate == name)
            .map(|(_, value)| value);
        path.push(Path::Field(field.id));
        let value = read_field(instance, path, field, template);
        path.pop();
        values.push((name.to_owned(), value?));
    }
    Ok(values)
}

fn read_leaf(instance: &TerminalInstance<'_, '_>, path: &[Path]) -> Result<BuildTimeValue, String> {
    let observed = instance
        .observe_borrowed_entry(0, path)
        .map_err(|error| format!("Build field readback: {error:?}"))?;
    match observed {
        TerminalEntryStorageValue::Bytes(bytes) => {
            let mut copied = vec![0; bytes.len()];
            bytes
                .copy_to(&mut copied)
                .map_err(|error| format!("Build byte readback: {error:?}"))?;
            Ok(BuildTimeValue::Text(copied))
        }
        TerminalEntryStorageValue::Storage(TerminalStorageContents::Scalar {
            value: TerminalScalarValue::Boolean(value),
            ..
        }) => Ok(BuildTimeValue::Bool(value)),
        TerminalEntryStorageValue::Storage(TerminalStorageContents::Scalar {
            value: TerminalScalarValue::Integer { value, .. },
            ..
        }) => {
            let integer = match value {
                IntegerValue::Signed(value) => i64::try_from(value),
                IntegerValue::Unsigned(value) => i64::try_from(value),
            }
            .map_err(|_| "Build setting integer exceeds the result representation".to_owned())?;
            Ok(BuildTimeValue::Int(integer))
        }
        _ => Err("Build setting has no supported scalar result representation".to_owned()),
    }
}
