/*
Executed dependency ledgers and conservative source-edit parsing for `build.omg`.

Source syntax locates conservative edit windows; it supplies no
package-resolution authority. `DependencyProjections` splits direct dependency
requests by their authorized context (`Product` vs. `Build`) so each scope
retains separate requester-local ordering and alias validation without falling
back to the other. `PackageSelection` records request custody inside an
acquired repository source (`Root` when omitted), and `resolved_alias` derives
the requester-local import name from either an explicit `dependency::with_alias` override
or the dependency's own declared package name.
*/

mod aliases;
mod error;
mod extraction;
mod policy;
mod projection;
mod source_literal;

pub(crate) use aliases::DependencyAliasError;
pub(crate) use error::DependencyProjectionError;

use crate::declarations::{AliasName, PackageName};
use aliases::validate_alias_uniqueness;
use build_declarations::BuildRequest;
pub use build_declarations::DependencyPurpose;
pub(crate) use extraction::extract_scoped_from_source;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PackageSelection {
    Root,
    Named(PackageName),
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectedDependencies {
    authored_dependencies: Vec<DependencySourceRequest>,
}

impl ProjectedDependencies {
    pub(crate) fn authored_dependencies(&self) -> &[DependencySourceRequest] {
        &self.authored_dependencies
    }

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DependencyProjections {
    product: ProjectedDependencies,
    build: ProjectedDependencies,
}

impl DependencyProjections {
    pub(crate) fn from_executed_requests<'request>(
        requests: impl IntoIterator<Item = &'request BuildRequest>,
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
    pub(crate) fn build_request(&self, purpose: DependencyPurpose) -> Result<BuildRequest, String> {
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

    pub fn resolved_alias(&self, package_name: &PackageName) -> AliasName {
        self.explicit_alias()
            .cloned()
            .unwrap_or_else(|| package_name.default_alias())
    }
}
