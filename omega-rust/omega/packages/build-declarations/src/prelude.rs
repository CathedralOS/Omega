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
public data Subsystem {
    case Console;
    case Gui;
    case EfiApplication;
    case Unspecified(value: u16);
}
// Compiler-owned composition choice for one exact provider selection. The
// omitted `select_provider` argument means Fused; Independent is never
// inferred from provider source.
public data CompositionMode [copy] {
    case Fused;
    case Independent;
}
// Compiler-owned crash-cause vocabulary for build behavior exclusions
// (wiki/spec/build/behavior_exclusions.md). `Build::exclude_crash(builder, case)`
// selects a product-admission requirement over the exact selected executable
// composition; it never masks effects on ordinary callable contracts.
public data CrashCause [copy] {
    case Trap;
    case Abort;
}
// The closed physical-authority vocabulary selected by Build exclusions.
// These are admission requirements, not grants to use the named authority.
public data PhysicalAuthorityClass [copy] {
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
public data X86DeploymentFeatures {
    case Baseline;
    case AvxFma3;
}
// compiler-owned optimization declarations
// Compiler-owned package-build path carrier. The evaluator replaces values
// produced by `resolve` with activation-local rooted authority; an authored
// `BuildPath {}` has the right static shape but no usable runtime root.
public data BuildPath [copy] {
}
public data BuildSource {
}
// Caller-captured immutable slots. Only the admitted Build value carries the
// runtime slot roster; constructing this empty shape grants no input access.
public data BuildInputs {
}
public data BuildOutput {
}
public data BuildLog {
}
// Compiler-owned product-selection facet. `builder.product.entry`,
// `builder.product.provider`, and `builder.product.schema` are the only routes
// to compiler-issued product descriptions: the evaluator answers each query
// by lexical lookup under the call occurrence's package and returns an opaque
// marker; the declared bodies are never executed.
public data BuildProduct {
}
// A restricted, non-callable product-entry description. Authored code can
// retain, copy, and hand it to helpers, but only a `Build::bind` delegated
// operand can consume it, and only the compiler-issued marker value carries
// the exact selected identity.
public data ProductEntryRef [copy] {
}
// Execution protocol for designated Product bindings and selections. Typing supplies
// the exact product-selection occurrence beside the ordinary boundary call.
// Calling this requirement directly supplies no selection or host authority.
// These are compiler host operations, not a user-selected service binding.
// Their activation-local ticket is interpreted only at admitted call sites.
public boundary trait BuildProductOperations {
    machine prepare(builder: &Build) -> u64 reaches;
    machine bind(activation: u64) reaches;
    machine select(activation: u64) reaches;
    machine select_mode(activation: u64, mode: CompositionMode) reaches;
    machine bind_description(activation: u64, description: ProductEntryRef) reaches;
}
// A restricted, non-callable product-provider description (the
// `ProductProviderRef` of wiki/spec/build/scoped_execution.md). Authored
// code can retain, copy, and hand it to helpers, and `provider.path()`
// inspects the declaration it describes, but only the compiler-issued
// marker value carries the exact selected identity.
public data ProductProviderRef [copy] {
}
// A restricted, non-callable product-type-schema description (the
// `ProductTypeSchema` of wiki/spec/build/scoped_execution.md). Authored
// code can retain, copy, and hand it to helpers, and `schema.path()`
// inspects the declaration it describes, but only the compiler-issued
// marker value carries the exact selected identity.
public data ProductTypeSchema [copy] {
}
// Compiler-owned required-output obligation marker. `builder.output.require`
// is the only route to one: the evaluator issues an opaque marker whose row
// lives in its private obligation table; an authored `RequiredOutput {}` has
// the same static shape but carries no obligation. Obligations are linear at
// the settlement boundary: `complete` or `fail` consumes them exactly once.
public data RequiredOutput {
}
// Compiler-owned output-completion receipt. `builder.output.complete` issues
// it when the obligation's sealed file is accepted; evaluated code may
// retain or copy it, but only the compiler-issued marker names a completed
// output.
public data OutputReceipt [copy] {
}
// Result of `builder.output.complete`. `Sealed` carries the compiler-issued
// completion receipt; `Retry` returns the obligation and file custody when
// the named output was not yet sealed, so an explicit retry stays possible.
public data OutputCompletion {
    case Sealed(receipt: OutputReceipt);
    case Retry(obligation: RequiredOutput, file: BuildPath);
}
// Authored granular privileged-service admission evidence
// (wiki/spec/build/permissions.md#privileged-services): each flag admits
// exactly one mediated privileged class for the produced image's assembly
// authority discharge — `port_io` admits port I/O, `interrupt_table` admits
// interrupt-table publication. They compose independently; machine-owner
// authority has no granular grant and stays `freestanding`-only, and
// `freestanding = true` already covers both classes.
public data PrivilegedServices {
    port_io: bool;
    interrupt_table: bool;
}
// Build-owned ELF loader selection
// (wiki/spec/build/configuration.md#elf-executable-loader-selection): the
// exact `PT_INTERP` pathname one Linux target's dynamically linked output asks
// its runtime environment to load, one row per canonical target name. Empty
// keeps the selected target environment's declared default. A selection never
// requests dynamic linking; static output emits no `PT_INTERP`.
public data ElfInterpreterSelection<'storage> {
    linux_arm64: &'storage [u8];
    linux_x86_64: &'storage [u8];
}
// Stored views share an explicit backing lifetime. Opaque mutable boundaries
// must promise output safety; an unnamed relation cannot imply preservation.
public data Build<'storage> {
    x86_deployment_features: X86DeploymentFeatures;
    subsystem: Subsystem;
    freestanding: bool;
    // Authored application identifier (wiki/spec/build/macos_application.md):
    // supplies the GUI CodeDirectory signing identity and CFBundleIdentifier.
    // Empty means no authored identifier; console output falls back to the
    // validated executable leaf. It is validated separately from the
    // application name and is never inferred from the PE subsystem word.
    identifier: &'storage [u8];
    elf_interpreter: ElfInterpreterSelection<'storage>;
    optimizations: Optimizations;
    privileged_services: PrivilegedServices;
    source: BuildSource;
    inputs: BuildInputs;
    output: BuildOutput;
    log: BuildLog;
    product: BuildProduct;
}
public data PackageSelection {
    case Root;
    case Named(package: &[u8]);
}
public data Source {
    case Path(location: &[u8]);
    case Git(repository: &[u8], revision: &[u8], selection: PackageSelection);
}
// Executed project requests cross one compiler-issued host protocol. Source
// variants are decomposed by ordinary Omega code, not a host syntax visitor.
public boundary trait BuildRequestOperations {
    machine package(builder: &Build, name: &[u8]) reaches;
    machine application(builder: &Build, name: &[u8]) reaches;
    machine member(builder: &Build, path: &[u8]) reaches;
    machine artifact_only(builder: &Build) reaches;
    machine depend_path(builder: &Build, build_scope: bool, explicit_alias: bool, alias: &[u8], location: &[u8]) reaches;
    machine depend_git(builder: &Build, build_scope: bool, explicit_alias: bool, alias: &[u8], repository: &[u8], revision: &[u8], named_package: bool, package: &[u8]) reaches;
    machine write_log(log: &BuildLog, text: &[u8]) reaches;
}
public machine Build::depend(builder: &mut Build, source: Source) reaches BuildRequestOperations invokes BuildRequestOperations; {
    Build::request_dependency(builder, false, false, "", source);
}
public machine Build::depend_as(builder: &mut Build, alias: &[u8], source: Source) reaches BuildRequestOperations invokes BuildRequestOperations; {
    Build::request_dependency(builder, false, true, alias, source);
}
public machine Build::build_depend(builder: &mut Build, source: Source) reaches BuildRequestOperations invokes BuildRequestOperations; {
    Build::request_dependency(builder, true, false, "", source);
}
public machine Build::build_depend_as(builder: &mut Build, alias: &[u8], source: Source) reaches BuildRequestOperations invokes BuildRequestOperations; {
    Build::request_dependency(builder, true, true, alias, source);
}
machine Build::request_dependency(builder: &mut Build, build_scope: bool, explicit_alias: bool, alias: &[u8], source: Source) reaches BuildRequestOperations {
    transition source {
        Source::Path { location } -> Build::request_path(builder, build_scope, explicit_alias, alias, location)
        Source::Git { repository, revision, selection } -> Build::request_git(builder, build_scope, explicit_alias, alias, repository, revision, selection)
    }
}
machine Build::request_path(builder: &mut Build, build_scope: bool, explicit_alias: bool, alias: &[u8], location: &[u8]) reaches BuildRequestOperations {
    BuildRequestOperations::depend_path(builder, build_scope, explicit_alias, alias, location);
}
machine Build::request_git(builder: &mut Build, build_scope: bool, explicit_alias: bool, alias: &[u8], repository: &[u8], revision: &[u8], selection: PackageSelection) reaches BuildRequestOperations {
    transition selection {
        PackageSelection::Root -> Build::request_selected_git(builder, build_scope, explicit_alias, alias, repository, revision, false, "")
        PackageSelection::Named { package } -> Build::request_selected_git(builder, build_scope, explicit_alias, alias, repository, revision, true, package)
    }
}
machine Build::request_selected_git(builder: &mut Build, build_scope: bool, explicit_alias: bool, alias: &[u8], repository: &[u8], revision: &[u8], named_package: bool, package: &[u8]) reaches BuildRequestOperations {
    BuildRequestOperations::depend_git(builder, build_scope, explicit_alias, alias, repository, revision, named_package, package);
}
public machine Build::package(builder: &mut Build, name: &[u8]) reaches BuildRequestOperations invokes BuildRequestOperations; {
    BuildRequestOperations::package(builder, name);
}
public machine Build::application(builder: &mut Build, name: &[u8]) reaches BuildRequestOperations invokes BuildRequestOperations; {
    BuildRequestOperations::application(builder, name);
}
public machine Build::member(builder: &mut Build, path: &[u8]) reaches BuildRequestOperations invokes BuildRequestOperations; {
    BuildRequestOperations::member(builder, path);
}
// The executed request is joined with the application role and required outputs.
public machine Build::artifact_only(builder: &mut Build) reaches BuildRequestOperations invokes BuildRequestOperations; {
    BuildRequestOperations::artifact_only(builder);
}
// Behavior exclusion (wiki/spec/build/behavior_exclusions.md): a
// product-admission requirement that the selected executable composition
// contains no reachable site of the named crash cause. The selection is
// recorded when this call executes against the original Build value;
// the ordinary boundary records its evaluated case, and a
// declaration that permits Trap cannot override it.
public machine Build::exclude_crash(builder: &mut Build, cause: CrashCause) reaches BuildBehaviorOperations invokes BuildBehaviorOperations; {
    BuildBehaviorOperations::exclude_crash(builder, cause);
}
// Native realization checks this executed selection using the shared
// physical mechanism classifier, independently of receiver permissions.
public machine Build::exclude_physical_authority(builder: &mut Build, authority: PhysicalAuthorityClass) reaches BuildBehaviorOperations invokes BuildBehaviorOperations; {
    BuildBehaviorOperations::exclude_physical_authority(builder, authority);
}
// Internal recording observes identity; it never replaces Build storage.
// Source admission permits these calls only in the exact mutable public
// helpers above. A user-authored direct call carries no host authority.
public boundary trait BuildBehaviorOperations {
    machine exclude_crash(builder: &Build, cause: CrashCause) reaches;
    machine exclude_physical_authority(builder: &Build, authority: PhysicalAuthorityClass) reaches;
}
// Independent-component assumption acceptance
// (wiki/spec/build/component_publication.md): a verified component
// description binds its environment-mediating mechanisms — an immediate
// port-space write, a declared physical mechanism — by digest. The
// consuming build accepts one exact digest per call, authored as the same
// lowercase hex spelling verification diagnostics report. The declaration
// is harvested statically with its authored span; the declared body is the
// evaluator no-op.
public machine Build::accept_component_assumption(builder: &mut Build, digest: &[u8]) {
}
// Ordinary boundary calls carry evaluated operands. Installation requires exact
// toolchain provenance and the activation's original facets or issued markers.
public boundary trait BuildFilesystemOperations {
    machine resolve_source(source: &BuildSource, relative: &[u8]) -> BuildPath reaches;
    machine resolve_output(output: &BuildOutput, relative: &[u8]) -> BuildPath reaches;
    machine named_input(inputs: &BuildInputs, name: &[u8]) -> BuildSource reaches;
    machine open(source: &BuildSource, path: BuildPath, flags: i32) -> i32 reaches;
    machine read(source: &BuildSource, descriptor: i32, buffer: &mut [u8], count: u64) -> i64 reaches;
    machine close_source(source: &BuildSource, descriptor: i32) -> i32 reaches;
    machine create(output: &mut BuildOutput, path: BuildPath, mode: i32) -> i32 reaches;
    machine write(output: &mut BuildOutput, descriptor: i32, bytes: &[u8]) -> i64 reaches;
    machine close_output(output: &mut BuildOutput, descriptor: i32) -> i32 reaches;
    machine include_source(output: &mut BuildOutput, generated: BuildPath) reaches;
    machine require(output: &mut BuildOutput, name: &[u8]) -> RequiredOutput reaches;
    machine can_complete(output: &BuildOutput, obligation: &RequiredOutput, file: BuildPath) -> bool reaches;
    machine complete(output: &mut BuildOutput, obligation: RequiredOutput, file: BuildPath) -> OutputReceipt reaches;
    machine fail(output: &mut BuildOutput, obligation: RequiredOutput, diagnostic: &[u8]) reaches;
    machine required_path(output: &RequiredOutput) -> &[u8] reaches;
}
public machine BuildSource::resolve(source: &BuildSource, relative: &[u8]) -> BuildPath reaches BuildFilesystemOperations invokes BuildFilesystemOperations; {
    BuildFilesystemOperations::resolve_source(source, relative)
}
public machine BuildOutput::resolve(output: &BuildOutput, relative: &[u8]) -> BuildPath reaches BuildFilesystemOperations invokes BuildFilesystemOperations; {
    BuildFilesystemOperations::resolve_output(output, relative)
}
public machine BuildInputs::get(inputs: &BuildInputs, name: &[u8]) -> BuildSource reaches BuildFilesystemOperations invokes BuildFilesystemOperations; {
    BuildFilesystemOperations::named_input(inputs, name)
}
public machine BuildSource::open(source: &BuildSource, path: BuildPath, flags: i32) -> i32 reaches BuildFilesystemOperations invokes BuildFilesystemOperations; {
    BuildFilesystemOperations::open(source, path, flags)
}
public machine BuildSource::read(source: &BuildSource, descriptor: i32, buffer: &mut [u8], count: u64) -> i64 reaches BuildFilesystemOperations invokes BuildFilesystemOperations; {
    BuildFilesystemOperations::read(source, descriptor, buffer, count)
}
public machine BuildSource::close(source: &BuildSource, descriptor: i32) -> i32 reaches BuildFilesystemOperations invokes BuildFilesystemOperations; {
    BuildFilesystemOperations::close_source(source, descriptor)
}
public machine BuildOutput::create(output: &mut BuildOutput, path: BuildPath, mode: i32) -> i32 reaches BuildFilesystemOperations invokes BuildFilesystemOperations; {
    BuildFilesystemOperations::create(output, path, mode)
}
public machine BuildOutput::write(output: &mut BuildOutput, descriptor: i32, bytes: &[u8]) -> i64 reaches BuildFilesystemOperations invokes BuildFilesystemOperations; {
    BuildFilesystemOperations::write(output, descriptor, bytes)
}
public machine BuildOutput::close(output: &mut BuildOutput, descriptor: i32) -> i32 reaches BuildFilesystemOperations invokes BuildFilesystemOperations; {
    BuildFilesystemOperations::close_output(output, descriptor)
}
public machine BuildOutput::include_source(output: &mut BuildOutput, generated: BuildPath) reaches BuildFilesystemOperations invokes BuildFilesystemOperations; {
    BuildFilesystemOperations::include_source(output, generated);
}
public machine BuildOutput::require(output: &mut BuildOutput, name: &[u8]) -> RequiredOutput reaches BuildFilesystemOperations invokes BuildFilesystemOperations; {
    BuildFilesystemOperations::require(output, name)
}
public machine BuildOutput::fail(output: &mut BuildOutput, obligation: RequiredOutput, diagnostic: &[u8]) reaches BuildFilesystemOperations invokes BuildFilesystemOperations; {
    BuildFilesystemOperations::fail(output, obligation, diagnostic);
}
public machine RequiredOutput::path(output: &RequiredOutput) -> &[u8] reaches BuildFilesystemOperations invokes BuildFilesystemOperations; {
    BuildFilesystemOperations::required_path(output)
}
// The ordinary Omega branch retains both owned inputs on Retry. The host
// rechecks the same sealed file and pending obligation when issuing a receipt.
public machine BuildOutput::complete(output: &mut BuildOutput, obligation: RequiredOutput, file: BuildPath) -> OutputCompletion reaches BuildFilesystemOperations invokes BuildFilesystemOperations; {
    let ready: bool = BuildFilesystemOperations::can_complete(output, &obligation, file);
    transition ready {
        true -> BuildOutput::seal(output, obligation, file)
        false -> BuildOutput::retry(obligation, file)
    }
}
machine BuildOutput::seal(output: &mut BuildOutput, obligation: RequiredOutput, file: BuildPath) -> OutputCompletion reaches BuildFilesystemOperations {
    let receipt: OutputReceipt = BuildFilesystemOperations::complete(output, obligation, file);
    OutputCompletion::Sealed { receipt: receipt }
}
machine BuildOutput::retry(obligation: RequiredOutput, file: BuildPath) -> OutputCompletion {
    OutputCompletion::Retry { obligation: obligation, file: file }
}
// The described product declaration's canonical path. The evaluator answers
// from its private description table; the declared body never executes.
public machine ProductTypeSchema::path(schema: &ProductTypeSchema) -> &[u8] {
    ""
}
// The described provider declaration's canonical path. The evaluator answers
// from its private description table; the declared body never executes.
public machine ProductProviderRef::path(provider: &ProductProviderRef) -> &[u8] {
    ""
}
public machine BuildLog::write_line(log: &mut BuildLog, text: &[u8]) reaches BuildRequestOperations invokes BuildRequestOperations; {
    BuildRequestOperations::write_log(log, text);
}
// Fallible logical query: select the exact product machine `path` declares in
// this call occurrence's own package for root slot `slot`. The evaluator
// intercepts the call and returns the restricted description marker; this
// declared body is only a statically well-typed stand-in.
public machine BuildProduct::entry(product: &BuildProduct, path: &[u8], slot: &[u8]) -> ProductEntryRef {
    ProductEntryRef {}
}
// Fallible logical query: select the exact product data declaration `path`
// names in this call occurrence's own package. The evaluator intercepts the
// call and returns the restricted schema-description marker; this declared
// body is only a statically well-typed stand-in.
public machine BuildProduct::schema(product: &BuildProduct, path: &[u8]) -> ProductTypeSchema {
    ProductTypeSchema {}
}
// Fallible logical query: select the exact product provider declaration `path`
// names in this call occurrence's own package. A provider declaration is a
// nominal data type owning at least one `satisfies` machine; other data does
// not qualify. The evaluator intercepts the call and returns the restricted
// provider-description marker; this declared body is only a statically
// well-typed stand-in.
public machine BuildProduct::provider(product: &BuildProduct, path: &[u8]) -> ProductProviderRef {
    ProductProviderRef {}
}
// compiler-owned optimization enable machine
// compiler-owned optimization report machine
"#;
