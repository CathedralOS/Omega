//! Generated `build` prelude content. The toolchain-provided build vocabulary
//! (`BUILD_PRELUDE` plus its compiler-owned slots) and the injected
//! build-machine fragment live here: literal generated source content owned
//! beside `build_vocabulary`, not sequencing in the assembly entry file.

use super::build_vocabulary;
use crate::frontend::{extend_source_storage, lex_sources, parse_sources};
use crate::source::SourceStorage;
use artifacts::compile_timings::CompileTimings;
use artifacts::compile_timings::{SOURCE_FILES_TO_TOKENS, TOKENS_TO_SYNTAX_TREES};
use diagnostics::Diagnostic;

/// Toolchain-provided build vocabulary (wiki/spec/build/declarations.md): a
/// build.omg has exactly one free `machine build(builder: &mut Build) { ... }`
/// entry. The `Build` / `Subsystem` types are CORE-DEFINED, never authored per
/// file. When a build.omg root declares that build-machine shape and no `Build` data of
/// its own, the build-machine fragment is injected as a virtual source (a
/// program-declared `Build` wins, which keeps migration and deliberate
/// overrides possible). Package identity uses the same injected `Build`
/// surface through `builder.package(name)`; there is no second declaration
/// type or compiler-only constant shape.
pub(super) const BUILD_PRELUDE: &str = r#"
// Toolchain-provided build vocabulary.
pub data Subsystem {
    case Console;
    case Gui;
    case EfiApplication;
    case Unspecified(value: u16);
}
// Compiler-owned composition choice for one exact provider selection. The
// omitted `select_provider` argument means Fused; Independent is never
// inferred from provider source.
pub data CompositionMode [copy] {
    case Fused;
    case Independent;
}
// Compiler-owned crash-cause vocabulary for build behavior exclusions
// (wiki/spec/build/behavior_exclusions.md). `builder.exclude_crash(case)`
// selects a product-admission requirement over the exact selected executable
// composition; it never masks effects on ordinary callable contracts.
pub data CrashCause [copy] {
    case Trap;
    case Abort;
}
// The closed physical-authority vocabulary selected by Build exclusions.
// These are admission requirements, not grants to use the named authority.
pub data PhysicalAuthorityClass [copy] {
    case FilesystemContentRead;
    case FilesystemContentWrite;
    case FilesystemMetadataQuery;
    case DirectoryEnumeration;
    case FilesystemNamespaceMutation;
    case FilesystemMetadataMutation;
    case ProcessOutput;
    case ProcessTermination;
    case MachineControl;
    case PortIo;
    case InterruptControl;
    case InterruptEntry;
    case RootMemoryAccess;
    case ProcessInput;
}
// compiler-owned TargetProfile declaration
// compiler-owned X86DeploymentFeatures declaration
// compiler-owned optimization declarations
// Compiler-owned package-build path carrier. The evaluator replaces values
// produced by `resolve` with activation-local rooted authority; an authored
// `BuildPath {}` has the right static shape but no usable runtime root.
pub data BuildPath [copy] {
}
pub data BuildSource {
}
// Caller-captured immutable slots. Only the admitted Build value carries the
// runtime slot roster; constructing this empty shape grants no input access.
pub data BuildInputs {
}
pub data BuildOutput {
}
pub data BuildLog {
}
// Compiler-owned product-selection facet. `builder.product.entry`,
// `builder.product.provider`, and `builder.product.schema` are the only routes
// to compiler-issued product descriptions: the evaluator answers each query
// by lexical lookup under the call occurrence's package and returns an opaque
// marker; the declared bodies are never executed.
pub data BuildProduct {
}
// A restricted, non-callable product-entry description. Authored code can
// retain, copy, and hand it to helpers, but only a `roots.bind` delegated
// operand can consume it, and only the compiler-issued marker value carries
// the exact selected identity.
pub data ProductEntryRef [copy] {
}
// A restricted, non-callable product-provider description (the
// `ProductProviderRef` of wiki/spec/build/scoped_execution.md). Authored
// code can retain, copy, and hand it to helpers, and `provider.path()`
// inspects the declaration it describes, but only the compiler-issued
// marker value carries the exact selected identity.
pub data ProductProviderRef [copy] {
}
// A restricted, non-callable product-type-schema description (the
// `ProductTypeSchema` of wiki/spec/build/scoped_execution.md). Authored
// code can retain, copy, and hand it to helpers, and `schema.path()`
// inspects the declaration it describes, but only the compiler-issued
// marker value carries the exact selected identity.
pub data ProductTypeSchema [copy] {
}
// Compiler-owned required-output obligation marker. `builder.output.require`
// is the only route to one: the evaluator issues an opaque marker whose row
// lives in its private obligation table; an authored `RequiredOutput {}` has
// the same static shape but carries no obligation. Obligations are linear at
// the settlement boundary: `complete` or `fail` consumes them exactly once.
pub data RequiredOutput {
}
// Compiler-owned output-completion receipt. `builder.output.complete` issues
// it when the obligation's sealed file is accepted; evaluated code may
// retain or copy it, but only the compiler-issued marker names a completed
// output.
pub data OutputReceipt [copy] {
}
// Result of `builder.output.complete`. `Sealed` carries the compiler-issued
// completion receipt; `Retry` returns the obligation and file custody when
// the named output was not yet sealed, so an explicit retry stays possible.
pub data OutputCompletion {
    case Sealed(receipt: OutputReceipt);
    case Retry(obligation: RequiredOutput, file: BuildPath);
}
// Optional proof-carrying product requests (wiki/spec/proofs/publication.md).
// Both flags are independent and default to false; they request adjacent
// `.proof` sidecars, never a different pipeline or weaker checking.
pub data Pcc {
    psi: bool;
    native: bool;
}
// Authored granular privileged-service admission evidence
// (wiki/spec/build/permissions.md#privileged-services): each flag admits
// exactly one mediated privileged class for the produced image's assembly
// authority discharge — `port_io` admits port I/O, `interrupt_table` admits
// interrupt-table publication. They compose independently; machine-owner
// authority has no granular grant and stays `freestanding`-only, and
// `freestanding = true` already covers both classes.
pub data PrivilegedServices {
    port_io: bool;
    interrupt_table: bool;
}
pub data Build {
    // compiler-owned Build.target field
    // compiler-owned Build.x86_deployment_features field
    subsystem: Subsystem;
    freestanding: bool;
    // Authored application identifier (wiki/spec/build/macos_application.md):
    // supplies the GUI CodeDirectory signing identity and CFBundleIdentifier.
    // Empty means no authored identifier; console output falls back to the
    // validated executable leaf. It is validated separately from the
    // application name and is never inferred from the PE subsystem word.
    identifier: &[u8];
    optimizations: Optimizations;
    pcc: Pcc;
    privileged_services: PrivilegedServices;
    source: BuildSource;
    inputs: BuildInputs;
    output: BuildOutput;
    log: BuildLog;
    product: BuildProduct;
}
pub data PackageSelection {
    case Root;
    case Named(package: &[u8]);
}
pub data Source {
    case Path(location: &[u8]);
    case Git(repository: &[u8], revision: &[u8], selection: PackageSelection);
}
pub machine Build::depend(&mut self, source: Source) {
}
pub machine Build::depend_as(&mut self, alias: &[u8], source: Source) {
}
pub machine Build::build_depend(&mut self, source: Source) {
}
pub machine Build::build_depend_as(&mut self, alias: &[u8], source: Source) {
}
pub machine Build::package(&mut self, name: &[u8]) {
}
pub machine Build::application(&mut self, name: &[u8]) {
}
pub machine Build::member(&mut self, path: &[u8]) {
}
// Artifact-only application modifier: a declaration statement harvested
// statically; the declared body is the evaluator no-op.
pub machine Build::artifact_only(&mut self) {
}
// Behavior exclusion (wiki/spec/build/behavior_exclusions.md): a
// product-admission requirement that the selected executable composition
// contains no reachable site of the named crash cause. The selection is
// recorded when this call executes against the original Build value;
// the evaluator intercepts the declaration, and a
// declaration that permits Trap cannot override it.
pub machine Build::exclude_crash(&mut self, cause: CrashCause) {
}
// Native realization checks this executed selection using the shared
// physical mechanism classifier, independently of receiver permissions.
pub machine Build::exclude_physical_authority(&mut self, authority: PhysicalAuthorityClass) {
}
// Independent-component assumption acceptance
// (wiki/spec/build/component_publication.md): a verified component
// description binds its environment-mediating mechanisms — an immediate
// port-space write, a declared physical mechanism — by digest. The
// consuming build accepts one exact digest per call, authored as the same
// lowercase hex spelling verification diagnostics report. The declaration
// is harvested statically with its authored span; the declared body is the
// evaluator no-op.
pub machine Build::accept_component_assumption(&mut self, digest: &[u8]) {
}
pub machine BuildSource::resolve(&self, relative: &[u8]) -> BuildPath {
    BuildPath {}
}
// Missing slots fail the activation; the declared body is never executed.
pub machine BuildInputs::get(&self, name: &[u8]) -> BuildSource {
    BuildSource {}
}
pub machine BuildOutput::resolve(&self, relative: &[u8]) -> BuildPath {
    BuildPath {}
}
pub machine BuildSource::open(&self, path: BuildPath, flags: i32) -> i32 {
    0
}
pub machine BuildSource::read(&self, descriptor: i32, buffer: &mut [u8], count: u64) -> i64 {
    0
}
pub machine BuildSource::close(&self, descriptor: i32) -> i32 {
    0
}
pub machine BuildOutput::create(&mut self, path: BuildPath, mode: i32) -> i32 {
    0
}
pub machine BuildOutput::write(&mut self, descriptor: i32, bytes: &[u8]) -> i64 {
    0
}
pub machine BuildOutput::close(&mut self, descriptor: i32) -> i32 {
    0
}
pub machine BuildOutput::include_source(&mut self, generated: BuildPath) {
}
// Required-output obligation declaration: reserve the canonical output name
// and return its obligation marker. Duplicate or colliding names reject in
// the evaluator; this declared body never executes.
pub machine BuildOutput::require(&mut self, name: &[u8]) -> RequiredOutput {
    RequiredOutput {}
}
// Settle one obligation against its sealed staged file. The evaluator
// intercepts the call: a sealed name yields `OutputCompletion::Sealed` with
// the receipt, an unsealed file yields `Retry` carrying the obligation and
// file custody back, and every other custody violation is a hard error.
pub machine BuildOutput::complete(&mut self, obligation: RequiredOutput, file: BuildPath) -> OutputCompletion {
    OutputCompletion::Retry { obligation: obligation, file: file }
}
// Consume an obligation with an authored diagnostic. The failure is sticky:
// the activation can never publish a successful product set.
pub machine BuildOutput::fail(&mut self, obligation: RequiredOutput, diagnostic: &[u8]) {
}
// The obligation's declared canonical output name.
pub machine RequiredOutput::path(&self) -> &[u8] {
    ""
}
// The described product declaration's canonical path. The evaluator answers
// from its private description table; the declared body never executes.
pub machine ProductTypeSchema::path(&self) -> &[u8] {
    ""
}
// The described provider declaration's canonical path. The evaluator answers
// from its private description table; the declared body never executes.
pub machine ProductProviderRef::path(&self) -> &[u8] {
    ""
}
pub machine BuildLog::write_line(&mut self, text: &[u8]) {
}
// Fallible logical query: select the exact product machine `path` declares in
// this call occurrence's own package for root slot `slot`. The evaluator
// intercepts the call and returns the restricted description marker; this
// declared body is only a statically well-typed stand-in.
pub machine BuildProduct::entry(&self, path: &[u8], slot: &[u8]) -> ProductEntryRef {
    ProductEntryRef {}
}
// Fallible logical query: select the exact product data declaration `path`
// names in this call occurrence's own package. The evaluator intercepts the
// call and returns the restricted schema-description marker; this declared
// body is only a statically well-typed stand-in.
pub machine BuildProduct::schema(&self, path: &[u8]) -> ProductTypeSchema {
    ProductTypeSchema {}
}
// Fallible logical query: select the exact product provider declaration `path`
// names in this call occurrence's own package. A provider declaration is a
// nominal data type owning at least one `satisfies` machine; other data does
// not qualify. The evaluator intercepts the call and returns the restricted
// provider-description marker; this declared body is only a statically
// well-typed stand-in.
pub machine BuildProduct::provider(&self, path: &[u8]) -> ProductProviderRef {
    ProductProviderRef {}
}
// compiler-owned optimization enable machine
// compiler-owned optimization report machine
"#;

