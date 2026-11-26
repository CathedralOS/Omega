//! Reconstruct the installed Filesystem::read_all result from its checked sum declarations.
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

pub(super) struct ReadResult {
    result: StructuralTypeId,
    ok: StructuralCaseId,
    count: StructuralFieldId,
    error: StructuralCaseId,
    kind: StructuralFieldId,
    errors: Vec<(ErrorKind, StructuralCaseId)>,
}

impl ReadResult {
    pub(super) fn admit(module: &TerminalModule, result: StructuralTypeId) -> Result<Self, String> {
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
        let ([count], [kind]) = (ok.fields.as_slice(), error.fields.as_slice()) else {
            return Err("filesystem result must carry its count or error kind".into());
        };
        let StructuralFieldType::Structural(kind_type) = kind.field_type else {
            return Err("filesystem error lost its ErrorKind".into());
        };
        let errors = [
            (ErrorKind::NotFound, "NotFound"),
            (ErrorKind::InvalidInput, "InvalidInput"),
            (ErrorKind::IsADirectory, "IsADirectory"),
            (ErrorKind::PermissionDenied, "PermissionDenied"),
        ]
        .into_iter()
        .map(|(kind, name)| Ok((kind, case(kind_type, name)?.id)))
        .collect::<Result<_, String>>()?;
        Ok(Self {
            result,
            ok: ok.id,
            count: count.id,
            error: error.id,
            kind: kind.id,
            errors,
        })
    }

    pub(super) fn finish(
        &self,
        result: Result<u64, ErrorKind>,
    ) -> Result<TerminalEffectResult, String> {
        let value = match result {
            Ok(count) => TerminalAggregateValue::from_scalars(
                self.result,
                Some(self.ok),
                vec![(
                    vec![Path::Field(self.count)],
                    TerminalScalarValue::Integer {
                        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64)
                            .map_err(|e| format!("{e:?}"))?,
                        value: IntegerValue::Unsigned(count.into()),
                    },
                )],
            ),
            Err(error) => {
                let (_, kind) = self
                    .errors
                    .iter()
                    .find(|(kind, _)| *kind == error)
                    .ok_or_else(|| format!("unknown snapshot read error {error:?}"))?;
                TerminalAggregateValue {
                    structural_type: self.result,
                    result_case: Some(self.error),
                    fields: vec![TerminalBoundaryStorageValue {
                        path: vec![Path::Field(self.kind)],
                        contents: TerminalStorageContents::Case(*kind),
                    }],
                }
            }
        };
        Ok(TerminalEffectResult::Aggregate(value))
    }
}
