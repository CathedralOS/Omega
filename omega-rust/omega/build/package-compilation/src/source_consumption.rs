use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use sha2::{Digest, Sha256};
use source::{SourceFile, SourceOrigin};
use std::path::{Component, Path};

const SOURCE_CONTENT_DOMAIN: &[u8] = b"OMEGA-CONSUMED-SOURCE-CONTENT-V1\0";
const SOURCE_CONSUMPTION_DOMAIN: &[u8] = b"OMEGA-PACKAGE-SOURCE-CONSUMPTION-V3\0";

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
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
    toolchain_namespace: Option<String>,
    relative_path: Vec<String>,
    byte_count: u64,
    content_digest: [u8; 32],
}

impl ConsumedSourceUnit {
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
    append_canonical_consumed_unit(&mut bytes, unit);
    bytes
}

fn append_canonical_consumed_unit(bytes: &mut Vec<u8>, unit: &ConsumedSourceUnit) {
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
mod tests {
    use super::*;
    use semantic_vocabulary::PackageKeyIdentity;
    use source::SourceId;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn generated_fixture() -> Vec<(SourceId, build_output::PackageGeneratedSource)> {
        let tree = build_output::replayed_ordinary_files(&[
            (b"first.omg", b"data First {}"),
            (b"second.omg", b"data Second {}"),
        ])
        .expect("retained generated files");
        build_output::select_included_sources(
            &tree,
            &[b"first.omg".to_vec(), b"second.omg".to_vec()],
        )
        .expect("included generated files")
        .into_iter()
        .enumerate()
        .map(|(position, generated)| (SourceId(position * 2 + 1), generated))
        .collect()
    }

    fn checked_sources(files: Vec<SourceFile>) -> CheckedTrees {
        let mut program = CheckedTrees::default();
        program.typed.symbols = program
            .typed
            .symbols
            .begin_extension(
                Some(Arc::new(source::SourceMap::from_files(files))),
                Vec::new(),
            )
            .finish();
        program
    }

    fn generated_files(
        custody: &[(SourceId, build_output::PackageGeneratedSource)],
    ) -> Vec<SourceFile> {
        custody
            .iter()
            .map(|(source_id, generated)| {
                let mut file = source(
                    &format!(
                        "/package/.omega/generated/{}",
                        String::from_utf8_lossy(generated.relative_path())
                    ),
                    "/package",
                    Some(PackageKeyIdentity::from_digest([7; 32]).expect("package identity")),
                    SourceOrigin::User,
                    std::str::from_utf8(generated.bytes()).expect("generated UTF-8"),
                );
                file.source_id = *source_id;
                file
            })
            .collect()
    }

    #[test]
    fn generated_custody_order_preserves_arbitrary_ids_and_first_duplicate() {
        let custody = generated_fixture();
        let ordered = GeneratedSourceOrder::new(&custody);
        assert!(
            ordered.positions.is_none(),
            "producer order needs no permutation"
        );
        assert!(GeneratedSourceOrder::new(&[]).positions.is_none());
        assert!(ordered.find(SourceId(0)).is_none());
        assert!(ordered.find(SourceId(2)).is_none());
        assert!(ordered.find(SourceId(usize::MAX)).is_none());
        let reordered = vec![
            custody[1].clone(),
            custody[0].clone(),
            (custody[1].0, custody[0].1.clone()),
        ];
        let ordered = GeneratedSourceOrder::new(&reordered);
        assert!(ordered.positions.is_some());
        assert_eq!(ordered.find(custody[1].0).unwrap().1, &custody[1].1);
    }

    #[test]
    fn generated_projection_preserves_order_independence_and_custody_errors() {
        let custody = generated_fixture();
        let files = generated_files(&custody);
        let baseline =
            derive_consumed_source_units(&checked_sources(files.clone()), &custody).unwrap();
        let mut reversed_files = files.clone();
        reversed_files.reverse();
        let reversed_custody = vec![custody[1].clone(), custody[0].clone()];
        assert_eq!(
            baseline,
            derive_consumed_source_units(&checked_sources(reversed_files), &reversed_custody)
                .unwrap()
        );
        let mut duplicate = custody.clone();
        duplicate.push(custody[0].clone());
        assert!(
            derive_consumed_source_units(&CheckedTrees::default(), &duplicate).unwrap_err()[0]
                .message
                .contains("duplicate frontend source IDs")
        );
        let mut missing = custody.clone();
        missing.push((SourceId(usize::MAX), custody[0].1.clone()));
        assert!(
            derive_consumed_source_units(&checked_sources(files.clone()), &missing).unwrap_err()[0]
                .message
                .contains("absent from the final checked closure")
        );
        let mut changed_content = files.clone();
        changed_content[0].source = Arc::from("changed");
        let mut changed_path = files.clone();
        changed_path[0].path = PathBuf::from("/package/.omega/generated/other.omg");
        let mut changed_root = files.clone();
        changed_root[0].package_root = PathBuf::from("/other-package");
        for (changed, expected) in [
            (changed_content, "does not match final checked source"),
            (changed_path, "path does not match final checked source"),
            (changed_root, "outside its reconciled package root"),
        ] {
            assert!(
                derive_consumed_source_units(&checked_sources(changed), &custody).unwrap_err()[0]
                    .message
                    .contains(expected)
            );
        }
        let mut repeated = files.clone();
        repeated.push(files[0].clone());
        assert!(
            derive_consumed_source_units(&checked_sources(repeated.clone()), &custody).unwrap_err()
                [0]
            .message
            .contains("duplicate canonical source coordinates")
        );
        assert!(
            derive_consumed_source_units(&checked_sources(repeated), &missing).unwrap_err()[0]
                .message
                .contains("absent from the final checked closure"),
            "repeated source IDs must not hide missing custody"
        );
        let mut wrong_origin = files;
        wrong_origin[0].origin = SourceOrigin::Toolchain;
        assert!(
            derive_consumed_source_units(&checked_sources(wrong_origin), &custody).unwrap_err()[0]
                .message
                .contains("does not match final checked source")
        );
    }

    #[test]
    fn generated_verification_preserves_physical_rereads_and_custody_only_behavior() {
        let custody = generated_fixture();
        let mut files = generated_files(&custody);
        // Verification is also callable alone: projection, not this reread,
        // owns duplicate/missing ID and generated logical-path admission.
        let mut unchecked = vec![custody[1].clone(), custody[0].clone()];
        unchecked.push((custody[0].0, custody[1].1.clone()));
        unchecked.push((SourceId(usize::MAX), custody[0].1.clone()));
        verify_current_files(&checked_sources(files.clone()), &unchecked).unwrap();
        files[0].source = Arc::from("drifted");
        assert!(
            verify_current_files(&checked_sources(files.clone()), &unchecked).unwrap_err()[0]
                .message
                .contains("drifted from staged-output custody")
        );
        files = generated_files(&custody);
        let path = std::env::temp_dir().join(format!(
            "omega-source-consumption-reread-{}.omg",
            std::process::id()
        ));
        std::fs::write(&path, "physical").expect("write physical source");
        let mut physical = source("unused", "unused", None, SourceOrigin::User, "physical");
        physical.path = path.clone();
        physical.source_id = SourceId(0);
        files.insert(1, physical);
        let program = checked_sources(files);
        verify_current_files(&program, &unchecked).unwrap();
        std::fs::write(&path, "changed").expect("change physical source");
        assert!(
            verify_current_files(&program, &unchecked).unwrap_err()[0]
                .message
                .contains("changed after frontend loading")
        );
        std::fs::remove_file(&path).expect("remove physical source");
        assert!(
            verify_current_files(&program, &unchecked).unwrap_err()[0]
                .message
                .contains("cannot be re-read")
        );
    }

    fn canonical_row_fixture() -> Vec<ConsumedSourceUnit> {
        let package = PackageKeyIdentity::from_digest([7; 32]).expect("package identity");
        [
            (
                ConsumedSourceUnitKind::PackageAuthored,
                Some(package),
                None,
                vec!["main.omg".to_owned()],
            ),
            (
                ConsumedSourceUnitKind::PackageGenerated,
                Some(package),
                None,
                vec!["generated".to_owned(), "λ.omg".to_owned()],
            ),
            (
                ConsumedSourceUnitKind::ToolchainVirtual,
                None,
                Some("std".to_owned()),
                vec!["<prelude>".to_owned()],
            ),
            (
                ConsumedSourceUnitKind::ToolchainOwned,
                None,
                Some("core".to_owned()),
                vec!["nested".to_owned(), "types.omg".to_owned()],
            ),
        ]
        .into_iter()
        .map(
            |(kind, package, toolchain_namespace, relative_path)| ConsumedSourceUnit {
                kind,
                package,
                toolchain_namespace,
                relative_path,
                byte_count: 123,
                content_digest: [13; 32],
            },
        )
        .collect()
    }

    #[test]
    fn canonical_row_layout_and_source_commitment_are_stable() {
        let units = canonical_row_fixture();
        let mut identities = units
            .iter()
            .map(|unit| format!("{:x}", Sha256::digest(canonical_consumed_unit_bytes(unit))))
            .collect::<Vec<_>>();
        let package = units[0].package().expect("authored owner");
        let inputs = super::super::PackageCompilationInputs::new_package(
            package,
            vec![super::super::PackageSourceBinding::new(
                package,
                "canonical-row-fixture",
                std::env::current_dir().expect("package root"),
            )],
            Vec::new(),
        )
        .expect("single package graph");
        let commitment = derive_source_consumption_commitment(&units, &inputs).expect("commitment");
        identities.push(
            commitment
                .digest()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        );
        // Captured from the V3 row/commitment encoder before changing its storage.
        assert_eq!(
            identities,
            [
                "ae0e65b19e6af4b93dce85e0feb096a5858db96308d6c7e60d99915732e3a213",
                "519ca0511d1018331db0e01bb39405e40d0660985e099f787918737492c15671",
                "eb986861f4e7b8b2018605c9436f9aabf7e535182dd8ea9f774d90cdba4a9285",
                "24b8039fc620e9d7d4a69da5af6c2a203ecabb369fcf263a8acb134cd1021141",
                "0085e87aac427a0e725966f478af42996e1714c151c8e93e10b9ad423278c6bd",
            ]
        );
    }

    #[test]
    fn canonical_source_rows_append_in_place_without_changing_framing() {
        let fixture = canonical_row_fixture();
        let package = fixture[0].package().expect("authored owner");
        for row_count in [0, 1, 4, 4096] {
            let mut units = (0..row_count)
                .map(|ordinal| {
                    let mut unit = fixture[ordinal % fixture.len()].clone();
                    unit.relative_path
                        .push(format!("{ordinal}-{}", "x".repeat(ordinal % 257)));
                    unit
                })
                .collect::<Vec<_>>();
            units.sort();
            // Independent outer framing preserves the former manifest protocol:
            // count, then each raw row's length and bytes, in canonical order.
            let mut expected = b"manifest-prefix".to_vec();
            expected.extend_from_slice(&(row_count as u64).to_le_bytes());
            for unit in &units {
                append_field(&mut expected, &canonical_consumed_unit_bytes(unit));
            }
            expected.extend_from_slice(b"manifest-suffix");
            let subject = PackageCompilationSubject {
                root: package,
                dependency_closure: super::super::PackageDependencyClosure::from_canonical_parts(
                    package,
                    super::super::BuildDeclarationKind::Package,
                    vec![package],
                    Vec::new(),
                )
                .expect("single package closure"),
                source_consumption_commitment: PackageSourceConsumptionCommitment::for_test(
                    [1; 32],
                ),
                consumed_units: units,
            };
            let mut actual = Vec::with_capacity(expected.len());
            actual.extend_from_slice(b"manifest-prefix");
            let storage = actual.as_ptr();
            subject.append_canonical_consumed_units(&mut actual);
            actual.extend_from_slice(b"manifest-suffix");
            assert_eq!(actual, expected, "{row_count} rows");
            assert_eq!(
                actual.as_ptr(),
                storage,
                "caller-supplied storage is retained"
            );
        }
    }

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
            resolution_stratum: source::SourceResolutionStratum::Base,
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
