#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryDiagnostic(pub String);

diagnostics::string_diagnostic!(ProgramStorageEntryDiagnostic);
