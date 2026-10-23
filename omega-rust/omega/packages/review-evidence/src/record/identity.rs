use semantic_vocabulary::PackageKeyIdentity;

/// One shared length-prefixed semantic identity framing, independent of source
/// capture and persistence envelopes. The caller chooses the storage strategy.
pub(crate) fn write_framed_identity<'a>(
    writer: &mut impl std::fmt::Write,
    label: &str,
    children: impl IntoIterator<Item = &'a str>,
) -> std::fmt::Result {
    write!(writer, "{}:{label}", label.len())?;
    for child in children {
        write!(writer, "{}:{child}", child.len())?;
    }
    Ok(())
}

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

    /// Review evidence accepts a nominal identity only when it names a path
    /// and an exact owner: absence is the `Unresolved` owner, never merely an
    /// empty path. Every record family consumes this one predicate instead of
    /// restating it.
    pub(crate) fn validate_resolved(&self) -> Result<(), &'static str> {
        if self.path.is_empty() || self.owner == PackageReviewNominalOwner::Unresolved {
            Err("nominal identity is empty or unresolved")
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewSemanticDependencyExposure {
    PrivateImplementation,
    PublicInterface,
}

pub type PackageReviewSemanticDependencyKind = checked_trees::CheckedSemanticDependencyKind;
