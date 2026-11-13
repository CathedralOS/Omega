// Nominal declaration identity, toolchain source digests, and semantic
// dependency exposure vocabulary.
//
// `write_framed_identity` formats length-prefixed semantic identities
// (`len:label`) independent of source capture or persistence envelopes.
// `PackageReviewNominalOwner::Unresolved` surfaces any nominal reference that
// checked lowering retained without an authored source owner or mandatory
// compiler derivation origin so admission rejects it; `validate_resolved`
// enforces non-empty path + resolved owner across all record families.

use crate::record::accessors::field_projection_accessors;
use semantic_vocabulary::PackageKeyIdentity;

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
    field_projection_accessors!(valued digest: [u8; 32]);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewNominalOwner {
    Package(PackageKeyIdentity),
    ToolchainSource(PackageReviewToolchainSourceIdentity),
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewNominalIdentity {
    pub(crate) owner: PackageReviewNominalOwner,
    pub(crate) path: String,
}

impl PackageReviewNominalIdentity {
    field_projection_accessors!(copied owner: PackageReviewNominalOwner);

    pub fn path(&self) -> &str {
        &self.path
    }

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

pub type PackageReviewSemanticDependencyKind = semantic_vocabulary::SemanticDependencyKind;
