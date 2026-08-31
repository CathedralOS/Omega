use crate::declarations::dependencies::read::aliases::{
    DependencyAliasError, validate_alias_uniqueness,
};
use crate::declarations::roles::BuildDeclaration;
use crate::declarations::{AliasName, PackageName};

/// Package selection inside one acquired repository source.
///
/// Selection is request custody, not source or package identity. Omitting the
/// source field normalizes to the zero case, `Root`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSelection {
    Root,
    Named(PackageName),
}

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
        selection: PackageSelection,
    },
}

/// The one ordered set of unconditional direct dependency rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedDependencies {
    authored_dependencies: Vec<DependencySourceRequest>,
}

impl ProjectedDependencies {
    pub fn authored_dependencies(&self) -> &[DependencySourceRequest] {
        &self.authored_dependencies
    }

    pub fn into_authored_dependencies(self) -> Vec<DependencySourceRequest> {
        self.authored_dependencies
    }

    /// Validate requester-local aliases after every selected package name is
    /// known. Names follow [`Self::authored_dependencies`] order.
    pub fn validate_aliases(
        &self,
        selected_package_names: &[PackageName],
    ) -> Result<(), DependencyAliasError> {
        validate_alias_uniqueness(self, selected_package_names)
    }
}

impl From<Vec<DependencySourceRequest>> for ProjectedDependencies {
    fn from(authored_dependencies: Vec<DependencySourceRequest>) -> Self {
        Self {
            authored_dependencies,
        }
    }
}

impl DependencySourceRequest {
    pub fn explicit_alias(&self) -> Option<&AliasName> {
        match self {
            Self::Path { explicit_alias, .. } | Self::Git { explicit_alias, .. } => {
                explicit_alias.as_ref()
            }
        }
    }

    pub const fn package_selection(&self) -> Option<&PackageSelection> {
        match self {
            Self::Path { .. } => None,
            Self::Git { selection, .. } => Some(selection),
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
    dependencies: ProjectedDependencies,
}

impl BuildDependencyProjection {
    pub(super) fn new(declaration: BuildDeclaration, dependencies: ProjectedDependencies) -> Self {
        Self {
            declaration,
            dependencies,
        }
    }

    pub const fn declaration(&self) -> &BuildDeclaration {
        &self.declaration
    }

    pub fn dependencies(&self) -> &[DependencySourceRequest] {
        self.dependencies.authored_dependencies()
    }

    pub const fn projected_dependencies(&self) -> &ProjectedDependencies {
        &self.dependencies
    }

    pub fn into_parts(self) -> (BuildDeclaration, ProjectedDependencies) {
        (self.declaration, self.dependencies)
    }
}