const BUILD_TARGET_FIELD_SLOT: &str = "    // compiler-owned Build.target field\n";
const BUILD_TARGET_FIELD: &str = "    target: TargetProfile;\n";
const BUILD_X86_DEPLOYMENT_FEATURES_FIELD_SLOT: &str =
    "    // compiler-owned Build.x86_deployment_features field\n";
const BUILD_X86_DEPLOYMENT_FEATURES_FIELD: &str =
    "    x86_deployment_features: X86DeploymentFeatures;\n";
const BUILD_TARGET_PROFILE_SLOT: &str = "// compiler-owned TargetProfile declaration\n";
const BUILD_TARGET_PROFILE: &str = r#"pub data TargetProfile {
    case LinuxArm64;
    case LinuxX86_64;
    case MacosArm64;
    case WindowsX86_64;
    case UefiX86_64;
    case CrossPlatformCli;
    case LocalUnchecked;
}
"#;
const BUILD_X86_DEPLOYMENT_FEATURES_SLOT: &str =
    "// compiler-owned X86DeploymentFeatures declaration\n";
const BUILD_X86_DEPLOYMENT_FEATURES: &str = r#"pub data X86DeploymentFeatures {
    case Baseline;
    case AvxFma3;
}
"#;

pub(super) fn construct_build_prelude(base: &str, has_exact_target: bool) -> String {
    assert_eq!(
        base.matches(BUILD_TARGET_FIELD_SLOT).count(),
        1,
        "build prelude must contain exactly one compiler-owned target slot"
    );
    assert_eq!(
        base.matches(BUILD_TARGET_PROFILE_SLOT).count(),
        1,
        "build prelude must contain exactly one compiler-owned target profile slot"
    );
    assert_eq!(
        base.matches(BUILD_X86_DEPLOYMENT_FEATURES_FIELD_SLOT)
            .count(),
        1,
        "build prelude must contain exactly one compiler-owned x86 deployment-feature field slot"
    );
    assert_eq!(
        base.matches(BUILD_X86_DEPLOYMENT_FEATURES_SLOT).count(),
        1,
        "build prelude must contain exactly one compiler-owned x86 deployment-feature declaration slot"
    );
    let replacement = if has_exact_target {
        BUILD_TARGET_FIELD
    } else {
        ""
    };
    let with_target = base
        .replacen(BUILD_TARGET_PROFILE_SLOT, BUILD_TARGET_PROFILE, 1)
        .replacen(
            BUILD_X86_DEPLOYMENT_FEATURES_SLOT,
            BUILD_X86_DEPLOYMENT_FEATURES,
            1,
        )
        .replacen(BUILD_TARGET_FIELD_SLOT, replacement, 1)
        .replacen(
            BUILD_X86_DEPLOYMENT_FEATURES_FIELD_SLOT,
            if has_exact_target {
                BUILD_X86_DEPLOYMENT_FEATURES_FIELD
            } else {
                ""
            },
            1,
        );
    build_vocabulary::install(&with_target)
}

