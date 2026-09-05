use package_source::SourceLineage;
use semantic_vocabulary::PackageKeyIdentity;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageName(String);

impl PackageName {
    pub fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        build_declarations::ProjectName::parse(value.clone())?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn default_alias(&self) -> AliasName {
        AliasName(self.0.replace('-', "_"))
    }
}

impl From<build_declarations::ProjectName> for PackageName {
    fn from(value: build_declarations::ProjectName) -> Self {
        Self(value.into_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AliasName(String);

impl AliasName {
    pub(crate) fn is_valid(value: &str) -> bool {
        is_snake_case(value)
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if Self::is_valid(&value) {
            Ok(Self(value))
        } else {
            Err(format!(
                "dependency alias `{value}` must use snake_case Omega identifier spelling"
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable nominal identity: authored name plus canonical source lineage.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageKey {
    name: PackageName,
    source_lineage: SourceLineage,
}

impl PackageKey {
    pub fn new(name: PackageName, source_lineage: SourceLineage) -> Self {
        Self {
            name,
            source_lineage,
        }
    }

    pub fn name(&self) -> &PackageName {
        &self.name
    }

    pub fn source_lineage(&self) -> &SourceLineage {
        &self.source_lineage
    }

    pub fn identity(&self) -> PackageKeyIdentity {
        let mut hasher = Sha256::new();
        hash_field(&mut hasher, b"omega-package-key-identity-v1");
        hash_field(&mut hasher, self.name.as_str().as_bytes());
        self.source_lineage.hash_canonical(&mut hasher);
        PackageKeyIdentity::from_digest(hasher.finalize().into())
            .expect("domain-separated SHA-256 package identity must be nonzero")
    }
}

fn is_snake_case(value: &str) -> bool {
    value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && !value.ends_with('_')
        && !value.contains("__")
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}
