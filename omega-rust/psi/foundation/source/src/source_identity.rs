//! Stable source ownership and byte identity shared by early Psi and package review.
//!
//! These records identify retained source bytes; they do not acquire sources,
//! select packages or grant declaration authority. The existing source-content,
//! consumed-unit and toolchain digest protocols remain byte-for-byte unchanged.

use crate::{SourceFile, SourceOrigin};
use sha2::{Digest, Sha256};
use std::path::{Component, Path};

const SOURCE_CONTENT_DOMAIN: &[u8] = b"OMEGA-CONSUMED-SOURCE-CONTENT-V1\0";

/// Exact owner class of one source unit consumed by the final checked
/// frontend closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConsumedSourceUnitKind {
    PackageAuthored,
    PackageGenerated(crate::DependencyScope),
    ToolchainVirtual,
    ToolchainOwned,
}

/// One path-stable, content-addressed source unit consumed by the final
/// checked compilation.
///
/// Physical cache roots and compiler-local source IDs are deliberately absent.
/// Package identity or toolchain namespace plus the canonical relative path
/// owns the coordinate, with the checked import scope additionally distinguishing
/// generated units; the collision-resistant content digest owns the exact
/// bytes. The ordered set of these rows is the sole source projection used by
/// both production manifests and source-consumption commitments.
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
    /// Rebuild one retained row from its decoded coordinates. Persisted
    /// projections are validated on the way in rather than trusted.
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
            ConsumedSourceUnitKind::ToolchainOwned => {
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

pub fn consumed_source_unit(
    source: &SourceFile,
    generated: bool,
) -> Result<ConsumedSourceUnit, String> {
    let (kind, package, toolchain_namespace, relative_path) = match source.origin {
        SourceOrigin::User => {
            let package = source.package_identity.ok_or_else(|| {
                format!(
                    "package-aware source `{}` has no reconciled package identity",
                    source.path.display()
                )
            })?;
            let relative = source
                .path
                .strip_prefix(&source.package_root)
                .map_err(|_| {
                    format!(
                        "package-aware source `{}` is outside its reconciled root `{}`",
                        source.path.display(),
                        source.package_root.display()
                    )
                })?;
            (
                if generated {
                    ConsumedSourceUnitKind::PackageGenerated(source.dependency_scope)
                } else {
                    ConsumedSourceUnitKind::PackageAuthored
                },
                Some(package),
                None,
                canonical_path_components(relative, &source.path)?,
            )
        }
        SourceOrigin::Toolchain if generated => {
            return Err(format!(
                "generated-source custody cannot name toolchain source `{}`",
                source.path.display()
            ));
        }
        SourceOrigin::Toolchain if is_virtual_toolchain_path(&source.path) => (
            ConsumedSourceUnitKind::ToolchainVirtual,
            None,
            None,
            canonical_path_components(&source.path, &source.path)?,
        ),
        SourceOrigin::Toolchain => {
            let namespace = source
                .package_root
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| {
                    format!(
                        "toolchain source root `{}` has no canonical UTF-8 namespace",
                        source.package_root.display()
                    )
                })?;
            let relative = source
                .path
                .strip_prefix(&source.package_root)
                .map_err(|_| {
                    format!(
                        "toolchain source `{}` is outside its canonical root `{}`",
                        source.path.display(),
                        source.package_root.display()
                    )
                })?;
            (
                ConsumedSourceUnitKind::ToolchainOwned,
                None,
                Some(namespace.to_owned()),
                canonical_path_components(relative, &source.path)?,
            )
        }
    };
    let byte_count = u64::try_from(source.source.len()).map_err(|_| {
        format!(
            "compiler-consumed source `{}` byte length exceeds the canonical range",
            source.path.display()
        )
    })?;
    let mut digest = Sha256::new();
    digest.update(SOURCE_CONTENT_DOMAIN);
    digest.update(byte_count.to_le_bytes());
    digest.update(source.source.as_bytes());
    Ok(ConsumedSourceUnit {
        kind,
        package,
        toolchain_namespace,
        relative_path,
        byte_count,
        content_digest: digest.finalize().into(),
    })
}

fn canonical_consumed_unit_bytes(unit: &ConsumedSourceUnit) -> Vec<u8> {
    let mut bytes = Vec::new();
    append_canonical_consumed_unit(&mut bytes, unit);
    bytes
}

