//! Toolchain-authored Build declarations consumed as ordinary Omega source.
//!
//! These declarations supply static shapes only. Evaluation admits exact
//! toolchain provenance and activation-local values before granting any access.
//! Optimization cases and counters are projected from the shared catalog.

mod optimizations;

/// Construct the target-neutral Build API with the exact optimization catalog.
/// Loading and parsing this source do not grant build-host authority.
pub fn build_prelude_source() -> String {
    optimizations::install(BUILD_PRELUDE)
}

const BUILD_PRELUDE: &str = r#"
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
// (wiki/spec/build/behavior_exclusions.md). `Build::exclude_crash(builder, case)`
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
pub data X86DeploymentFeatures {
    case Baseline;
    case AvxFma3;
}
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
// Build-owned ELF loader selection
// (wiki/spec/build/configuration.md#elf-executable-loader-selection): the
// exact `PT_INTERP` pathname one Linux target's dynamically linked output asks
// its runtime environment to load, one row per canonical target name. Empty
// keeps the selected target environment's declared default. A selection never
// requests dynamic linking; static output emits no `PT_INTERP`.
pub data ElfInterpreterSelection {
    linux_arm64: &[u8];
    linux_x86_64: &[u8];
}
pub data Build {
    x86_deployment_features: X86DeploymentFeatures;
    subsystem: Subsystem;
    freestanding: bool;
    // Authored application identifier (wiki/spec/build/macos_application.md):
    // supplies the GUI CodeDirectory signing identity and CFBundleIdentifier.
    // Empty means no authored identifier; console output falls back to the
    // validated executable leaf. It is validated separately from the
    // application name and is never inferred from the PE subsystem word.
    identifier: &[u8];
    elf_interpreter: ElfInterpreterSelection;
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
pub machine Build::depend(builder: &mut Build, source: Source) {
}
pub machine Build::depend_as(builder: &mut Build, alias: &[u8], source: Source) {
}
pub machine Build::build_depend(builder: &mut Build, source: Source) {
}
pub machine Build::build_depend_as(builder: &mut Build, alias: &[u8], source: Source) {
}
pub machine Build::package(builder: &mut Build, name: &[u8]) {
}
pub machine Build::application(builder: &mut Build, name: &[u8]) {
}
pub machine Build::member(builder: &mut Build, path: &[u8]) {
}
// Artifact-only application modifier: a declaration statement harvested
// statically; the declared body is the evaluator no-op.
pub machine Build::artifact_only(builder: &mut Build) {
}
// Behavior exclusion (wiki/spec/build/behavior_exclusions.md): a
// product-admission requirement that the selected executable composition
// contains no reachable site of the named crash cause. The selection is
// recorded when this call executes against the original Build value;
// the evaluator intercepts the declaration, and a
// declaration that permits Trap cannot override it.
pub machine Build::exclude_crash(builder: &mut Build, cause: CrashCause) {
}
// Native realization checks this executed selection using the shared
// physical mechanism classifier, independently of receiver permissions.
pub machine Build::exclude_physical_authority(builder: &mut Build, authority: PhysicalAuthorityClass) {
}
// Independent-component assumption acceptance
// (wiki/spec/build/component_publication.md): a verified component
// description binds its environment-mediating mechanisms — an immediate
// port-space write, a declared physical mechanism — by digest. The
// consuming build accepts one exact digest per call, authored as the same
// lowercase hex spelling verification diagnostics report. The declaration
// is harvested statically with its authored span; the declared body is the
// evaluator no-op.
pub machine Build::accept_component_assumption(builder: &mut Build, digest: &[u8]) {
}
pub machine BuildSource::resolve(source: &BuildSource, relative: &[u8]) -> BuildPath {
    BuildPath {}
}
// Missing slots fail the activation; the declared body is never executed.
pub machine BuildInputs::get(inputs: &BuildInputs, name: &[u8]) -> BuildSource {
    BuildSource {}
}
pub machine BuildOutput::resolve(output: &BuildOutput, relative: &[u8]) -> BuildPath {
    BuildPath {}
}
pub machine BuildSource::open(source: &BuildSource, path: BuildPath, flags: i32) -> i32 {
    0
}
pub machine BuildSource::read(source: &BuildSource, descriptor: i32, buffer: &mut [u8], count: u64) -> i64 {
    0
}
pub machine BuildSource::close(source: &BuildSource, descriptor: i32) -> i32 {
    0
}
pub machine BuildOutput::create(output: &mut BuildOutput, path: BuildPath, mode: i32) -> i32 {
    0
}
pub machine BuildOutput::write(output: &mut BuildOutput, descriptor: i32, bytes: &[u8]) -> i64 {
    0
}
pub machine BuildOutput::close(output: &mut BuildOutput, descriptor: i32) -> i32 {
    0
}
pub machine BuildOutput::include_source(output: &mut BuildOutput, generated: BuildPath) {
}
// Required-output obligation declaration: reserve the canonical output name
// and return its obligation marker. Duplicate or colliding names reject in
// the evaluator; this declared body never executes.
pub machine BuildOutput::require(output: &mut BuildOutput, name: &[u8]) -> RequiredOutput {
    RequiredOutput {}
}
// Settle one obligation against its sealed staged file. The evaluator
// intercepts the call: a sealed name yields `OutputCompletion::Sealed` with
// the receipt, an unsealed file yields `Retry` carrying the obligation and
// file custody back, and every other custody violation is a hard error.
pub machine BuildOutput::complete(output: &mut BuildOutput, obligation: RequiredOutput, file: BuildPath) -> OutputCompletion {
    OutputCompletion::Retry { obligation: obligation, file: file }
}
// Consume an obligation with an authored diagnostic. The failure is sticky:
// the activation can never publish a successful product set.
pub machine BuildOutput::fail(output: &mut BuildOutput, obligation: RequiredOutput, diagnostic: &[u8]) {
}
// The obligation's declared canonical output name.
pub machine RequiredOutput::path(output: &RequiredOutput) -> &[u8] {
    ""
}
// The described product declaration's canonical path. The evaluator answers
// from its private description table; the declared body never executes.
pub machine ProductTypeSchema::path(schema: &ProductTypeSchema) -> &[u8] {
    ""
}
// The described provider declaration's canonical path. The evaluator answers
// from its private description table; the declared body never executes.
pub machine ProductProviderRef::path(provider: &ProductProviderRef) -> &[u8] {
    ""
}
pub machine BuildLog::write_line(log: &mut BuildLog, text: &[u8]) {
}
// Fallible logical query: select the exact product machine `path` declares in
// this call occurrence's own package for root slot `slot`. The evaluator
// intercepts the call and returns the restricted description marker; this
// declared body is only a statically well-typed stand-in.
pub machine BuildProduct::entry(product: &BuildProduct, path: &[u8], slot: &[u8]) -> ProductEntryRef {
    ProductEntryRef {}
}
// Fallible logical query: select the exact product data declaration `path`
// names in this call occurrence's own package. The evaluator intercepts the
// call and returns the restricted schema-description marker; this declared
// body is only a statically well-typed stand-in.
pub machine BuildProduct::schema(product: &BuildProduct, path: &[u8]) -> ProductTypeSchema {
    ProductTypeSchema {}
}
// Fallible logical query: select the exact product provider declaration `path`
// names in this call occurrence's own package. A provider declaration is a
// nominal data type owning at least one `satisfies` machine; other data does
// not qualify. The evaluator intercepts the call and returns the restricted
// provider-description marker; this declared body is only a statically
// well-typed stand-in.
pub machine BuildProduct::provider(product: &BuildProduct, path: &[u8]) -> ProductProviderRef {
    ProductProviderRef {}
}
// compiler-owned optimization enable machine
// compiler-owned optimization report machine
"#;
