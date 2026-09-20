use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use sha2::{Digest, Sha256};
use source::{SourceFile, SourceOrigin};
use std::path::{Component, Path};

const SOURCE_CONTENT_DOMAIN: &[u8] = b"OMEGA-CONSUMED-SOURCE-CONTENT-V1\0";
const SOURCE_CONSUMPTION_DOMAIN: &[u8] = b"OMEGA-PACKAGE-SOURCE-CONSUMPTION-V4\0";

/// Exact owner class of one source unit consumed by the final checked
/// frontend closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConsumedSourceUnitKind {
    PackageAuthored,
    PackageGenerated(source::DependencyScope),
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
    /// Construct one canonical source row for custody coverage. The retained
    /// coordinate must match the shape [`consumed_source_unit`] derives: a
    /// package-authored or package-generated unit names its reconciled package
    /// and no toolchain namespace, an owned toolchain unit names its canonical
    /// namespace and no package, and a virtual toolchain unit names neither.
    /// Every row retains a nonempty relative path of nonempty components, and
    /// a toolchain namespace is never empty.
    #[cfg(any(test, feature = "test-support"))]
    pub fn for_test(
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
    root: semantic_vocabulary::PackageKeyIdentity,
    dependency_closure: super::PackageDependencyClosure,
    source_consumption_commitment: PackageSourceConsumptionCommitment,
    consumed_units: Vec<ConsumedSourceUnit>,
}

impl PackageCompilationSubject {
    pub const fn root(&self) -> semantic_vocabulary::PackageKeyIdentity {
        self.root
    }

    pub const fn dependency_closure(&self) -> &super::PackageDependencyClosure {
        &self.dependency_closure
    }

    pub const fn root_role(&self) -> super::BuildDeclarationKind {
        self.dependency_closure.root_role()
    }

    pub const fn source_consumption_commitment(&self) -> PackageSourceConsumptionCommitment {
        self.source_consumption_commitment
    }

    pub fn consumed_units(&self) -> &[ConsumedSourceUnit] {
        &self.consumed_units
    }

    /// Append the counted, length-prefixed source rows directly to a production
    /// manifest. No separately allocated row collection is retained.
    /// Package-graph coordinates are carried separately by
    /// `dependency_closure`; absolute routing paths never enter these bytes.
    #[doc(hidden)]
    pub fn append_canonical_consumed_units(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(
            &u64::try_from(self.consumed_units.len())
                .expect("consumed source count fits u64")
                .to_le_bytes(),
        );
        for unit in &self.consumed_units {
            // Fill the length after writing the row, avoiding both a sizing
            // traversal and temporary row storage.
            let length_offset = bytes.len();
            bytes.extend_from_slice(&0u64.to_le_bytes());
            let row_start = bytes.len();
            append_canonical_consumed_unit(bytes, unit);
            let row_length = u64::try_from(bytes.len() - row_start)
                .expect("consumed source row length fits u64");
            bytes[length_offset..row_start].copy_from_slice(&row_length.to_le_bytes());
        }
    }

    /// Construct the package/source subject from canonical parts for custody
    /// coverage. The retained shape must match what
    /// [`derive_package_compilation_subject`] produces: `root` is the closure
    /// root, the consumed rows are nonempty, strictly ordered, and unique, and
    /// every row naming a package resolves inside the retained closure.
    #[cfg(any(test, feature = "test-support"))]
    pub fn for_test(
        root: semantic_vocabulary::PackageKeyIdentity,
        dependency_closure: super::PackageDependencyClosure,
        source_consumption_commitment: PackageSourceConsumptionCommitment,
        consumed_units: Vec<ConsumedSourceUnit>,
    ) -> Result<Self, &'static str> {
        if dependency_closure.root() != root {
            return Err("package compilation subject root disagrees with its dependency closure");
        }
        if consumed_units.is_empty() {
            return Err("package compilation subject requires consumed source units");
        }
        if consumed_units.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("package compilation subject requires strictly ordered unique units");
        }
        let packages = dependency_closure.packages();
        if consumed_units
            .iter()
            .filter_map(ConsumedSourceUnit::package)
            .any(|package| !packages.contains(&package))
        {
            return Err("consumed source unit names a package outside the dependency closure");
        }
        Ok(Self {
            root,
            dependency_closure,
            source_consumption_commitment,
            consumed_units,
        })
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
    digest.update([canonical_root_role(inputs.root_role())]);
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
    let mut row_bytes = Vec::new();
    for unit in consumed_units {
        row_bytes.clear();
        append_canonical_consumed_unit(&mut row_bytes, unit);
        hash_field(&mut digest, &row_bytes);
    }
    Ok(PackageSourceConsumptionCommitment {
        digest: digest.finalize().into(),
    })
}

fn canonical_root_role(role: super::BuildDeclarationKind) -> u8 {
    match role {
        super::BuildDeclarationKind::Package => 0,
        super::BuildDeclarationKind::Application => 1,
        super::BuildDeclarationKind::Workspace => {
            unreachable!("workspace roots reject before source-consumption commitment")
        }
    }
}

