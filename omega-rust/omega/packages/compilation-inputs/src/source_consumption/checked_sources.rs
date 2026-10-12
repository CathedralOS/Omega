//! Seal a compilation's source consumption and retain its generated-source handoff.

use super::{
    ConsumedSourceProjection, PackageCompilationSubject, PackageSourceConsumptionCommitment,
    derive_consumed_source_units, derive_package_compilation_subject, toolchain_source_identities,
};
use crate::{PackageCompilationInputs, PackageGeneratedSourceBundle};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;

/// Source custody spanning pre-build admission and the generated extension.
/// These are continuation inputs, not evidence of a final checked closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSourceCustody {
    source_extensions: Vec<crate::source_imports::GeneratedSourceLineage>,
    pub source_file_count: usize,
    pub build_execution_profile: Option<target::TargetProfile>,
    pub generated_source_custody: Vec<(source::SourceId, build_output::PackageGeneratedSource)>,
    pub base_source_consumption_commitment: Option<PackageSourceConsumptionCommitment>,
}

/// Exact source evidence sealed against one final checked program. The package
/// owner derives identities, checks live custody, and constructs dependency
/// handoffs; the compiler retains this result without assembling its fields.
#[derive(Debug, Clone)]
pub struct CheckedSourceCustody {
    build: BuildSourceCustody,
    package_purpose: Option<build_declarations::DependencyPurpose>,
    source_subject: Option<CheckedSourceSubject>,
    exact_toolchain_sources: Vec<(source::SourceId, [u8; 32])>,
    source_owner: Option<std::sync::Arc<source::SourceMap>>,
}

/// Package graph identity and captured-root source identity are distinct
/// issuers. Each carries the same original sealed source projection; a source
/// root does not acquire package or dependency authority through this sum.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CheckedSourceSubject {
    Package(PackageCompilationSubject),
    CapturedRoot(ConsumedSourceProjection),
}

/// Original declaration coordinates in the already sealed source roster.
/// A valid source range identifies bytes, not a semantic kind, checked body,
/// executed Build root or admission. Those associations stay with the caller.
#[derive(Debug, Clone, Copy)]
pub struct ConsumedDeclarationSource<'custody> {
    unit: &'custody super::ConsumedSourceUnit,
    start: u64,
    end: u64,
}

impl<'custody> ConsumedDeclarationSource<'custody> {
    pub const fn unit(self) -> &'custody super::ConsumedSourceUnit {
        self.unit
    }

    pub const fn start(self) -> u64 {
        self.start
    }

    pub const fn end(self) -> u64 {
        self.end
    }
}

// Owner pointer joins stay local; canonical source equality keeps its original
// fields and does not acquire host paths or pointer addresses as identity.
impl PartialEq for CheckedSourceCustody {
    fn eq(&self, other: &Self) -> bool {
        self.build == other.build
            && self.package_purpose == other.package_purpose
            && self.source_subject == other.source_subject
            && self.exact_toolchain_sources == other.exact_toolchain_sources
    }
}
impl Eq for CheckedSourceCustody {}

impl BuildSourceCustody {
    /// Original frontend metadata starts with no generated continuation. Only
    /// admitted generated-source production can append a lineage association.
    pub fn new(
        source_file_count: usize,
        build_execution_profile: Option<target::TargetProfile>,
        generated_source_custody: Vec<(source::SourceId, build_output::PackageGeneratedSource)>,
        base_source_consumption_commitment: Option<PackageSourceConsumptionCommitment>,
    ) -> Self {
        Self {
            source_extensions: Vec::new(),
            source_file_count,
            build_execution_profile,
            generated_source_custody,
            base_source_consumption_commitment,
        }
    }

    /// Seal a completed Build's generated handoff against its retained typed
    /// continuation. This establishes source and staged-output custody only;
    /// it does not claim the dependency's product has passed final checking.
    /// Consumers still check these bytes in their own product or Build scope.
    pub fn package_generated_source_bundle(
        &self,
        program: &typed_trees::TypedTrees,
        inputs: &PackageCompilationInputs,
        target: target::TargetProfile,
        own_generated_sources: Vec<build_output::PackageGeneratedSource>,
    ) -> Result<PackageGeneratedSourceBundle, Vec<Diagnostic>> {
        let subject = derive_package_compilation_subject(
            program.symbols.source_files(),
            inputs,
            &self.generated_source_custody,
        )?;
        Ok(PackageGeneratedSourceBundle::from_source_consumption(
            subject.root(),
            inputs.compilation_purpose(),
            target,
            self.build_execution_profile,
            subject.dependency_closure().clone(),
            subject.source_consumption_commitment(),
            own_generated_sources,
        ))
    }

