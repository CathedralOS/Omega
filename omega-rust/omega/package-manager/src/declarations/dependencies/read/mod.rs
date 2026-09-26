//! Hermetic projection of literal dependency requests from `build.omg`.

mod aliases;
mod error;
mod extraction;
mod policy;
mod projection;
mod source_literal;

pub use aliases::DependencyAliasError;
pub use error::DependencyProjectionError;
pub use extraction::{extract_build_dependency_projection, extract_dependency_projection};

use crate::declarations::roles::BuildDeclaration;
use crate::declarations::{AliasName, PackageName};
use aliases::validate_alias_uniqueness;
/// Which authorized context one direct dependency edge serves.
///
/// `depend`/`depend_as` rows authorize product imports. `build_depend`/
/// `build_depend_as` rows authorize the host build context. The two scopes
/// are distinct: an alias or a package may appear in both, and neither scope
/// falls back to the other. The vocabulary lives in `build_declarations`
/// so package management and the compiler share one grammar.
pub use build_declarations::DependencyPurpose;
pub(crate) use extraction::extract_scoped_from_source;
pub(crate) use projection::validate_static_dependency_source;

#[cfg(test)]
mod tests;

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

/// The one ordered set of unconditional direct dependency rows within a
/// single purpose scope.
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

/// The direct dependency rows split by their authorized context.
///
/// Product rows and build rows retain separate requester-local ordering and
/// alias scopes. One package may appear in both scopes, and each scope's
/// aliases are validated independently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyProjections {
    product: ProjectedDependencies,
    build: ProjectedDependencies,
}

impl DependencyProjections {
    pub fn new(product: ProjectedDependencies, build: ProjectedDependencies) -> Self {
        Self { product, build }
    }

    pub const fn product(&self) -> &ProjectedDependencies {
        &self.product
    }

    pub const fn build(&self) -> &ProjectedDependencies {
        &self.build
    }

    pub const fn projections(&self, purpose: DependencyPurpose) -> &ProjectedDependencies {
        match purpose {
            DependencyPurpose::Product => &self.product,
            DependencyPurpose::Build => &self.build,
        }
    }

    pub fn into_product(self) -> ProjectedDependencies {
        self.product
    }

    pub fn requests(&self, purpose: DependencyPurpose) -> &[DependencySourceRequest] {
        self.projections(purpose).authored_dependencies()
    }

    pub fn authored_request_count(&self) -> usize {
        self.product.authored_dependencies().len() + self.build.authored_dependencies().len()
    }

    pub fn is_empty(&self) -> bool {
        self.authored_request_count() == 0
    }

    /// Validate requester-local aliases for one purpose scope after every
    /// selected package name is known for that scope.
    pub fn validate_aliases(
        &self,
        purpose: DependencyPurpose,
        selected_package_names: &[PackageName],
    ) -> Result<(), DependencyAliasError> {
        self.projections(purpose)
            .validate_aliases(selected_package_names)
    }
}

impl From<Vec<DependencySourceRequest>> for DependencyProjections {
    fn from(product_requests: Vec<DependencySourceRequest>) -> Self {
        Self::new(product_requests.into(), Vec::new().into())
    }
}

impl From<ProjectedDependencies> for DependencyProjections {
    fn from(product: ProjectedDependencies) -> Self {
        Self::new(product, Vec::new().into())
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
    dependencies: DependencyProjections,
}

impl BuildDependencyProjection {
    fn new(declaration: BuildDeclaration, dependencies: DependencyProjections) -> Self {
        Self {
            declaration,
            dependencies,
        }
    }

    pub const fn declaration(&self) -> &BuildDeclaration {
        &self.declaration
    }

    pub fn product_dependencies(&self) -> &[DependencySourceRequest] {
        self.dependencies.requests(DependencyPurpose::Product)
    }

    pub fn build_dependencies(&self) -> &[DependencySourceRequest] {
        self.dependencies.requests(DependencyPurpose::Build)
    }

    pub const fn dependency_projections(&self) -> &DependencyProjections {
        &self.dependencies
    }

    pub fn into_parts(self) -> (BuildDeclaration, DependencyProjections) {
        (self.declaration, self.dependencies)
    }
}
