//! Authority a receiving host grants to one service requirement.
use semantic_vocabulary::{TerminalAuthorityClass, provider_identity::ServiceSchemaDigest};
use std::collections::BTreeSet;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PortableFilesystemAuthorityFacet {
    ContentRead,
    ContentWrite,
    MetadataQuery,
    DirectoryEnumeration,
    NamespaceMutation,
    MetadataMutation,
}

impl PortableFilesystemAuthorityFacet {
    pub const ALL: [Self; 6] = [
        Self::ContentRead,
        Self::ContentWrite,
        Self::MetadataQuery,
        Self::DirectoryEnumeration,
        Self::NamespaceMutation,
        Self::MetadataMutation,
    ];

    pub const fn terminal_authority_class(self) -> TerminalAuthorityClass {
        match self {
            Self::ContentRead => TerminalAuthorityClass::FilesystemContentRead,
            Self::ContentWrite => TerminalAuthorityClass::FilesystemContentWrite,
            Self::MetadataQuery => TerminalAuthorityClass::FilesystemMetadataQuery,
            Self::DirectoryEnumeration => TerminalAuthorityClass::DirectoryEnumeration,
            Self::NamespaceMutation => TerminalAuthorityClass::FilesystemNamespaceMutation,
            Self::MetadataMutation => TerminalAuthorityClass::FilesystemMetadataMutation,
        }
    }
}

impl From<PortableFilesystemAuthorityFacet> for TerminalAuthorityClass {
    fn from(facet: PortableFilesystemAuthorityFacet) -> Self {
        facet.terminal_authority_class()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct TerminalAuthorityDisposition {
    classes: Vec<TerminalAuthorityClass>,
}

impl TerminalAuthorityDisposition {
    pub fn try_from_canonical_classes(
        classes: Vec<TerminalAuthorityClass>,
    ) -> Result<Self, Vec<TerminalAuthorityClass>> {
        if classes.windows(2).any(|pair| pair[0] >= pair[1]) {
            Err(classes)
        } else {
            Ok(Self { classes })
        }
    }

    pub fn from_classes(classes: impl IntoIterator<Item = TerminalAuthorityClass>) -> Self {
        let classes = classes
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Self { classes }
    }

    pub fn from_filesystem_facets(
        facets: impl IntoIterator<Item = PortableFilesystemAuthorityFacet>,
    ) -> Self {
        Self::from_classes(facets.into_iter().map(TerminalAuthorityClass::from))
    }

    pub fn classes(&self) -> &[TerminalAuthorityClass] {
        &self.classes
    }

    pub fn contains_all(&self, other: &Self) -> bool {
        self.first_missing_class(other).is_none()
    }

    pub fn first_missing_class(&self, other: &Self) -> Option<TerminalAuthorityClass> {
        other
            .classes
            .iter()
            .find(|class| self.classes.binary_search(class).is_err())
            .copied()
    }
}
