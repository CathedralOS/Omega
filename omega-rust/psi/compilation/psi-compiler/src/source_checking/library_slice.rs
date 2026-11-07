//! Name the library part of one checking input, and count equal names.
//!
//! A root loads only the library modules its `use` items reach, transitively
//! (wiki/spec/packages/acceptance.md, "Ordinary compilation"). That import-closed
//! set of toolchain and dependency sources is the root's *library slice*. Library
//! bodies cannot name a root symbol, and a callable contract is never inferred
//! from "the current consumer set" (wiki/spec/language/generics.md), so a
//! slice's own judgment depends on its source bytes, its
//! dependency scope and the Build-admitted inputs that act on library code:
//! the accepted process-exit completion, fixed-layout target scopes, opaque
//! representation selections on slice declarations, asm authority and wire
//! demands. Root bodies, generic instances at root types, and whole-program
//! coherence stay per root.
//!
//! [`LibrarySliceKey`] digests exactly that input. Every component is portable:
//! package identities, package-relative paths, content digests and declaration
//! paths, never an arena handle or a hash over one. The key omits the compiler:
//! one process runs one compiler, and these counters live in one process. A
//! persistent record would add `compiler_input_identity`.
//!
//! Nothing here skips, replays or supplies a judgment. The counters measure how
//! many slice judgments a run computes and how many of them were equal inputs,
//! which is the reuse a later step could take.

use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceFile, SourceOrigin};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// Who owns the program's root, so its sources stay out of the slice.
/// `None` is a standalone compilation: every user-origin source is root-owned.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LibrarySliceRequest {
    pub root_package: Option<PackageKeyIdentity>,
}

/// Portable identity of one library slice's checking input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LibrarySliceKey([u8; 32]);

/// Slice judgments this process computed, and how many distinct keys they had.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LibrarySliceTotals {
    pub distinct: u64,
    pub computed: u64,
}

static COMPUTED: AtomicU64 = AtomicU64::new(0);
static DISTINCT: Mutex<Vec<LibrarySliceKey>> = Mutex::new(Vec::new());

pub fn library_slice_totals() -> LibrarySliceTotals {
    let distinct = DISTINCT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .len();
    LibrarySliceTotals {
        distinct: distinct as u64,
        computed: COMPUTED.load(Ordering::Relaxed),
    }
}

/// Count one freshly computed check whose program has a nonempty slice.
pub(super) fn record_computed(key: LibrarySliceKey) {
    COMPUTED.fetch_add(1, Ordering::Relaxed);
    let mut distinct = DISTINCT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Err(position) = distinct.binary_search(&key) {
        distinct.insert(position, key);
    }
}

/// The Build-admitted inputs that can act on library code, read before the
/// check mutates the program.
pub(super) struct SliceAdmission<'input> {
    pub completion: &'input validation::source_completion::SourceCompletionAdmission<'input>,
    pub layout_scopes: &'input [build_time_evaluation::FixedLayoutTargetScope],
    pub representations: &'input [representation_selections::OpaqueRepresentationSelection],
    pub asm_authority: Option<validation::AsmAuthorityAdmission>,
    pub wire_demands: &'input [language_semantics::wire::WireCompatibilityDemand],
}

