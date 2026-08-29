use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use psi_source::{SourceFile, SourceOrigin};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Component, Path};

const SOURCE_CONTENT_DOMAIN: &[u8] = b"OMEGA-CONSUMED-SOURCE-CONTENT-V1\0";
const SOURCE_CONSUMPTION_DOMAIN: &[u8] = b"OMEGA-PACKAGE-SOURCE-CONSUMPTION-V2\0";

/// Exact owner class of one source unit consumed by the final checked
/// frontend closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConsumedSourceUnitKind {
    PackageAuthored,
    PackageGenerated,
    ToolchainVirtual,
    ToolchainOwned,
}

/// One path-stable, content-addressed source unit consumed by the final
/// checked compilation.
///
/// Physical cache roots and compiler-local source IDs are deliberately absent.
/// Package identity or toolchain namespace plus the canonical relative path
/// owns the coordinate; the collision-resistant content digest owns the exact
/// bytes. The ordered set of these rows is the sole source projection used by
/// both production manifests and source-consumption commitments.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConsumedSourceUnit {
    kind: ConsumedSourceUnitKind,
    package: Option<psi_core::PackageKeyIdentity>,
    toolchain_namespace: Option<String>,
    relative_path: Vec<String>,
    byte_count: u64,
    content_digest: [u8; 32],
}

impl ConsumedSourceUnit {
    pub const fn kind(&self) -> ConsumedSourceUnitKind {
        self.kind
    }

    pub const fn package(&self) -> Option<psi_core::PackageKeyIdentity> {
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

/// Compiler-owned commitment to the reconciled package graph and exact
/// authored/toolchain source bytes retained by one package-aware frontend run.
///
/// This binds review output to what the compiler consumed. It is not an
/// accepted package instance, a whole-compiler identity, or protection against
/// a hostile process that can race every filesystem observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageSourceConsumptionCommitment {
    digest: [u8; 32],
}

/// Canonical package and source subject consumed by one final checked
/// package-aware compilation. This is the only retained package-source
/// authority: compatibility accessors must project from it rather than retain
/// parallel copies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageCompilationSubject {
    root: psi_core::PackageKeyIdentity,
    dependency_closure: super::PackageDependencyClosure,
    source_consumption_commitment: PackageSourceConsumptionCommitment,
    consumed_units: Vec<ConsumedSourceUnit>,
}

impl PackageCompilationSubject {
    pub const fn root(&self) -> psi_core::PackageKeyIdentity {
        self.root
    }

    pub const fn dependency_closure(&self) -> &super::PackageDependencyClosure {
        &self.dependency_closure
    }

    pub const fn source_consumption_commitment(&self) -> PackageSourceConsumptionCommitment {
        self.source_consumption_commitment
    }

    pub fn consumed_units(&self) -> &[ConsumedSourceUnit] {
        &self.consumed_units
    }

    /// Canonical source rows used by the production compilation manifest.
    /// Package-graph coordinates are carried separately by
    /// `dependency_closure`; absolute routing paths never enter these bytes.
    #[doc(hidden)]
    pub fn canonical_consumed_unit_bytes(&self) -> Vec<Vec<u8>> {
        self.consumed_units
            .iter()
            .map(canonical_consumed_unit_bytes)
            .collect()
    }
}

impl PackageSourceConsumptionCommitment {
    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub const fn for_test(digest: [u8; 32]) -> Self {
        Self { digest }
    }

    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }
}

