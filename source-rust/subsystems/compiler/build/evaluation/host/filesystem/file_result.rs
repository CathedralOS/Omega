//! Reconstruct the installed Filesystem operation result from its checked sum declarations.
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Path, IntegerSign, IntegerType, IntegerValue,
    StructuralCaseId, StructuralFieldId, StructuralTypeId,
};
use std::io::ErrorKind;
use terminal_interpreter::{
    TerminalAggregateValue, TerminalBoundaryStorageValue, TerminalEffectResult,
    TerminalScalarValue, TerminalStorageContents,
};
use terminal_psi::{
    StructuralCaseDeclaration, StructuralFieldType, StructuralTypeShape, TerminalModule,
};

pub(super) struct FileResult {
    result: StructuralTypeId,
    ok: StructuralCaseId,
    count: Option<StructuralFieldId>,
    error: StructuralCaseId,
    kind: StructuralFieldId,
    errors: Vec<(ErrorKind, StructuralCaseId)>,
    other_error: StructuralCaseId,
}

impl FileResult {
    pub(super) fn admit(
        module: &TerminalModule,
        result: StructuralTypeId,
        counted: bool,
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
        let count = match (ok.fields.as_slice(), counted) {
            ([count], true) => Some(count.id),
            ([], false) => None,
            _ => return Err("filesystem result has the wrong success payload".into()),
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
            count,
            error: error.id,
            kind: kind.id,
            errors,
            other_error: case(kind_type, "Other")?.id,
        })
    }

    pub(super) fn finish(
        &self,
        result: Result<Option<u64>, ErrorKind>,
    ) -> Result<TerminalEffectResult, String> {
        let value = match (result, self.count) {
            (Ok(None), None) => {
                TerminalAggregateValue::from_scalars(self.result, Some(self.ok), Vec::new())
            }
            (Ok(Some(count)), Some(field)) => TerminalAggregateValue::from_scalars(
                self.result,
                Some(self.ok),
                vec![(
                    vec![Path::Field(field)],
                    TerminalScalarValue::Integer {
                        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64)
                            .map_err(|e| format!("{e:?}"))?,
                        value: IntegerValue::Unsigned(count.into()),
                    },
                )],
            ),
            (Ok(_), _) => {
                return Err("filesystem success payload disagrees with its declaration".into());
            }
            (Err(error), _) => {
                let kind = self
                    .errors
                    .iter()
                    .find(|(kind, _)| *kind == error)
                    .map_or(self.other_error, |(_, case)| *case);
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
