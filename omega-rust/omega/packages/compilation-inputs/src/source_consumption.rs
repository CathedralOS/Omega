use diagnostics::Diagnostic;
use source::{SourceFile, SourceOrigin};

mod checked_sources;
pub use checked_sources::{BuildSourceCustody, CheckedSourceCustody};

const SOURCE_CONSUMPTION_DOMAIN: &[u8] = b"OMEGA-PACKAGE-SOURCE-CONSUMPTION-V4\0";

use source::source_identity::append_canonical_consumed_unit;
pub use source::source_identity::{ConsumedSourceUnit, ConsumedSourceUnitKind};

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
}

impl PackageSourceConsumptionCommitment {
    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub const fn for_test(digest: [u8; 32]) -> Self {
        Self { digest }
    }

    /// Rebuild a commitment from its retained digest. The digest was bound
    /// when the rows were derived; recovery replays the same value.
    #[doc(hidden)]
    pub const fn from_retained_digest(digest: [u8; 32]) -> Self {
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

    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(SOURCE_CONSUMPTION_DOMAIN);
    digest.raw(&inputs.root().digest());
    digest.byte(canonical_root_role(inputs.root_role()));
    let packages = inputs
        .packages()
        .map(|(identity, _)| identity)
        .collect::<Vec<_>>();
    digest.u64(u64::try_from(packages.len()).expect("compiler package count fits u64"));
    for package in packages {
        digest.raw(&package.digest());
    }
    let dependencies = inputs.dependencies().collect::<Vec<_>>();
    digest.u64(u64::try_from(dependencies.len()).expect("compiler dependency count fits u64"));
    for (requester, alias, target) in dependencies {
        digest.raw(&requester.digest());
        digest.bytes(alias.as_bytes());
        digest.raw(&target.digest());
    }
    digest.u64(u64::try_from(consumed_units.len()).expect("loaded source count fits u64"));
    let mut row_bytes = Vec::new();
    for unit in consumed_units {
        row_bytes.clear();
        append_canonical_consumed_unit(&mut row_bytes, unit);
        digest.bytes(&row_bytes);
    }
    Ok(PackageSourceConsumptionCommitment {
        digest: digest.finish(),
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

/// Derive the canonical projection of the frontend's exact retained sources.
/// Generated-source custody is joined by private `SourceId` only while this
/// projection is constructed; IDs never enter the retained rows.
/// This establishes source identity, not semantic validity. Both pre-build
/// admission and final checked publication use the same projection.
#[doc(hidden)]
pub(crate) fn derive_consumed_source_units<'source>(
    sources: impl IntoIterator<Item = &'source SourceFile>,
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
    let mut units = sources
        .into_iter()
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
    let logical_path = source::source_identity::canonical_path_components(relative, &source.path)
        .map_err(|reason| vec![Diagnostic::error(reason)])?
        .join("/");
    let expected_path = [b".omega/generated/".as_slice(), generated.relative_path()].concat();
    if logical_path.as_bytes() != expected_path {
        return Err(vec![Diagnostic::error(format!(
            "generated-source custody path does not match final checked source `{}`",
            source.path.display()
        ))]);
    }
    Ok(())
}

/// Bind retained source identity to the reconciled package/dependency closure.
/// Semantic checking and authority admission remain the caller's obligations.
#[doc(hidden)]
pub fn derive_package_compilation_subject<'source>(
    sources: impl IntoIterator<Item = &'source SourceFile>,
    inputs: &super::PackageCompilationInputs,
    generated_sources: &[(source::SourceId, build_output::PackageGeneratedSource)],
) -> Result<PackageCompilationSubject, Vec<Diagnostic>> {
    let consumed_units = derive_consumed_source_units(sources, generated_sources)?;
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
    source::source_identity::consumed_source_unit(source, generated)
        .map_err(|reason| vec![Diagnostic::error(reason)])
}

fn same_source_coordinate(left: &ConsumedSourceUnit, right: &ConsumedSourceUnit) -> bool {
    left.kind() == right.kind()
        && left.package() == right.package()
        && left.toolchain_namespace() == right.toolchain_namespace()
        && left.relative_path() == right.relative_path()
}

/// Exact retained toolchain identities, shared with early Psi production.
pub fn toolchain_source_identities<'source>(
    sources: impl IntoIterator<Item = &'source SourceFile>,
) -> Result<Vec<(source::SourceId, [u8; 32])>, Vec<Diagnostic>> {
    source::source_identity::toolchain_source_identities(sources)
        .map_err(|reason| vec![Diagnostic::error(reason)])
}

#[doc(hidden)]
pub fn toolchain_source_identity_digest(source: &SourceFile) -> Result<[u8; 32], Vec<Diagnostic>> {
    source::source_identity::toolchain_source_identity_digest(source)
        .map_err(|reason| vec![Diagnostic::error(reason)])
}