    /// Admit this activation's generated sources and retain their exact source
    /// coordinates before parsing. Empty output needs no package context.
    /// Rejection leaves the prior count and custody unchanged; admission alone
    /// does not seal a checked source closure.
    pub fn admit_generated_extension<'inputs>(
        &mut self,
        base_sources: &std::sync::Arc<source::SourceMap>,
        package_inputs: Option<&'inputs PackageCompilationInputs>,
        generated_sources: &[build_output::PackageGeneratedSource],
    ) -> Result<
        Option<(
            crate::source_imports::GeneratedSourceExtension,
            &'inputs PackageCompilationInputs,
        )>,
        Vec<Diagnostic>,
    > {
        if generated_sources.is_empty() {
            return Ok(None);
        }
        let package_inputs = package_inputs.ok_or_else(|| {
            vec![Diagnostic::error(
                "generated-source final compilation requires package-aware source custody",
            )]
        })?;
        let package_root = package_inputs
            .package_root(package_inputs.root())
            .expect("validated package inputs retain their root package");
        let extension = crate::source_imports::admit_generated_extension(
            base_sources,
            package_root,
            Some(package_inputs.root()),
            generated_sources,
        )?;
        let lineage = extension.lineage();
        if self
            .source_extensions
            .last()
            .is_some_and(|previous| !previous.continues_to(base_sources))
        {
            return Err(vec![Diagnostic::error(
                "generated-source admission lost its exact preceding source frontier",
            )]);
        }
        let source_file_count = self
            .source_file_count
            .checked_add(extension.custody().len())
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "generated package source count exceeds the compiler range",
                )]
            })?;
        self.generated_source_custody
            .extend(extension.custody().iter().cloned());
        self.source_file_count = source_file_count;
        self.source_extensions.push(lineage);
        Ok(Some((extension, package_inputs)))
    }

    /// Preserve the source-admission order: seal the original source subject,
    /// then identify toolchain sources. Neither step reopens the host: the
    /// frontend read each consumed file once and this seal commits to those
    /// bytes, so there is nothing here for a second read to establish.
    pub fn seal(
        self,
        program: &CheckedTrees,
        package_inputs: Option<&PackageCompilationInputs>,
    ) -> Result<CheckedSourceCustody, Vec<Diagnostic>> {
        if !self.source_extensions.is_empty() {
            let mut continuation = program.symbols.retained_source_map().ok_or_else(|| {
                vec![Diagnostic::error(
                    "generated-source seal lost its final immutable source owner",
                )]
            })?;
            for extension in self.source_extensions.iter().rev() {
                if !extension.continues_to(continuation) {
                    return Err(vec![Diagnostic::error(
                        "generated-source seal does not consume its produced source continuation",
                    )]);
                }
                continuation = extension.base();
            }
        }
        let source_subject = match package_inputs {
            Some(inputs) => Some(CheckedSourceSubject::Package(
                derive_package_compilation_subject(
                    program.symbols.source_files(),
                    inputs,
                    &self.generated_source_custody,
                )?,
            )),
            None => match program.symbols.retained_source_map().and_then(|sources| {
                Some((
                    sources.retained_source_root()?,
                    sources.retained_source_root_revision()?,
                ))
            }) {
                Some(captured_root) => {
                    let (units, coordinates) = derive_consumed_source_units(
                        program.symbols.source_files(),
                        &self.generated_source_custody,
                        Some(captured_root),
                    )?;
                    Some(CheckedSourceSubject::CapturedRoot(
                        ConsumedSourceProjection::from_rows(units, coordinates),
                    ))
                }
                None => None,
            },
        };
        let exact_toolchain_sources = toolchain_source_identities(program.symbols.source_files())?;
        Ok(CheckedSourceCustody {
            build: self,
            package_purpose: package_inputs.map(PackageCompilationInputs::compilation_purpose),
            source_subject,
            exact_toolchain_sources,
            source_owner: program.symbols.retained_source_map().cloned(),
        })
    }
}

impl CheckedSourceCustody {
    pub const fn source_file_count(&self) -> usize {
        self.build.source_file_count
    }

    pub const fn package_subject(&self) -> Option<&PackageCompilationSubject> {
        match self.source_subject.as_ref() {
            Some(CheckedSourceSubject::Package(subject)) => Some(subject),
            Some(CheckedSourceSubject::CapturedRoot(_)) | None => None,
        }
    }

    /// Reconciled root package identity of a package-aware compilation.
    /// Standalone compilation has no package identity.
    pub const fn package_identity(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
        match self.package_subject() {
            Some(subject) => Some(subject.root()),
            None => None,
        }
    }

    /// The actual package compilation occurrence, independently of each
    /// consumed file's Product or Build import scope. Captured roots have no
    /// package occurrence and do not acquire one through source ownership.
    pub const fn package_purpose(&self) -> Option<build_declarations::DependencyPurpose> {
        self.package_purpose
    }

    /// Exact source-path-free dependency closure consumed by a package-aware
    /// compilation. Standalone compilation has no package closure.
    pub const fn dependency_closure(&self) -> Option<&crate::PackageDependencyClosure> {
        match self.package_subject() {
            Some(subject) => Some(subject.dependency_closure()),
            None => None,
        }
    }

