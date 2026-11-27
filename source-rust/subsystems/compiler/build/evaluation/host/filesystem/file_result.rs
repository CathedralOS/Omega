//! Reconstruct the installed Filesystem operation result from its checked sum declarations.
use package_compilation::build_execution::BuildFileError;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Path, IntegerSign, IntegerType, IntegerValue, ScalarType,
    StructuralCaseId, StructuralFieldId, StructuralTypeId,
};
use source_compiler::publication::TerminalBuildFilesystemOperation as Operation;
use std::io::ErrorKind;
use terminal_interpreter::{
    TerminalAggregateValue, TerminalBoundaryStorageBuffer, TerminalBoundaryStorageValue,
    TerminalEffectResult, TerminalScalarValue, TerminalStorageContents, TerminalStructuralValue,
};
use terminal_psi::{
    StructuralCaseDeclaration, StructuralFieldType, StructuralTypeShape, TerminalModule,
};

pub(super) struct FileResult {
    result: StructuralTypeId,
    ok: StructuralCaseId,
    success_path: Option<Vec<Path>>,
    operand: Option<(StructuralTypeId, StructuralFieldId)>,
    bad_descriptor: StructuralCaseId,
    error: StructuralCaseId,
    kind: StructuralFieldId,
    errors: Vec<(ErrorKind, StructuralCaseId)>,
    other_error: StructuralCaseId,
}

impl FileResult {
    pub(super) fn admit(
        module: &TerminalModule,
        result: StructuralTypeId,
        operation: Operation,
        boundary: &terminal_psi::BoundaryMachineDeclaration,
    ) -> Result<Self, String> {
        let case = |ty, name: &str| -> Result<&StructuralCaseDeclaration, String> {
            let row = module
                .types
                .structural_types
                .iter()
                .find(|row| row.id == ty)
                .ok_or("missing filesystem result type")?;
            let StructuralTypeShape::Sum { cases } = &row.shape else {
                return Err("filesystem result must be a sum".into());
            };
            cases
                .iter()
                .find(|case| module.identities.text(case.identity) == Some(name))
                .ok_or_else(|| format!("missing filesystem result case {name}"))
        };
        let ok = case(result, "Ok")?;
        let error = case(result, "Error")?;
        let file_handle = |ty| {
            let row = module
                .types
                .structural_types
                .iter()
                .find(|row| row.id == ty)?;
            let StructuralTypeShape::Record { fields, .. } = &row.shape else {
                return None;
            };
            let [field] = fields.as_slice() else {
                return None;
            };
            (module.identities.text(field.identity) == Some("handle")
                && field.field_type == word_field())
            .then_some(field.id)
        };
        let success_path = match (operation, ok.fields.as_slice()) {
            (Operation::ReadFile | Operation::ReadAll, [count])
                if count.field_type == word_field() =>
            {
                Some(vec![Path::Field(count.id)])
            }
            (Operation::OpenFile, [file]) => {
                let StructuralFieldType::Structural(ty) = file.field_type else {
                    return Err("OpenResult::Ok requires a File".into());
                };
                let handle = file_handle(ty).ok_or("File requires its checked u64 handle")?;
                Some(vec![Path::Field(file.id), Path::Field(handle)])
            }
            (Operation::WriteAll | Operation::CreateDirectory | Operation::CloseFile, []) => None,
            _ => return Err("filesystem result has the wrong success payload".into()),
        };
        let operand = if matches!(operation, Operation::ReadFile | Operation::CloseFile) {
            let parameter = boundary
                .structural_parameters
                .iter()
                .find(|p| p.position == 0 && p.access == terminal_psi::StructuralAccess::Owned)
                .ok_or("missing checked File operand")?;
            Some((
                parameter.structural_type,
                file_handle(parameter.structural_type).ok_or("invalid File operand declaration")?,
            ))
        } else {
            None
        };
        let [kind] = error.fields.as_slice() else {
            return Err("filesystem result must carry an error kind".into());
        };
        let StructuralFieldType::Structural(kind_type) = kind.field_type else {
            return Err("filesystem error lost its ErrorKind".into());
        };
        let errors = [
            (ErrorKind::NotFound, "NotFound"),
            (ErrorKind::AlreadyExists, "AlreadyExists"),
            (ErrorKind::InvalidInput, "InvalidInput"),
            (ErrorKind::IsADirectory, "IsADirectory"),
            (ErrorKind::PermissionDenied, "PermissionDenied"),
            (ErrorKind::WouldBlock, "WouldBlock"),
            (ErrorKind::Unsupported, "Unsupported"),
            (ErrorKind::WriteZero, "WriteZero"),
        ]
        .into_iter()
        .map(|(kind, name)| Ok((kind, case(kind_type, name)?.id)))
        .collect::<Result<_, String>>()?;
        Ok(Self {
            result,
            ok: ok.id,
            success_path,
            operand,
            bad_descriptor: case(kind_type, "BadDescriptor")?.id,
            error: error.id,
            kind: kind.id,
            errors,
            other_error: case(kind_type, "Other")?.id,
        })
    }

