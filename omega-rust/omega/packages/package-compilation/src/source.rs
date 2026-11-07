/*
We own the package and standalone source subsystem for compilation: capturing and assembling the source forest (`source_assembly`), discovering imports in authored order (`source_imports`), binding neutral source acquisition to `Build`'s canonical input metadata (`source_snapshot`), recording exact consumed source bytes (`source_consumption`), deriving anonymous invocation ownership (`anonymous_owner`), locating the toolchain's own bundled library (`toolchain_library`), and enforcing canonical host-filesystem modes and byte ceilings (`canonical_fs`).
*/

/*
Why source assembly and source consumption live in `package-compilation`:
Both standalone and package-aware compilations must prove which exact source bytes, generated dependency handoffs, and toolchain contracts entered a checked program without letting `psi/` depend on package graphs or host staging sponsors. Centralizing source assembly, import discovery, and consumption commitments here keeps the `psi/` frontend target-neutral and package-graph-agnostic while binding every parsed `SourceId` to its exact package or anonymous content owner.
*/

/*
Provenance:
- Anonymous source owner derivation (`AnonymousSourceCapture`): `src/source/anonymous_owner.rs`.
- Canonical host filesystem mode and symlink target helpers: `src/source/canonical_fs.rs`.
- Source forest assembly (`ImmutableSourceParseCheckpoint`, `DeferredProductSources`, `AssembledSyntax`): `src/source/source_assembly.rs`.
- Source consumption commitments and custody (`PackageCompilationSubject`, `BuildSourceCustody`, `CheckedSourceCustody`): `src/source/source_consumption.rs`.
- Import discovery, frontier storage, and module bindings: `src/source/source_imports.rs`.
- Canonical `Build` source snapshots and physical capture: `src/source/source_snapshot.rs`.
- Toolchain library discovery and release digest verification: `src/source/toolchain_library.rs`.
*/

pub(crate) mod anonymous_owner;
pub(crate) mod canonical_fs;
pub mod source_assembly;
pub(crate) mod source_consumption;
pub mod source_imports;
pub(crate) mod source_snapshot;
pub mod toolchain_library;