#[doc(hidden)]
pub fn derive_source_consumption_commitment(
    consumed_units: &[ConsumedSourceUnit],
    inputs: &super::PackageCompilationInputs,
) -> Result<PackageSourceConsumptionCommitment, Vec<Diagnostic>> {
    if consumed_units.is_empty() {
        return Err(vec![Diagnostic::error(
            "package source consumption commitment requires retained frontend source metadata",
        )]);
    }
    if consumed_units.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(vec![Diagnostic::error(
            "package source consumption commitment requires strictly ordered unique consumed units",
        )]);
    }

    let mut digest = Sha256::new();
    digest.update(SOURCE_CONSUMPTION_DOMAIN);
    hash_field(&mut digest, &inputs.root().digest());
    let packages = inputs
        .packages()
        .map(|(identity, _)| identity)
        .collect::<Vec<_>>();
    digest.update(
        u64::try_from(packages.len())
            .expect("compiler package count fits u64")
            .to_le_bytes(),
    );
    for package in packages {
        hash_field(&mut digest, &package.digest());
    }
    let dependencies = inputs.dependencies().collect::<Vec<_>>();
    digest.update(
        u64::try_from(dependencies.len())
            .expect("compiler dependency count fits u64")
            .to_le_bytes(),
    );
    for (requester, alias, target) in dependencies {
        hash_field(&mut digest, &requester.digest());
        hash_field(&mut digest, alias.as_bytes());
        hash_field(&mut digest, &target.digest());
    }
    digest.update(
        u64::try_from(consumed_units.len())
            .expect("loaded source count fits u64")
            .to_le_bytes(),
    );
    for unit in consumed_units {
        hash_field(&mut digest, &canonical_consumed_unit_bytes(unit));
    }
    Ok(PackageSourceConsumptionCommitment {
        digest: digest.finalize().into(),
    })
}