    /// Canonical commitment to the exact source bytes a package-aware
    /// compilation consumed. Standalone compilation has none.
    pub const fn source_consumption_commitment(
        &self,
    ) -> Option<PackageSourceConsumptionCommitment> {
        match self.package_subject() {
            Some(subject) => Some(subject.source_consumption_commitment()),
            None => None,
        }
    }

    pub const fn base_source_consumption_commitment(
        &self,
    ) -> Option<PackageSourceConsumptionCommitment> {
        self.build.base_source_consumption_commitment
    }

    /// Custody join against the original immutable frontend owner, including
    /// clones retained before Product pruning. This is not a source recheck.
    pub fn owns_sources(&self, symbols: &symbols::SymbolTable) -> bool {
        match (self.source_owner.as_ref(), symbols.retained_source_map()) {
            (Some(owner), Some(candidate)) => std::sync::Arc::ptr_eq(owner, candidate),
            _ => false,
        }
    }

    /// Join a retained declaration symbol to its original consumed source.
    /// Product pruning may remove its machine row while preserving immutable
    /// symbol provenance. An original Build owner may precede the producer's
    /// admitted generated-source continuation; that exact lineage is retained
    /// independently. This query neither searches by name nor rereads bytes.
    pub fn declaration_source(
        &self,
        symbols: &symbols::SymbolTable,
        symbol: symbols::SymbolHandle,
    ) -> Option<ConsumedDeclarationSource<'_>> {
        if !symbol.is_valid() || !self.owns_original_sources(symbols) {
            return None;
        }
        let span = symbols.symbol_provenance_source_span(symbol)?;
        let file = symbols.source_file(span)?;
        let unit = self.consumed_unit_for_source(span.source_id)?;
        if span.span.start >= span.span.end
            || span.span.end > file.source.len()
            || !file.source.is_char_boundary(span.span.start)
            || !file.source.is_char_boundary(span.span.end)
            || unit.package() != file.package_identity
        {
            return None;
        }
        Some(ConsumedDeclarationSource {
            unit,
            start: u64::try_from(span.span.start).ok()?,
            end: u64::try_from(span.span.end).ok()?,
        })
    }

    /// Join an original executed owner through only the generated-source
    /// continuations this seal consumed. Direct ownership remains strict;
    /// equal source bytes or matching source-id prefixes provide no authority.
    pub fn owns_original_sources(&self, symbols: &symbols::SymbolTable) -> bool {
        let (Some(final_owner), Some(original)) =
            (self.source_owner.as_ref(), symbols.retained_source_map())
        else {
            return false;
        };
        if std::sync::Arc::ptr_eq(final_owner, original) {
            return true;
        }
        let mut continuation = final_owner;
        for extension in self.build.source_extensions.iter().rev() {
            if !extension.continues_to(continuation) {
                return false;
            }
            if std::sync::Arc::ptr_eq(extension.base(), original) {
                return true;
            }
            continuation = extension.base();
        }
        false
    }

    /// Original seal-owned source association for either exact package custody
    /// or an original captured consuming root. Input without either issuer
    /// stays unavailable; source identity does not grant runtime authority.
    pub fn consumed_unit_for_source(
        &self,
        source_id: source::SourceId,
    ) -> Option<&super::ConsumedSourceUnit> {
        match self.source_subject.as_ref()? {
            CheckedSourceSubject::Package(subject) => subject.consumed_unit_for_source(source_id),
            CheckedSourceSubject::CapturedRoot(projection) => {
                projection.consumed_unit_for_source(source_id)
            }
        }
    }

    pub fn exact_toolchain_sources(&self) -> &[(source::SourceId, [u8; 32])] {
        &self.exact_toolchain_sources
    }

    /// Export only the activation's own included sources. The final checked
    /// closure also contains dependency-generated sources; those establish the
    /// subject but must not be re-exported as this package's generated output.
    /// The caller supplies the evaluated Build's own source roster and selected
    /// product target, which remain distinct from its build execution profile.
    pub fn package_generated_source_bundle(
        &self,
        selected_target: Option<target::TargetProfile>,
        own_generated_sources: Vec<build_output::PackageGeneratedSource>,
    ) -> Result<PackageGeneratedSourceBundle, &'static str> {
        let subject = self
            .package_subject()
            .ok_or("generated-source bundles require package-aware compilation")?;
        let target =
            selected_target.ok_or("generated-source bundles require one selected target")?;
        Ok(PackageGeneratedSourceBundle::from_checked(
            subject.root(),
            self.package_purpose
                .ok_or("generated-source bundles require a checked package purpose")?,
            target,
            self.build.build_execution_profile,
            subject.dependency_closure().clone(),
            subject.source_consumption_commitment(),
            own_generated_sources,
        ))
    }
}
