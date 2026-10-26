//! Ordered dependency request records and legacy literal projection adapters.
//! Executed Build ledgers supply the records directly; static extraction remains
//! only until production package preparation migrates to that executed route.

mod aliases;
mod error;
mod extraction;
mod policy;
mod projection;
mod source_literal;

pub(crate) use aliases::DependencyAliasError;
pub(crate) use error::DependencyProjectionError;
pub(crate) use extraction::extract_build_dependency_projection;

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

/// Package selection inside one acquired repository source.
///
/// Selection is request custody, not source or package identity. Omitting the
/// source field normalizes to the zero case, `Root`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSelection {
    Root,
    Named(PackageName),
}

/// One source request, retaining its exact locator and explicit alias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DependencySourceRequest {
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

/// The ordered direct dependency requests within one purpose scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectedDependencies {
    authored_dependencies: Vec<DependencySourceRequest>,
}

impl ProjectedDependencies {
    pub(crate) fn authored_dependencies(&self) -> &[DependencySourceRequest] {
        &self.authored_dependencies
    }

    /// Validate requester-local aliases after every selected package name is
    /// known. Names follow [`Self::authored_dependencies`] order.
    pub(crate) fn validate_aliases(
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
pub(crate) struct DependencyProjections {
    product: ProjectedDependencies,
    build: ProjectedDependencies,
}

impl DependencyProjections {
    /// Retain exactly the issued dependency ledger, preserving ordinal order
    /// within each purpose. No source syntax or unselected branch contributes.
    pub(crate) fn from_executed_requests<'request>(
        requests: impl IntoIterator<Item = &'request build_declarations::BuildRequest>,
    ) -> Result<Self, String> {
        use build_declarations::{BuildDependencySource, BuildRequest};
        let mut product = Vec::new();
        let mut build = Vec::new();
        for request in requests {
            let BuildRequest::Dependency {
                purpose,
                alias,
                source,
            } = request
            else {
                continue;
            };
            let explicit_alias = alias
                .as_ref()
                .map(|name| AliasName::parse(name.as_str()))
                .transpose()?;
            let request = match source {
                BuildDependencySource::Path(location) => DependencySourceRequest::Path {
                    explicit_alias,
                    location: location.clone(),
                },
                BuildDependencySource::Git {
                    repository,
                    revision,
                    package,
                } => DependencySourceRequest::Git {
                    explicit_alias,
                    repository: repository.clone(),
                    revision: revision.clone(),
                    selection: match package {
                        Some(name) => PackageSelection::Named(name.clone().into()),
                        None => PackageSelection::Root,
                    },
                },
            };
            match purpose {
                DependencyPurpose::Product => product.push(request),
                DependencyPurpose::Build => build.push(request),
            }
        }
        Ok(Self::new(product.into(), build.into()))
    }

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

    pub fn requests(&self, purpose: DependencyPurpose) -> &[DependencySourceRequest] {
        self.projections(purpose).authored_dependencies()
    }

    pub(crate) fn authored_request_count(&self) -> usize {
        self.product.authored_dependencies().len() + self.build.authored_dependencies().len()
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
    /// Retain acquisition's exact request in the shared executed vocabulary.
    pub(crate) fn build_request(
        &self,
        purpose: DependencyPurpose,
    ) -> Result<build_declarations::BuildRequest, String> {
        use build_declarations::{BuildDependencySource, BuildRequest, ProjectName};
        let alias = self
            .explicit_alias()
            .map(|name| ProjectName::parse(name.as_str()))
            .transpose()?;
        let source = match self {
            Self::Path { location, .. } => BuildDependencySource::Path(location.clone()),
            Self::Git {
                repository,
                revision,
                selection,
                ..
            } => BuildDependencySource::Git {
                repository: repository.clone(),
                revision: revision.clone(),
                package: match selection {
                    PackageSelection::Root => None,
                    PackageSelection::Named(name) => Some(ProjectName::parse(name.as_str())?),
                },
            },
        };
        Ok(BuildRequest::Dependency {
            purpose,
            alias,
            source,
        })
    }

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
pub(crate) struct BuildDependencyProjection {
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
