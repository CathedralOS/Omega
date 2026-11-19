//! Source provenance and its canonical identity encoding, without source custody.
//! These records contain content digests and coordinates, never file contents,
//! filesystem authority, declaration lookup or a source compiler continuation.
use sha2::{Digest, Sha256};
pub use source_files::DependencyScope;

const SOURCE_CONTENT_DOMAIN: &[u8] = b"OMEGA-CONSUMED-SOURCE-CONTENT-V1\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConsumedSourceUnitKind {
    PackageAuthored,
    PackageGenerated(DependencyScope),
    ToolchainVirtual,
    ToolchainOwned,
    ToolchainContract,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConsumedSourceUnit {
    kind: ConsumedSourceUnitKind,
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
    toolchain_namespace: Option<String>,
    relative_path: Vec<String>,
    byte_count: u64,
    content_digest: [u8; 32],
}

impl ConsumedSourceUnit {
    #[doc(hidden)]
    pub fn from_retained(
        kind: ConsumedSourceUnitKind,
        package: Option<semantic_vocabulary::PackageKeyIdentity>,
        toolchain_namespace: Option<String>,
        relative_path: Vec<String>,
        byte_count: u64,
        content_digest: [u8; 32],
    ) -> Result<Self, &'static str> {
        let coordinates_match = match kind {
            ConsumedSourceUnitKind::PackageAuthored
            | ConsumedSourceUnitKind::PackageGenerated(_) => {
                package.is_some() && toolchain_namespace.is_none()
            }
            ConsumedSourceUnitKind::ToolchainOwned | ConsumedSourceUnitKind::ToolchainContract => {
                package.is_none()
                    && toolchain_namespace
                        .as_deref()
                        .is_some_and(|namespace| !namespace.is_empty())
            }
            ConsumedSourceUnitKind::ToolchainVirtual => {
                package.is_none() && toolchain_namespace.is_none()
            }
        };
        if !coordinates_match {
            return Err("consumed source unit kind disagrees with its retained coordinates");
        }
        if let Some(namespace) = &toolchain_namespace
            && namespace.is_empty()
        {
            return Err("consumed source unit retains an empty toolchain namespace");
        }
        if relative_path.is_empty() || relative_path.iter().any(|component| component.is_empty()) {
            return Err("consumed source unit retains a non-canonical relative path");
        }
        Ok(Self {
            kind,
            package,
            toolchain_namespace,
            relative_path,
            byte_count,
            content_digest,
        })
    }

    pub const fn kind(&self) -> ConsumedSourceUnitKind {
        self.kind
    }

    pub const fn package(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
        self.package
    }

    pub fn toolchain_namespace(&self) -> Option<&str> {
        self.toolchain_namespace.as_deref()
    }

    pub fn relative_path(&self) -> &[String] {
        &self.relative_path
    }

    pub const fn byte_count(&self) -> u64 {
        self.byte_count
    }

    pub const fn content_digest(&self) -> [u8; 32] {
        self.content_digest
    }
}

pub fn canonical_consumed_unit_bytes(unit: &ConsumedSourceUnit) -> Vec<u8> {
    let mut bytes = Vec::new();
    append_canonical_consumed_unit(&mut bytes, unit);
    bytes
}

pub fn append_canonical_consumed_unit(bytes: &mut Vec<u8>, unit: &ConsumedSourceUnit) {
    bytes.push(match unit.kind {
        ConsumedSourceUnitKind::PackageAuthored => 0,
        ConsumedSourceUnitKind::PackageGenerated(DependencyScope::Product) => 1,
        ConsumedSourceUnitKind::PackageGenerated(DependencyScope::Build) => 4,
        ConsumedSourceUnitKind::ToolchainVirtual => 2,
        ConsumedSourceUnitKind::ToolchainOwned => 3,
        ConsumedSourceUnitKind::ToolchainContract => 5,
    });
    match unit.package {
        None => bytes.push(0),
        Some(package) => {
            bytes.push(1);
            append_field(bytes, &package.digest());
        }
    }
    match &unit.toolchain_namespace {
        None => bytes.push(0),
        Some(namespace) => {
            bytes.push(1);
            append_field(bytes, namespace.as_bytes());
        }
    }
    bytes.extend_from_slice(
        &u64::try_from(unit.relative_path.len())
            .expect("consumed source path component count fits u64")
            .to_le_bytes(),
    );
    for component in &unit.relative_path {
        append_field(bytes, component.as_bytes());
    }
    bytes.extend_from_slice(&unit.byte_count.to_le_bytes());
    append_field(bytes, &unit.content_digest);
}

/// Match retained source bytes without reopening a source file. Declaration
/// descriptions and their exact toolchain owners must commit to the same
/// revision using the producer's existing framing.
pub fn toolchain_source_revision_digest(custody_entry: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"OMEGA-PACKAGE-REVIEW-TOOLCHAIN-SOURCE\0");
    digest.update(
        u64::try_from(custody_entry.len())
            .expect("canonical source custody entry length fits u64")
            .to_le_bytes(),
    );
    digest.update(custody_entry);
    digest.finalize().into()
}

fn append_field(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(
        &u64::try_from(bytes.len())
            .expect("source commitment field length fits u64")
            .to_le_bytes(),
    );
    output.extend_from_slice(bytes);
}

pub fn source_content_digest(source: &[u8]) -> [u8; 32] {
    let byte_count = u64::try_from(source.len()).expect("source byte length fits u64");
    let mut digest = Sha256::new();
    digest.update(SOURCE_CONTENT_DOMAIN);
    digest.update(byte_count.to_le_bytes());
    digest.update(source);
    digest.finalize().into()
}

/// Identify one immutable bundled definition using the same framing that
/// source admission publishes. The caller supplies explicit bytes; this does
/// not open a source file, admit a declaration or establish package authority.
pub fn toolchain_owned_identity_digest(
    namespace: &str,
    relative_path: Vec<String>,
    source: &[u8],
) -> Result<[u8; 32], &'static str> {
    let canonical_component = |component: &str| {
        !component.is_empty()
            && !matches!(component, "." | "..")
            && !component.contains(['/', '\\'])
    };
    if !canonical_component(namespace)
        || relative_path.is_empty()
        || relative_path
            .iter()
            .any(|component| !canonical_component(component))
    {
        return Err("toolchain-owned source has a noncanonical namespace or relative path");
    }
    let unit = ConsumedSourceUnit::from_retained(
        ConsumedSourceUnitKind::ToolchainOwned,
        None,
        Some(namespace.to_owned()),
        relative_path,
        u64::try_from(source.len()).map_err(|_| "source byte length exceeds u64")?,
        source_content_digest(source),
    )?;
    Ok(toolchain_source_revision_digest(
        &canonical_consumed_unit_bytes(&unit),
    ))
}
