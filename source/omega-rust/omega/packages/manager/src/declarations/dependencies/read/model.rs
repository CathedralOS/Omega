use crate::declarations::dependencies::read::active_aliases::{
    ActiveDependencyAliasError, validate_active_alias_uniqueness,
};
use crate::declarations::roles::BuildDeclaration;
use crate::declarations::{AliasName, PackageName};
use omega_target::{TargetProfile, TargetProfileIdentity};

pub const TARGET_DEPENDENCY_CONDITION_SCHEMA_VERSION: u32 = 1;

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

/// Exact dependency column for one compiler-owned deployment profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDependencyColumn {
    profile: TargetProfile,
    occurrence_indices: Vec<usize>,
}

impl TargetDependencyColumn {
    pub(crate) fn new(profile: TargetProfile, occurrence_indices: Vec<usize>) -> Self {
        Self {
            profile,
            occurrence_indices,
        }
    }

    pub const fn profile(&self) -> TargetProfile {
        self.profile
    }

    pub fn occurrence_indices(&self) -> &[usize] {
        &self.occurrence_indices
    }
}

/// Versioned identity of the exact target cases consulted by projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDependencyConditionSchema {
    version: u32,
    referenced_profile_identities: Vec<TargetProfileIdentity>,
}

impl TargetDependencyConditionSchema {
    pub const fn version(&self) -> u32 {
        self.version
    }

    pub fn referenced_profile_identities(&self) -> &[TargetProfileIdentity] {
        &self.referenced_profile_identities
    }
}

/// Complete target-independent result of projecting one build state graph.
///
/// `occurrences` owns each authored request exactly once. Common/profile
/// membership is retained only as occurrence indices, so editing and
/// resolution views cannot drift between parallel request copies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedDependencies {
    occurrences: Vec<DependencySourceRequest>,
    common_occurrence_indices: Vec<usize>,
    by_profile: Vec<TargetDependencyColumn>,
    condition_schema: TargetDependencyConditionSchema,
}

impl ProjectedDependencies {
    pub(super) fn new(
        occurrences: Vec<DependencySourceRequest>,
        common_occurrence_indices: Vec<usize>,
        by_profile: Vec<TargetDependencyColumn>,
        referenced_profile_identities: Vec<TargetProfileIdentity>,
    ) -> Self {
        Self::from_retained_parts(
            occurrences,
            common_occurrence_indices,
            by_profile,
            TARGET_DEPENDENCY_CONDITION_SCHEMA_VERSION,
            referenced_profile_identities,
        )
    }

    pub(crate) fn from_retained_parts(
        occurrences: Vec<DependencySourceRequest>,
        common_occurrence_indices: Vec<usize>,
        by_profile: Vec<TargetDependencyColumn>,
        condition_schema_version: u32,
        referenced_profile_identities: Vec<TargetProfileIdentity>,
    ) -> Self {
        Self {
            occurrences,
            common_occurrence_indices,
            by_profile,
            condition_schema: TargetDependencyConditionSchema {
                version: condition_schema_version,
                referenced_profile_identities,
            },
        }
    }

    pub fn common_occurrence_indices(&self) -> &[usize] {
        &self.common_occurrence_indices
    }

    pub fn common(&self) -> impl Iterator<Item = &DependencySourceRequest> {
        self.common_occurrence_indices
            .iter()
            .map(|index| &self.occurrences[*index])
    }

    pub fn by_profile(&self) -> &[TargetDependencyColumn] {
        &self.by_profile
    }

    pub const fn condition_schema(&self) -> &TargetDependencyConditionSchema {
        &self.condition_schema
    }

    pub fn authored_dependencies(&self) -> &[DependencySourceRequest] {
        &self.occurrences
    }

    pub fn for_profile(
        &self,
        profile: TargetProfile,
    ) -> impl Iterator<Item = &DependencySourceRequest> {
        let profile_indices = self
            .by_profile
            .iter()
            .find(|column| column.profile == profile)
            .map(|column| column.occurrence_indices.as_slice())
            .unwrap_or_default();
        self.common_occurrence_indices
            .iter()
            .chain(profile_indices)
            .map(|index| &self.occurrences[*index])
    }

    /// Authored occurrence positions active for one exact target profile.
    ///
    /// The positions, rather than copied requests, preserve the complete
    /// target-independent projection for later identity work.
    pub fn occurrence_indices_for_profile(
        &self,
        profile: TargetProfile,
    ) -> impl Iterator<Item = usize> + '_ {
        let profile_indices = self
            .by_profile
            .iter()
            .find(|column| column.profile == profile)
            .map(|column| column.occurrence_indices.as_slice())
            .unwrap_or_default();
        self.common_occurrence_indices
            .iter()
            .chain(profile_indices)
            .copied()
    }

    pub fn has_target_conditions(&self) -> bool {
        !self.by_profile.is_empty()
    }

    /// Validate requester-local aliases after the selected package names for
    /// one exact active request set are known.
    ///
    /// `selected_package_names` follows [`Self::for_profile`] order. Inactive
    /// columns need not be acquired merely to discover their package names.
    pub fn validate_active_aliases(
        &self,
        profile: TargetProfile,
        selected_package_names: &[PackageName],
    ) -> Result<(), ActiveDependencyAliasError> {
        validate_active_alias_uniqueness(self, profile, selected_package_names)
    }
}

impl From<Vec<DependencySourceRequest>> for ProjectedDependencies {
    fn from(occurrences: Vec<DependencySourceRequest>) -> Self {
        let common_occurrence_indices = (0..occurrences.len()).collect();
        Self::new(
            occurrences,
            common_occurrence_indices,
            Vec::new(),
            Vec::new(),
        )
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
