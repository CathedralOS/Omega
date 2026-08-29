use psi_core::PackageKeyIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageReviewToolchainSourceIdentity {
    pub(crate) digest: [u8; 32],
}

impl PackageReviewToolchainSourceIdentity {
    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewNominalOwner {
    Package(PackageKeyIdentity),
    /// Exact authored toolchain source coordinate and bytes. This binds the
    /// nominal declaration but is not the whole compiler/toolchain commitment
    /// required by sealed admission.
    ToolchainSource(PackageReviewToolchainSourceIdentity),
    /// Checked lowering retained a nominal reference without an authored
    /// source owner or mandatory compiler derivation origin. Review surfaces
    /// it explicitly; admission must reject it.
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewNominalIdentity {
    pub(crate) owner: PackageReviewNominalOwner,
    pub(crate) path: String,
}

impl PackageReviewNominalIdentity {
    pub const fn owner(&self) -> PackageReviewNominalOwner {
        self.owner
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewSemanticDependencyExposure {
    PrivateImplementation,
    PublicInterface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewSemanticDependencyKind {
    NominalIdentity,
    Layout,
    OwnershipBehavior,
    AutomaticCleanup,
    AutomaticCleanupMachine,
}

/// One exact declaration whose semantics are carried by a reviewed package's
/// machine without granting that machine authored source authority.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewSemanticDependency {
    pub(crate) consumer: PackageReviewNominalIdentity,
    pub(crate) dependency: PackageReviewNominalIdentity,
    pub(crate) exposure: PackageReviewSemanticDependencyExposure,
    pub(crate) kind: PackageReviewSemanticDependencyKind,
}

impl PackageReviewSemanticDependency {
    pub const fn consumer(&self) -> &PackageReviewNominalIdentity {
        &self.consumer
    }

    pub const fn dependency(&self) -> &PackageReviewNominalIdentity {
        &self.dependency
    }

    pub const fn exposure(&self) -> PackageReviewSemanticDependencyExposure {
        self.exposure
    }

    pub const fn kind(&self) -> PackageReviewSemanticDependencyKind {
        self.kind
    }
}
