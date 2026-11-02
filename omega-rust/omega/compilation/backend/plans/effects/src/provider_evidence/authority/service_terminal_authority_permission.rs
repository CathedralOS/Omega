//! What one service is permitted to exercise, against the classes terminal_authority
//! defines.

use crate::{
    PortableFilesystemAuthorityFacet, TerminalAuthorityDisposition,
    provider_plan::ServiceSchemaDigest,
};

/// One consumer-supplied permission for an exact requirement in one complete
/// normalized service schema.
///
/// This row is inert policy input. It does not discover a service, select a
/// provider, or infer authority from readable names. A later compiler seam
/// must rejoin the retained schema and requirement identities to checked
/// declarations before the row can participate in terminal-authority review.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct ServiceTerminalAuthorityPermission {
    service_schema: ServiceSchemaDigest,
    /// Index into the permission policy's identity table.
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

    /// Construct consumer policy from D45's closed portable filesystem facet
    /// vocabulary. The caller must still supply the exact checked schema and
    /// requirement identities; this helper performs no name-based inference.
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

    /// Re-keys this row to the same requirement under a different identity
    /// table. Caller supplies the already-interned index.
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