/// Derive the one canonical source projection from the final checked closure.
/// Generated-source custody is joined by private `SourceId` only while this
/// projection is constructed; IDs never enter the retained rows.
#[doc(hidden)]
pub fn derive_consumed_source_units(
    program: &CheckedTrees,
    generated_sources: &[(
        psi_source::SourceId,
        omega_build_output::PackageGeneratedSource,
    )],
) -> Result<Vec<ConsumedSourceUnit>, Vec<Diagnostic>> {
    let generated_by_id = generated_sources
        .iter()
        .map(|(source_id, generated)| (source_id.0, generated))
        .collect::<BTreeMap<_, _>>();
    if generated_by_id.len() != generated_sources.len() {
        return Err(vec![Diagnostic::error(
            "generated-source custody contains duplicate frontend source IDs",
        )]);
    }

    let mut observed_generated = BTreeMap::new();
    let mut units = program
        .typed
        .symbols
        .source_files()
        .map(|source| {
            let generated = generated_by_id.get(&source.source_id.0).copied();
            if let Some(generated) = generated {
                validate_generated_source_join(source, generated)?;
                observed_generated.insert(source.source_id.0, generated);
            }
            consumed_source_unit(source, generated.is_some())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if observed_generated.len() != generated_by_id.len() {
        return Err(vec![Diagnostic::error(
            "generated-source custody names a source absent from the final checked closure",
        )]);
    }
    if units.is_empty() {
        return Err(vec![Diagnostic::error(
            "consumed-source projection requires retained frontend source metadata",
        )]);
    }
    units.sort();
    if units
        .windows(2)
        .any(|pair| same_source_coordinate(&pair[0], &pair[1]))
    {
        return Err(vec![Diagnostic::error(
            "final checked closure contains duplicate canonical source coordinates",
        )]);
    }
    Ok(units)
}

fn validate_generated_source_join(
    source: &SourceFile,
    generated: &omega_build_output::PackageGeneratedSource,
) -> Result<(), Vec<Diagnostic>> {
    if source.origin != SourceOrigin::User || source.source.as_bytes() != generated.bytes() {
        return Err(vec![Diagnostic::error(format!(
            "generated-source custody does not match final checked source `{}`",
            source.path.display()
        ))]);
    }
    let relative = source
        .path
        .strip_prefix(&source.package_root)
        .map_err(|_| {
            vec![Diagnostic::error(format!(
                "generated source `{}` is outside its reconciled package root",
                source.path.display()
            ))]
        })?;
    let logical_path = canonical_path_components(relative, &source.path)?.join("/");
    let expected_path = [b".omega/generated/".as_slice(), generated.relative_path()].concat();
    if logical_path.as_bytes() != expected_path {
        return Err(vec![Diagnostic::error(format!(
            "generated-source custody path does not match final checked source `{}`",
            source.path.display()
        ))]);
    }
    Ok(())
}

/// Derive the one package/source subject from the final checked closure.
#[doc(hidden)]
pub fn derive_package_compilation_subject(
    program: &CheckedTrees,
    inputs: &super::PackageCompilationInputs,
    generated_sources: &[(
        psi_source::SourceId,
        omega_build_output::PackageGeneratedSource,
    )],
) -> Result<PackageCompilationSubject, Vec<Diagnostic>> {
    let consumed_units = derive_consumed_source_units(program, generated_sources)?;
    let source_consumption_commitment =
        derive_source_consumption_commitment(&consumed_units, inputs)?;
    Ok(PackageCompilationSubject {
        root: inputs.root(),
        dependency_closure: inputs.dependency_closure(),
        source_consumption_commitment,
        consumed_units,
    })
}

fn consumed_source_unit(
    source: &SourceFile,
    generated: bool,
) -> Result<ConsumedSourceUnit, Vec<Diagnostic>> {
    let (kind, package, toolchain_namespace, relative_path) = match source.origin {
        SourceOrigin::User => {
            let package = source.package_identity.ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "package-aware source `{}` has no reconciled package identity",
                    source.path.display()
                ))]
            })?;
            let relative = source
                .path
                .strip_prefix(&source.package_root)
                .map_err(|_| {
                    vec![Diagnostic::error(format!(
                        "package-aware source `{}` is outside its reconciled root `{}`",
                        source.path.display(),
                        source.package_root.display()
                    ))]
                })?;
            (
                if generated {
                    ConsumedSourceUnitKind::PackageGenerated
                } else {
                    ConsumedSourceUnitKind::PackageAuthored
                },
                Some(package),
                None,
                canonical_path_components(relative, &source.path)?,
            )
        }
        SourceOrigin::Toolchain if generated => {
            return Err(vec![Diagnostic::error(format!(
                "generated-source custody cannot name toolchain source `{}`",
                source.path.display()
            ))]);
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
                    vec![Diagnostic::error(format!(
                        "toolchain source root `{}` has no canonical UTF-8 namespace",
                        source.package_root.display()
                    ))]
                })?;
            let relative = source
                .path
                .strip_prefix(&source.package_root)
                .map_err(|_| {
                    vec![Diagnostic::error(format!(
                        "toolchain source `{}` is outside its canonical root `{}`",
                        source.path.display(),
                        source.package_root.display()
                    ))]
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
        vec![Diagnostic::error(format!(
            "compiler-consumed source `{}` byte length exceeds the canonical range",
            source.path.display()
        ))]
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

fn same_source_coordinate(left: &ConsumedSourceUnit, right: &ConsumedSourceUnit) -> bool {
    left.kind == right.kind
        && left.package == right.package
        && left.toolchain_namespace == right.toolchain_namespace
        && left.relative_path == right.relative_path
}

fn canonical_consumed_unit_bytes(unit: &ConsumedSourceUnit) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(match unit.kind {
        ConsumedSourceUnitKind::PackageAuthored => 0,
        ConsumedSourceUnitKind::PackageGenerated => 1,
        ConsumedSourceUnitKind::ToolchainVirtual => 2,
        ConsumedSourceUnitKind::ToolchainOwned => 3,
    });
    match unit.package {
        None => bytes.push(0),
        Some(package) => {
            bytes.push(1);
            append_field(&mut bytes, &package.digest());
        }
    }
    match &unit.toolchain_namespace {
        None => bytes.push(0),
        Some(namespace) => {
            bytes.push(1);
            append_field(&mut bytes, namespace.as_bytes());
        }
    }
    bytes.extend_from_slice(
        &u64::try_from(unit.relative_path.len())
            .expect("consumed source path component count fits u64")
            .to_le_bytes(),
    );
    for component in &unit.relative_path {
        append_field(&mut bytes, component.as_bytes());
    }
    bytes.extend_from_slice(&unit.byte_count.to_le_bytes());
    append_field(&mut bytes, &unit.content_digest);
    bytes
}

fn canonical_path_components(
    relative: &Path,
    diagnostic_path: &Path,
) -> Result<Vec<String>, Vec<Diagnostic>> {
    let components = relative.components().collect::<Vec<_>>();
    if components.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "compiler-consumed source `{}` has an empty canonical path",
            diagnostic_path.display()
        ))]);
    }
    components
        .into_iter()
        .map(|component| {
            let Component::Normal(component) = component else {
                return Err(vec![Diagnostic::error(format!(
                    "compiler-consumed source `{}` has a non-canonical relative path",
                    diagnostic_path.display()
                ))]);
            };
            component.to_str().map(str::to_owned).ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "compiler-consumed source `{}` has a non-UTF-8 path component",
                    diagnostic_path.display()
                ))]
            })
        })
        .collect()
}

