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

pub fn write_framed_identity<'a>(
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
    pub digest: [u8; 32],
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
    pub owner: PackageReviewNominalOwner,
    pub path: String,
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

/// One boundary effect. Its original declaration establishes ownership, while
/// an applied declaration also retains the compiler's normalized static inputs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewServiceIdentity {
    pub declaration: PackageReviewNominalIdentity,
    pub application: Option<super::PackageReviewTypeIdentity>,
}

impl From<PackageReviewNominalIdentity> for PackageReviewServiceIdentity {
    fn from(declaration: PackageReviewNominalIdentity) -> Self {
        Self {
            declaration,
            application: None,
        }
    }
}

impl PackageReviewServiceIdentity {
    pub fn owner(&self) -> PackageReviewNominalOwner {
        self.declaration.owner()
    }
    pub fn path(&self) -> &str {
        self.declaration.path()
    }
    pub(crate) fn validate_resolved(&self) -> Result<(), &'static str> {
        self.declaration.validate_resolved()?;
        if self.application.as_ref().is_some_and(|application| {
            !self
                .declaration
                .matches_applied_trait_declaration(&application.canonical)
        }) {
            return Err(
                "applied service identity does not name its declared receiver and complete arguments",
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewSemanticDependencyExposure {
    PrivateImplementation,
    PublicInterface,
}

pub type PackageReviewSemanticDependencyKind = semantic_vocabulary::SemanticDependencyKind;