impl LibrarySliceRequest {
    /// The slice key of `program` under `admission`, or `None` when the
    /// program loads no library source.
    pub(super) fn key(
        &self,
        program: &typed_trees::TypedTrees,
        admission: &SliceAdmission<'_>,
    ) -> Option<LibrarySliceKey> {
        let sources = program.symbols.source_map()?;
        let mut slice: Vec<SliceSource> = sources
            .selected_files()
            .filter(|file| self.in_slice(file))
            .map(SliceSource::of)
            .collect();
        if slice.is_empty() {
            return None;
        }
        slice.sort();
        let mut digest =
            optimization::digest_encoder::DomainDigestEncoder::new(b"omega-library-slice-v1\0");
        digest.len(slice.len());
        for source in &slice {
            source.encode(&mut digest);
        }
        let in_slice = |symbol: symbols::SymbolHandle| {
            program
                .symbols
                .symbol_provenance_source_span(symbol)
                .and_then(|span| program.symbols.source_file(span))
                .is_some_and(|file| self.in_slice(file))
        };
        encode_completion(&mut digest, admission.completion);
        // Scopes are nonempty only when the program has a fixed-layout
        // application. Their entries select which bodies each target's work
        // reaches, so root entries join the key as well: conservative until
        // entry independence of library layout evaluation is shown.
        digest.len(admission.layout_scopes.len());
        for scope in admission.layout_scopes {
            digest.bytes(scope.target.as_bytes());
            match &scope.facts {
                Some(facts) => {
                    digest.byte(1);
                    digest.bytes(facts.profile.as_bytes());
                    digest.u64(facts.pointer_size);
                    digest.u64(facts.pointer_alignment);
                    digest.u64(facts.case_tag_size);
                    digest.u64(facts.case_tag_alignment);
                    digest.byte(u8::from(facts.little_endian));
                }
                None => digest.byte(0),
            }
            match &scope.entry_declarations {
                Some(entries) => {
                    digest.byte(1);
                    let mut paths: Vec<String> = entries
                        .iter()
                        .map(|entry| portable_symbol(program, *entry))
                        .collect();
                    paths.sort();
                    digest.len(paths.len());
                    for path in &paths {
                        digest.bytes(path.as_bytes());
                    }
                }
                None => digest.byte(0),
            }
            let mut providers: Vec<Vec<u8>> = scope
                .provider_declarations
                .iter()
                .map(portable_provider_selection)
                .collect();
            providers.sort();
            digest.len(providers.len());
            for provider in &providers {
                digest.bytes(provider);
            }
        }
        // A selection on a root type cannot reach a library body.
        let mut representations: Vec<([u8; 32], String)> = admission
            .representations
            .iter()
            .filter(|selection| in_slice(selection.opaque()))
            .map(|selection| {
                (
                    selection.rederived_selected_application_commitment(),
                    portable_symbol(program, selection.opaque()),
                )
            })
            .collect();
        representations.sort();
        digest.len(representations.len());
        for (commitment, opaque) in &representations {
            digest.raw(commitment);
            digest.bytes(opaque.as_bytes());
        }
        match admission.asm_authority {
            Some(authority) => {
                digest.byte(1);
                digest.bytes(format!("{authority:?}").as_bytes());
            }
            None => digest.byte(0),
        }
        let mut wire: Vec<String> = admission
            .wire_demands
            .iter()
            .map(|demand| format!("{demand:?}"))
            .collect();
        wire.sort();
        digest.len(wire.len());
        for demand in &wire {
            digest.bytes(demand.as_bytes());
        }
        Some(LibrarySliceKey(digest.finish()))
    }

    /// Toolchain sources and sources of every package except the root.
    fn in_slice(&self, file: &SourceFile) -> bool {
        file.origin == SourceOrigin::Toolchain
            || file
                .package_identity
                .is_some_and(|package| Some(package) != self.root_package)
    }
}

/// One slice source by package, package-relative path, scope and bytes.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct SliceSource {
    package: [u8; 32],
    path: String,
    scope: u8,
    stratum: u8,
    content: [u8; 32],
}

impl SliceSource {
    fn of(file: &SourceFile) -> Self {
        let relative = file
            .path
            .strip_prefix(&file.package_root)
            .unwrap_or(&file.path);
        let mut content =
            optimization::digest_encoder::DomainDigestEncoder::new(b"omega-library-source-v1\0");
        content.bytes(file.source.as_bytes());
        Self {
            package: file
                .package_identity
                .map(PackageKeyIdentity::digest)
                .unwrap_or([0; 32]),
            path: relative.to_string_lossy().replace('\\', "/"),
            scope: file.dependency_scope as u8,
            stratum: file.resolution_stratum as u8,
            content: content.finish(),
        }
    }

    fn encode(&self, digest: &mut optimization::digest_encoder::DomainDigestEncoder) {
        digest.raw(&self.package);
        digest.bytes(self.path.as_bytes());
        digest.byte(self.scope);
        digest.byte(self.stratum);
        digest.raw(&self.content);
    }
}

fn encode_completion(
    digest: &mut optimization::digest_encoder::DomainDigestEncoder,
    completion: &validation::source_completion::SourceCompletionAdmission<'_>,
) {
    use validation::source_completion::SourceCompletionAdmission;
    match completion {
        SourceCompletionAdmission::None => digest.byte(0),
        SourceCompletionAdmission::AcceptedProcessExit {
            package,
            declaration_path,
        } => {
            digest.byte(1);
            digest.raw(&package.digest());
            digest.bytes(declaration_path.as_bytes());
        }
        SourceCompletionAdmission::DiscoverProcessExit => digest.byte(2),
    }
}

