use crate::manifest::declaration::BuildDeclaration;
use omega_package_source::{AliasName, PackageName};

/// One source request projected without evaluating `build.omg`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencySourceRequest {
    Path {
        explicit_alias: Option<AliasName>,
        location: String,
    },
    Git {
        explicit_alias: Option<AliasName>,
        repository: String,
        revision: String,
    },
}

impl DependencySourceRequest {
    pub fn explicit_alias(&self) -> Option<&AliasName> {
        match self {
            Self::Path { explicit_alias, .. } | Self::Git { explicit_alias, .. } => {
                explicit_alias.as_ref()
            }
        }
    }

    /// Resolve the requester-local import name after source custody has read
    /// the dependency's own package declaration.
    ///
    /// The package-authored name supplies the ordinary alias. An explicit
    /// `depend_as` alias is only a local name-resolution override and never
    /// participates in package or source identity.
    pub fn resolved_alias(&self, package_name: &PackageName) -> AliasName {
        self.explicit_alias()
            .cloned()
            .unwrap_or_else(|| package_name.default_alias())
    }
}

/// One authoritative project role and its direct dependency requests,
/// projected from the same parsed `build.omg` tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildDependencyProjection {
    declaration: BuildDeclaration,
    dependencies: Vec<DependencySourceRequest>,
}

impl BuildDependencyProjection {
    pub(super) fn new(
        declaration: BuildDeclaration,
        dependencies: Vec<DependencySourceRequest>,
    ) -> Self {
        Self {
            declaration,
            dependencies,
        }
    }

    pub const fn declaration(&self) -> &BuildDeclaration {
        &self.declaration
    }

    pub fn dependencies(&self) -> &[DependencySourceRequest] {
        &self.dependencies
    }

    pub fn into_parts(self) -> (BuildDeclaration, Vec<DependencySourceRequest>) {
        (self.declaration, self.dependencies)
    }
}