/// Derive the one canonical source projection from the final checked closure.
/// Generated-source custody is joined by private `SourceId` only while this
/// projection is constructed; IDs never enter the retained rows.
#[doc(hidden)]
pub fn derive_consumed_source_units(
    program: &CheckedTrees,
    generated_sources: &[(source::SourceId, build_output::PackageGeneratedSource)],
) -> Result<Vec<ConsumedSourceUnit>, Vec<Diagnostic>> {
    let generated_by_id = GeneratedSourceOrder::new(generated_sources);
    if (1..generated_sources.len())
        .any(|position| generated_by_id.at(position - 1).0 == generated_by_id.at(position).0)
    {
        return Err(vec![Diagnostic::error(
            "generated-source custody contains duplicate frontend source IDs",
        )]);
    }

    let mut observed_generated = vec![false; generated_sources.len()];
    let mut units = program
        .typed
        .symbols
        .source_files()
        .map(|source| {
            let generated = generated_by_id.find(source.source_id);
            if let Some((position, generated)) = generated {
                validate_generated_source_join(source, generated)?;
                observed_generated[position] = true;
            }
            consumed_source_unit(source, generated.is_some())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if observed_generated.contains(&false) {
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
    // Authored instances share their physical bytes across checked scopes and
    // retain one source row. Generated instances include their checked scope
    // in the coordinate: equal virtual paths may carry different bytes across
    // scopes, but conflicting bytes within one coordinate still reject.
    units.dedup();
    if units
        .windows(2)
        .any(|pair| same_source_coordinate(&pair[0], &pair[1]))
    {
        return Err(vec![Diagnostic::error(
            "final checked closure contains distinct sources at duplicate canonical coordinates",
        )]);
    }
    Ok(units)
}

/// Producers append custody in source-ID order. Borrow that order directly;
/// arbitrary callers need only a compact permutation, never a source-ID-sized
/// table. SourceMap::from_files can retain arbitrary source order, so consumers
/// search this view rather than assume that source iteration is monotonic.
struct GeneratedSourceOrder<'source> {
    sources: &'source [(source::SourceId, build_output::PackageGeneratedSource)],
    positions: Option<Vec<usize>>,
}

impl<'source> GeneratedSourceOrder<'source> {
    fn new(sources: &'source [(source::SourceId, build_output::PackageGeneratedSource)]) -> Self {
        let positions = sources
            .windows(2)
            .any(|pair| pair[0].0.0 > pair[1].0.0)
            .then(|| {
                let mut positions = (0..sources.len()).collect::<Vec<_>>();
                // Preserve the first input occurrence for verification callers;
                // projection separately rejects every duplicate source ID.
                positions.sort_unstable_by_key(|position| (sources[*position].0.0, *position));
                positions
            });
        Self { sources, positions }
    }

    fn at(
        &self,
        position: usize,
    ) -> &'source (source::SourceId, build_output::PackageGeneratedSource) {
        &self.sources[self
            .positions
            .as_ref()
            .map_or(position, |positions| positions[position])]
    }

    fn find(
        &self,
        source_id: source::SourceId,
    ) -> Option<(usize, &'source build_output::PackageGeneratedSource)> {
        let position = match &self.positions {
            Some(positions) => {
                positions.partition_point(|position| self.sources[*position].0.0 < source_id.0)
            }
            None => self
                .sources
                .partition_point(|(candidate, _)| candidate.0 < source_id.0),
        };
        (position < self.sources.len() && self.at(position).0 == source_id)
            .then(|| (position, &self.at(position).1))
    }
}

fn validate_generated_source_join(
    source: &SourceFile,
    generated: &build_output::PackageGeneratedSource,
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
    generated_sources: &[(source::SourceId, build_output::PackageGeneratedSource)],
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
    append_canonical_consumed_unit(&mut bytes, unit);
    bytes
}

fn append_canonical_consumed_unit(bytes: &mut Vec<u8>, unit: &ConsumedSourceUnit) {
    bytes.push(match unit.kind {
        ConsumedSourceUnitKind::PackageAuthored => 0,
        ConsumedSourceUnitKind::PackageGenerated(source::DependencyScope::Product) => 1,
        ConsumedSourceUnitKind::PackageGenerated(source::DependencyScope::Build) => 4,
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
    generated_sources: &[(source::SourceId, build_output::PackageGeneratedSource)],
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let generated_by_id = GeneratedSourceOrder::new(generated_sources);
    for source in program.typed.symbols.source_files() {
        if source.origin == SourceOrigin::Toolchain && is_virtual_toolchain_path(&source.path) {
            continue;
        }
        if let Some((_, generated)) = generated_by_id.find(source.source_id) {
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

/// Exact source-backed toolchain owners retained for compiler source-free
/// nominal and type identity. `SourceId` is only the compiler-internal join
/// coordinate; canonical identity contains the collision-resistant digest.
#[doc(hidden)]
pub fn toolchain_source_identities(
    program: &CheckedTrees,
) -> Result<Vec<(source::SourceId, [u8; 32])>, Vec<Diagnostic>> {
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
mod tests;