pub(super) fn inject_build_prelude(
    source_storage: &mut SourceStorage,
    build_source_id: Option<source::SourceId>,
    has_exact_target: bool,
    timings: &mut CompileTimings,
) -> Result<Vec<symbols::SourceScopedTopLevelBinding>, Vec<Diagnostic>> {
    let mut has_build_machine = false;
    let mut build_source_declares_build_data = false;
    let mut program_declares_build_data = false;
    for (_, file) in source_storage.files.iter() {
        let is_build_file = Some(file.source_id) == build_source_id;
        for root_item in &file.root_items {
            match source_storage.syntax_trees.root_item(*root_item) {
                syntax_trees::item::Item::Machine(machine)
                    if is_build_file && machine.name.as_str() == "build" =>
                {
                    has_build_machine = true;
                }
                syntax_trees::item::Item::Data(data) if data.name.as_str() == "Build" => {
                    if is_build_file {
                        build_source_declares_build_data = true;
                    } else {
                        program_declares_build_data = true;
                    }
                }
                _ => {}
            }
        }
    }
    let inject_build_vocabulary = has_build_machine && !build_source_declares_build_data;
    if !inject_build_vocabulary {
        return Ok(Vec::new());
    }

    // Targetless checking is not an artifact activation and therefore exposes
    // no synthetic target. Exact-target requests receive the canonical field;
    // retaining the former Build shape here keeps targetless semantic checks
    // honest while product requests migrate to immutable activation.
    let prelude = construct_build_prelude(BUILD_PRELUDE, has_exact_target);

    let first_source_id = source_storage.next_source_id();
    let lexed = timings.record(SOURCE_FILES_TO_TOKENS, || {
        let sources =
            crate::frontend::load_injected_source("<build-prelude>", &prelude, first_source_id);
        lex_sources(sources)
    })?;
    let parsed = timings.record(TOKENS_TO_SYNTAX_TREES, || {
        parse_sources(lexed, &mut source_storage.syntax_trees)
    })?;
    extend_source_storage(source_storage, parsed)?;
    let bindings = match (program_declares_build_data, build_source_id) {
        (true, Some(build_source_id)) => vec![symbols::SourceScopedTopLevelBinding::new(
            build_source_id,
            source::SourceId(first_source_id),
            "Build",
        )],
        _ => Vec::new(),
    };
    Ok(bindings)
}
