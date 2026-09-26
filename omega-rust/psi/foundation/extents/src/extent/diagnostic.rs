//! The diagnostic every extent operation reports, and the range and identity
//! checks shared by the other modules.

pub(crate) fn validate_range(base: u64, length: u64) -> Result<(), ExtentDiagnostic> {
    if length == 0 {
        return Err(ExtentDiagnostic(
            "root extent must carry nonempty authority".into(),
        ));
    }
    base.checked_add(length)
        .ok_or_else(|| ExtentDiagnostic("extent range overflows address width".into()))?;
    Ok(())
}

pub(crate) fn nonzero_identity(identity: u64, name: &str) -> Result<(), ExtentDiagnostic> {
    if identity == 0 {
        return Err(ExtentDiagnostic(format!(
            "normalized {name} identity cannot be zero"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtentDiagnostic(pub String);

impl std::fmt::Display for ExtentDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ExtentDiagnostic {}