/// A declaration by its owning package and display path, never its handle.
fn portable_symbol(program: &typed_trees::TypedTrees, symbol: symbols::SymbolHandle) -> String {
    let owner = program
        .symbols
        .symbol_provenance_source_span(symbol)
        .and_then(|span| program.symbols.source_file(span))
        .map(|file| match file.package_identity {
            Some(package) => hex(&package.digest()),
            None => format!("{:?}", file.origin),
        })
        .unwrap_or_default();
    format!("{owner}::{}", program.symbols.display_path(symbol, "::"))
}

/// A provider selection by its portable declaration identities. The Build
/// origin is provenance of the row, not an input to checking library bodies.
fn portable_provider_selection(
    selection: &build_time_evaluation::provider_declarations::ProviderSelection,
) -> Vec<u8> {
    use build_time_evaluation::provider_declarations::{
        ProviderSelectionIdentity, ProviderSelectionSubject,
    };
    fn identity(text: &mut String, identity: &ProviderSelectionIdentity) {
        let package = identity
            .package
            .map(|package| hex(&package.digest()))
            .unwrap_or_default();
        text.push_str(&format!(
            "{package}|{}|{:?}|{}\n",
            identity.canonical_path, identity.application_identity, identity.applied
        ));
    }
    let mut text = String::new();
    text.push_str(
        selection
            .target
            .map(|target| target.canonical_name())
            .unwrap_or("portable"),
    );
    text.push('\n');
    match &selection.subject {
        ProviderSelectionSubject::BoundaryTrait(subject) => {
            text.push_str("trait\n");
            identity(&mut text, subject);
        }
        ProviderSelectionSubject::BoundaryRequirement(subject) => {
            text.push_str("requirement\n");
            identity(&mut text, subject);
        }
        ProviderSelectionSubject::BoundaryOperatorFamily(family) => {
            let package = family
                .package
                .map(|package| hex(&package.digest()))
                .unwrap_or_default();
            text.push_str(&format!("family\n{package}|{}\n", family.canonical_path));
            for coordinate in family.coordinates() {
                text.push_str(&format!(
                    "{}|{}\n",
                    coordinate.requirement_identity, coordinate.static_parameter_count
                ));
            }
        }
    }
    identity(&mut text, &selection.provider_type);
    text.push_str(&format!("{:?}", selection.composition_mode));
    text.into_bytes()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::{LibrarySliceRequest, SliceSource};
    use semantic_vocabulary::PackageKeyIdentity;
    use source::{DependencyScope, SourceFile, SourceId, SourceOrigin, SourceResolutionStratum};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn file(
        root: &str,
        path: &str,
        origin: SourceOrigin,
        package: Option<PackageKeyIdentity>,
        text: &str,
    ) -> SourceFile {
        SourceFile {
            source_id: SourceId(0),
            path: PathBuf::from(root).join(path),
            package_root: PathBuf::from(root),
            package_identity: package,
            dependency_scope: DependencyScope::Product,
            origin,
            resolution_stratum: SourceResolutionStratum::Base,
            source: Arc::from(text),
        }
    }

    #[test]
    fn the_slice_is_every_source_except_the_root_package() {
        let root = PackageKeyIdentity::from_digest([1; 32]);
        let dependency = PackageKeyIdentity::from_digest([2; 32]);
        let request = LibrarySliceRequest { root_package: root };
        let core = file("/core", "option.omg", SourceOrigin::Toolchain, None, "");
        let std = file("/std", "console.omg", SourceOrigin::User, dependency, "");
        let main = file("/app", "main.omg", SourceOrigin::User, root, "");
        assert!(request.in_slice(&core));
        assert!(request.in_slice(&std));
        assert!(!request.in_slice(&main));
        let standalone = LibrarySliceRequest::default();
        let loose = file("/app", "main.omg", SourceOrigin::User, None, "");
        assert!(standalone.in_slice(&core));
        assert!(!standalone.in_slice(&loose));
    }

    #[test]
    fn a_slice_source_is_named_by_package_relative_path_and_bytes() {
        let package = PackageKeyIdentity::from_digest([2; 32]);
        let here = SliceSource::of(&file("/a/std", "x.omg", SourceOrigin::User, package, "m"));
        let moved = SliceSource::of(&file("/b/std", "x.omg", SourceOrigin::User, package, "m"));
        let edited = SliceSource::of(&file("/a/std", "x.omg", SourceOrigin::User, package, "n"));
        assert!(here == moved);
        assert!(here != edited);
    }
}