#[doc(hidden)]
pub fn verify_current_files(
    program: &CheckedTrees,
    generated_sources: &[(
        psi_source::SourceId,
        omega_build_output::PackageGeneratedSource,
    )],
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for source in program.typed.symbols.source_files() {
        if source.origin == SourceOrigin::Toolchain && is_virtual_toolchain_path(&source.path) {
            continue;
        }
        if let Some((_, generated)) = generated_sources
            .iter()
            .find(|(source_id, _)| *source_id == source.source_id)
        {
            if source.source.as_bytes() != generated.bytes() {
                diagnostics.push(Diagnostic::error(format!(
                    "compiler-retained generated source `{}` drifted from staged-output custody",
                    source.path.display()
                )));
            }
            continue;
        }
        match std::fs::read(&source.path) {
            Ok(current) if current == source.source.as_bytes() => {}
            Ok(_) => diagnostics.push(Diagnostic::error(format!(
                "compiler-consumed source `{}` changed after frontend loading",
                source.path.display()
            ))),
            Err(error) => diagnostics.push(Diagnostic::error(format!(
                "compiler-consumed source `{}` cannot be re-read: {error}",
                source.path.display()
            ))),
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Exact source-backed toolchain owners retained for package-review nominal
/// type identity. `SourceId` is only the compiler-internal join coordinate;
/// the canonical type string contains the collision-resistant digest alone.
#[doc(hidden)]
pub fn toolchain_source_identities(
    program: &CheckedTrees,
) -> Result<Vec<(psi_source::SourceId, [u8; 32])>, Vec<Diagnostic>> {
    let mut identities = program
        .typed
        .symbols
        .source_files()
        .filter(|source| source.origin == SourceOrigin::Toolchain)
        .map(|source| Ok((source.source_id, toolchain_source_identity_digest(source)?)))
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    identities.sort_by_key(|(source_id, _)| source_id.0);
    Ok(identities)
}

#[doc(hidden)]
pub fn toolchain_source_identity_digest(source: &SourceFile) -> Result<[u8; 32], Vec<Diagnostic>> {
    if source.origin != SourceOrigin::Toolchain {
        return Err(vec![Diagnostic::error(format!(
            "toolchain source identity requested for non-toolchain source `{}`",
            source.path.display(),
        ))]);
    }
    let custody_entry = canonical_source_entry(source)?;
    let mut digest = Sha256::new();
    digest.update(b"OMEGA-PACKAGE-REVIEW-TOOLCHAIN-SOURCE\0");
    digest.update(
        u64::try_from(custody_entry.len())
            .expect("canonical source custody entry length fits u64")
            .to_le_bytes(),
    );
    digest.update(custody_entry);
    Ok(digest.finalize().into())
}

pub(super) fn canonical_source_entry(source: &SourceFile) -> Result<Vec<u8>, Vec<Diagnostic>> {
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

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(
        u64::try_from(bytes.len())
            .expect("source commitment entry length fits u64")
            .to_le_bytes(),
    );
    digest.update(bytes);
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
    use psi_core::PackageKeyIdentity;
    use psi_source::SourceId;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn source(
        path: &str,
        root: &str,
        package: Option<PackageKeyIdentity>,
        origin: SourceOrigin,
        text: &str,
    ) -> SourceFile {
        SourceFile {
            source_id: SourceId(0),
            path: PathBuf::from(path),
            package_root: PathBuf::from(root),
            package_identity: package,
            origin,
            source: Arc::from(text),
        }
    }

    #[test]
    fn canonical_entries_ignore_absolute_package_location() {
        let package = PackageKeyIdentity::from_digest([7; 32]).expect("package identity");
        let first = source(
            "/cache/one/pkg/main.omg",
            "/cache/one/pkg",
            Some(package),
            SourceOrigin::User,
            "machine main {}",
        );
        let second = source(
            "/different/cache/pkg/main.omg",
            "/different/cache/pkg",
            Some(package),
            SourceOrigin::User,
            "machine main {}",
        );
        assert_eq!(
            canonical_source_entry(&first).expect("first canonical entry"),
            canonical_source_entry(&second).expect("second canonical entry")
        );
    }

    #[test]
    fn canonical_entries_bind_package_path_and_bytes() {
        let first_package = PackageKeyIdentity::from_digest([7; 32]).expect("first package");
        let second_package = PackageKeyIdentity::from_digest([8; 32]).expect("second package");
        let baseline = source(
            "/cache/pkg/main.omg",
            "/cache/pkg",
            Some(first_package),
            SourceOrigin::User,
            "machine main {}",
        );
        let other_package = source(
            "/cache/pkg/main.omg",
            "/cache/pkg",
            Some(second_package),
            SourceOrigin::User,
            "machine main {}",
        );
        let other_path = source(
            "/cache/pkg/lib.omg",
            "/cache/pkg",
            Some(first_package),
            SourceOrigin::User,
            "machine main {}",
        );
        let other_bytes = source(
            "/cache/pkg/main.omg",
            "/cache/pkg",
            Some(first_package),
            SourceOrigin::User,
            "machine changed {}",
        );
        let baseline = canonical_source_entry(&baseline).expect("baseline entry");
        assert_ne!(baseline, canonical_source_entry(&other_package).unwrap());
        assert_ne!(baseline, canonical_source_entry(&other_path).unwrap());
        assert_ne!(baseline, canonical_source_entry(&other_bytes).unwrap());
    }

    #[test]
    fn consumed_units_are_logical_content_addressed_and_classified() {
        let package = PackageKeyIdentity::from_digest([7; 32]).expect("package identity");
        let authored = source(
            "/host/cache/pkg/main.omg",
            "/host/cache/pkg",
            Some(package),
            SourceOrigin::User,
            "machine main {}",
        );
        let relocated = source(
            "/other/root/pkg/main.omg",
            "/other/root/pkg",
            Some(package),
            SourceOrigin::User,
            "machine main {}",
        );
        let generated = consumed_source_unit(&authored, true).expect("generated row");
        let authored = consumed_source_unit(&authored, false).expect("authored row");
        let relocated = consumed_source_unit(&relocated, false).expect("relocated row");

        assert_eq!(authored, relocated);
        assert_eq!(authored.kind(), ConsumedSourceUnitKind::PackageAuthored);
        assert_eq!(generated.kind(), ConsumedSourceUnitKind::PackageGenerated);
        assert_ne!(authored, generated);
        assert!(
            !canonical_consumed_unit_bytes(&authored)
                .windows(b"/host/cache".len())
                .any(|window| window == b"/host/cache")
        );

        let virtual_source = source(
            "<prelude>",
            "toolchain/std",
            None,
            SourceOrigin::Toolchain,
            "data Unit {}",
        );
        let owned_source = source(
            "toolchain/std/types.omg",
            "toolchain/std",
            None,
            SourceOrigin::Toolchain,
            "data Unit {}",
        );
        assert_eq!(
            consumed_source_unit(&virtual_source, false)
                .expect("virtual row")
                .kind(),
            ConsumedSourceUnitKind::ToolchainVirtual
        );
        assert_eq!(
            consumed_source_unit(&owned_source, false)
                .expect("owned row")
                .kind(),
            ConsumedSourceUnitKind::ToolchainOwned
        );
    }

    #[test]
    fn toolchain_source_identity_binds_namespace_path_and_exact_bytes() {
        let baseline = source(
            "toolchain/std/types.omg",
            "toolchain/std",
            None,
            SourceOrigin::Toolchain,
            "data Packet {}",
        );
        let changed_namespace = source(
            "toolchain/core/types.omg",
            "toolchain/core",
            None,
            SourceOrigin::Toolchain,
            "data Packet {}",
        );
        let changed_path = source(
            "toolchain/std/other.omg",
            "toolchain/std",
            None,
            SourceOrigin::Toolchain,
            "data Packet {}",
        );
        let changed_bytes = source(
            "toolchain/std/types.omg",
            "toolchain/std",
            None,
            SourceOrigin::Toolchain,
            "data Packet { value: u8; }",
        );

        let baseline = toolchain_source_identity_digest(&baseline).expect("baseline identity");
        assert_ne!(
            baseline,
            toolchain_source_identity_digest(&changed_namespace).unwrap()
        );
        assert_ne!(
            baseline,
            toolchain_source_identity_digest(&changed_path).unwrap()
        );
        assert_ne!(
            baseline,
            toolchain_source_identity_digest(&changed_bytes).unwrap()
        );
    }
}
