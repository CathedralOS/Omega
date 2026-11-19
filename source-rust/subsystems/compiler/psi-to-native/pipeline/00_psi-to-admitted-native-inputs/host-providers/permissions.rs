//! Authority a receiving host grants to one service requirement.
use semantic_vocabulary::{
    PortableFilesystemAuthorityFacet, TerminalAuthorityDisposition,
    provider_identity::ServiceSchemaDigest,
};

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct ServiceTerminalAuthorityPermission {
    service_schema: ServiceSchemaDigest,
    requirement_identity: terminal_psi::TerminalIdentityIndex,
    permitted: TerminalAuthorityDisposition,
}

impl ServiceTerminalAuthorityPermission {
    pub fn new(
        service_schema: ServiceSchemaDigest,
        requirement_identity: terminal_psi::TerminalIdentityIndex,
        permitted: TerminalAuthorityDisposition,
    ) -> Self {
        Self {
            service_schema,
            requirement_identity,
            permitted,
        }
    }

    pub fn for_filesystem_facets(
        service_schema: ServiceSchemaDigest,
        requirement_identity: terminal_psi::TerminalIdentityIndex,
        facets: impl IntoIterator<Item = PortableFilesystemAuthorityFacet>,
    ) -> Self {
        Self::new(
            service_schema,
            requirement_identity,
            TerminalAuthorityDisposition::from_filesystem_facets(facets),
        )
    }

    pub const fn service_schema(&self) -> ServiceSchemaDigest {
        self.service_schema
    }

    pub const fn requirement_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        self.requirement_identity
    }

    #[doc(hidden)]
    pub const fn rekey_requirement_identity(
        &mut self,
        requirement_identity: terminal_psi::TerminalIdentityIndex,
    ) {
        self.requirement_identity = requirement_identity;
    }

    pub const fn permitted(&self) -> &TerminalAuthorityDisposition {
        &self.permitted
    }
}