pub fn append_canonical_consumed_unit(bytes: &mut Vec<u8>, unit: &ConsumedSourceUnit) {
    bytes.push(match unit.kind {
        ConsumedSourceUnitKind::PackageAuthored => 0,
        ConsumedSourceUnitKind::PackageGenerated(crate::DependencyScope::Product) => 1,
        ConsumedSourceUnitKind::PackageGenerated(crate::DependencyScope::Build) => 4,
        ConsumedSourceUnitKind::ToolchainVirtual => 2,
        ConsumedSourceUnitKind::ToolchainOwned => 3,
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

pub fn canonical_path_components(
    relative: &Path,
    diagnostic_path: &Path,
) -> Result<Vec<String>, String> {
    let components = relative.components().collect::<Vec<_>>();
    if components.is_empty() {
        return Err(format!(
            "compiler-consumed source `{}` has an empty canonical path",
            diagnostic_path.display()
        ));
    }
    components
        .into_iter()
        .map(|component| {
            let Component::Normal(component) = component else {
                return Err(format!(
                    "compiler-consumed source `{}` has a non-canonical relative path",
                    diagnostic_path.display()
                ));
            };
            component.to_str().map(str::to_owned).ok_or_else(|| {
                format!(
                    "compiler-consumed source `{}` has a non-UTF-8 path component",
                    diagnostic_path.display()
                )
            })
        })
        .collect()
}

/// Exact source-backed toolchain owners retained for compiler source-free
/// nominal and type identity. `SourceId` is only the compiler-internal join
/// coordinate; canonical identity contains the collision-resistant digest.
#[doc(hidden)]
pub fn toolchain_source_identities<'source>(
    sources: impl IntoIterator<Item = &'source SourceFile>,
) -> Result<Vec<(crate::SourceId, [u8; 32])>, String> {
    let mut identities = sources
        .into_iter()
        .filter(|source| source.origin == SourceOrigin::Toolchain)
        .map(|source| Ok((source.source_id, toolchain_source_identity_digest(source)?)))
        .collect::<Result<Vec<_>, String>>()?;
    identities.sort_by_key(|(source_id, _)| source_id.0);
    Ok(identities)
}

#[doc(hidden)]
pub fn toolchain_source_identity_digest(source: &SourceFile) -> Result<[u8; 32], String> {
    if source.origin != SourceOrigin::Toolchain {
        return Err(format!(
            "toolchain source identity requested for non-toolchain source `{}`",
            source.path.display(),
        ));
    }
    let custody_entry = canonical_source_entry(source)?;
    let mut digest = Sha256::new();
    digest.update(b"OMEGA-PACKAGE-REVIEW-TOOLCHAIN-SOURCE\0");
    digest.update(
        u64::try_from(custody_entry.len())
            .expect("canonical source custody entry length fits u64")
            .to_le_bytes(),
    );
    digest.update(&custody_entry);
    Ok(digest.finalize().into())
}

pub fn canonical_source_entry(source: &SourceFile) -> Result<Vec<u8>, String> {
    Ok(canonical_consumed_unit_bytes(&consumed_source_unit(
        source, false,
    )?))
}

fn append_field(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(
        &u64::try_from(bytes.len())
            .expect("source commitment field length fits u64")
            .to_le_bytes(),
    );
    output.extend_from_slice(bytes);
}

fn is_virtual_toolchain_path(path: &Path) -> bool {
    let mut components = path.components();
    let Some(Component::Normal(component)) = components.next() else {
        return false;
    };
    components.next().is_none()
        && component
            .to_str()
            .is_some_and(|component| component.starts_with('<') && component.ends_with('>'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toolchain(root: &str) -> SourceFile {
        let mut sources = crate::SourceMap::default();
        let root = std::path::PathBuf::from(root).join("core");
        sources
            .add_with_metadata(
                root.join("numbers.omg"),
                "machine answer() -> u64 { 16 }".into(),
                root,
                None,
                SourceOrigin::Toolchain,
            )
            .clone()
    }

    #[test]
    fn toolchain_protocol_is_stable_across_physical_roots_and_source_ids() {
        let first = toolchain("first/toolchain");
        let mut relocated = toolchain("different/cache");
        relocated.source_id = crate::SourceId(417);
        let digest = toolchain_source_identity_digest(&first).expect("identity");
        let hexadecimal = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(
            hexadecimal,
            "8aa68ba24b8af7d4b20c3b58d6a447d0cd25214f14901a6956fb815578d4a4fb"
        );
        assert_eq!(
            digest,
            toolchain_source_identity_digest(&relocated).expect("relocated identity")
        );
        relocated.source = "machine answer() -> u64 { 17 }".into();
        assert_ne!(
            digest,
            toolchain_source_identity_digest(&relocated).expect("changed identity")
        );
    }

    #[test]
    fn source_identity_does_not_grant_toolchain_or_generated_authority() {
        let mut file = toolchain("toolchain");
        assert!(
            consumed_source_unit(&file, true)
                .expect_err("generated toolchain")
                .contains("generated-source custody")
        );
        file.origin = SourceOrigin::User;
        assert!(
            toolchain_source_identity_digest(&file)
                .expect_err("user source")
                .contains("non-toolchain")
        );
        assert!(
            consumed_source_unit(&file, false)
                .expect_err("missing package")
                .contains("no reconciled package identity")
        );
    }

    #[test]
    fn generated_coordinates_retain_checked_scope_but_authored_bytes_share() {
        let mut file = toolchain("package");
        file.origin = SourceOrigin::User;
        file.package_identity = semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]);
        let authored = consumed_source_unit(&file, false).expect("authored");
        let generated = consumed_source_unit(&file, true).expect("generated");
        file.dependency_scope = crate::DependencyScope::Build;
        assert_eq!(
            authored,
            consumed_source_unit(&file, false).expect("authored Build")
        );
        assert_ne!(
            generated,
            consumed_source_unit(&file, true).expect("generated Build")
        );
    }
}
