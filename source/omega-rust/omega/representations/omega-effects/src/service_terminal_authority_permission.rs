use crate::{TerminalAuthorityDisposition, provider_plan::ServiceSchemaDigest};

/// One consumer-supplied permission for an exact requirement in one complete
/// normalized service schema.
///
/// This row is inert policy input. It does not discover a service, select a
/// provider, or infer authority from readable names. A later compiler seam
/// must rejoin the retained schema and requirement identities to checked
/// declarations before the row can participate in terminal-authority review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceTerminalAuthorityPermission {
    service_schema: ServiceSchemaDigest,
    requirement_identity: String,
    permitted: TerminalAuthorityDisposition,
}

impl ServiceTerminalAuthorityPermission {
    pub fn new(
        service_schema: ServiceSchemaDigest,
        requirement_identity: impl Into<String>,
        permitted: TerminalAuthorityDisposition,
    ) -> Self {
        Self {
            service_schema,
            requirement_identity: requirement_identity.into(),
            permitted,
        }
    }

    pub const fn service_schema(&self) -> ServiceSchemaDigest {
        self.service_schema
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn permitted(&self) -> &TerminalAuthorityDisposition {
        &self.permitted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TerminalAuthorityClass;

    fn schema(marker: u8) -> ServiceSchemaDigest {
        ServiceSchemaDigest::from_digest([marker; 32])
    }

    #[test]
    fn permission_row_retains_exact_key_and_canonical_disposition() {
        let row = ServiceTerminalAuthorityPermission::new(
            schema(7),
            "Console::exit_process#exact",
            TerminalAuthorityDisposition::from_classes([
                TerminalAuthorityClass::ProcessTermination,
                TerminalAuthorityClass::ProcessOutput,
                TerminalAuthorityClass::ProcessTermination,
            ]),
        );

        assert_eq!(row.service_schema(), schema(7));
        assert_eq!(row.requirement_identity(), "Console::exit_process#exact");
        assert_eq!(
            row.permitted().classes(),
            &[
                TerminalAuthorityClass::ProcessOutput,
                TerminalAuthorityClass::ProcessTermination,
            ]
        );
    }
}