    pub(super) fn needs_file(&self) -> bool {
        self.operand.is_some()
    }

    pub(super) fn handle(
        &self,
        values: &[TerminalStructuralValue],
        buffers: &[TerminalBoundaryStorageBuffer],
    ) -> Result<u64, String> {
        let (ty, field) = self.operand.ok_or("filesystem call has no File operand")?;
        let [TerminalBoundaryStorageBuffer::Structural(buffer)] = buffers else {
            return Err("missing File storage operand".into());
        };
        if values.first().map(|v| v.structural_type) != Some(ty)
            || buffer.argument_index() != 0
            || buffer.structural_type() != ty
            || buffer.access() != terminal_psi::StructuralAccess::Owned
        {
            return Err("filesystem operand differs from its checked File".into());
        }
        let [stored] = buffer.values() else {
            return Err("invalid File contents".into());
        };
        let TerminalStorageContents::Scalar {
            value:
                TerminalScalarValue::Integer {
                    value: IntegerValue::Unsigned(value),
                    scalar_type,
                },
            stored_scalar,
        } = &stored.contents
        else {
            return Err("File.handle has no integer contents".into());
        };
        if stored.path != [Path::Field(field)]
            || *stored_scalar != ScalarType::Integer(word())
            || *scalar_type != word()
        {
            return Err("invalid File.handle projection".into());
        }
        u64::try_from(*value).map_err(|_| "File.handle exceeds u64".into())
    }

    pub(super) fn finish(
        &self,
        result: Result<Option<u64>, BuildFileError>,
    ) -> Result<TerminalEffectResult, String> {
        let value = match (result, &self.success_path) {
            (Ok(None), None) => {
                TerminalAggregateValue::from_scalars(self.result, Some(self.ok), Vec::new())
            }
            (Ok(Some(count)), Some(path)) => TerminalAggregateValue::from_scalars(
                self.result,
                Some(self.ok),
                vec![(
                    path.clone(),
                    TerminalScalarValue::Integer {
                        scalar_type: word(),
                        value: IntegerValue::Unsigned(count.into()),
                    },
                )],
            ),
            (Ok(_), _) => {
                return Err("filesystem success payload disagrees with its declaration".into());
            }
            (Err(error), _) => {
                let kind = match error {
                    BuildFileError::BadDescriptor => self.bad_descriptor,
                    BuildFileError::Io(error) => self
                        .errors
                        .iter()
                        .find(|(kind, _)| *kind == error)
                        .map_or(self.other_error, |(_, case)| *case),
                };
                TerminalAggregateValue {
                    structural_type: self.result,
                    result_case: Some(self.error),
                    fields: vec![TerminalBoundaryStorageValue {
                        path: vec![Path::Field(self.kind)],
                        contents: TerminalStorageContents::Case(kind),
                    }],
                }
            }
        };
        Ok(TerminalEffectResult::Aggregate(value))
    }
}

fn word() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is a supported carrier")
}

fn word_field() -> StructuralFieldType {
    StructuralFieldType::Scalar(ScalarType::Integer(word()))
}
