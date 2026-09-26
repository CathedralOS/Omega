# Consumer semantic bindings

[src/semantic_bindings.rs](src/semantic_bindings.rs) owns the closed roles and
their normalized schema keys. The [toolchain/library contract](../../../../wiki/spec/packages/toolchain.md)
owns their authority boundary. These are narrow supported signatures, not a
generic accepted-boundary protocol for arbitrary nominal carriers.

| Role | Required identity and current consumer |
| --- | --- |
| Console exit | Exact package, boundary declaration, normalized service schema, and complete selected-plan digest. Only the joined symbol receives Process classification. Physical `HostedExitProcessI32` realization remains a separate Linux x86-64/AArch64 and macOS AArch64 catalog route; catalog admission does not establish selected-instruction support. |
| `ProcessExit` exit | Exact package, normalized service schema, and complete selected-plan digest for the canonical core `ProcessExit` boundary requirement. The toolchain-owned trait and requirement carry no package identity; the binding commits only the package-owned provider nominal and its plan. Shares the `HostedExitProcessI32` physical realization route. |
| `FilesystemHostService` | Exact package, boundary declaration, and complete schema; no plan digest or provider synthesis. Candidate review may nominate a reached same-named declaration and expose its checked schema as inert policy-authoring input. Final checking consumes the exact accepted binding. |
| Physical program entries | Exact package, the target's application declaration, and schema for that target's physical-entry consumer — one role per contract package: `UefiX64ProgramEntry`, `MacosArm64ProgramEntry`, `MacosX64ProgramEntry`, `LinuxX86_64ProgramEntry`, `LinuxArm64ProgramEntry`, `WindowsX64ProgramEntry`. Each key omits the two target-evaluated calling-plan fields (`calling_plan_report_fingerprint` and `calling_plan_commitment`) because the target independently replays the semantic and physical plans; no second ABI authority is introduced. A role binds only its own contract package; cross-target use rejects. |

The two process-exit roles bind through `new` with a selected provider plan
digest; the filesystem service and six program-entry roles bind through
`new_service` with no plan digest and synthesize no provider.

The filesystem permission-authoring control authors permissions against the
current 50-method schema through the six portable filesystem facets —
`ContentRead`, `ContentWrite`, `MetadataQuery`, `DirectoryEnumeration`,
`NamespaceMutation`, `MetadataMutation` — drawn from the fourteen-class
terminal-authority vocabulary, with mechanism-level dispositions governed by
the [filesystem policy](../../../../wiki/spec/build/permissions.md#portable-filesystem-control-and-lifecycle-authority).
Missing entries are not empty grants. Classification retains exact requirement
and mechanism identities; ordinary release narrowing needs checked occurrence
flow and a release contract. A known class set outside policy fails containment;
missing classification fails realization. Neither changes source service reach
or makes otherwise valid Terminal Psi malformed.

Package-aware interpretation takes an opaque routing token for that exact
resolved declaration and checked-program instance. Provider authority comes
separately from interpreter options. The standalone filesystem fallback is the
exact bundled source, not readable service/method names.

Ordinary std review uses resolver snapshots and its declared alias. It rejects
non-core `omega::language::*` shortcuts with dependency guidance. Only the actual
core directory is excluded from ordinary package overlap; the repository's std
directory may be an ordinary package. Standalone std/alloc provenance remains
temporary until each consumer has exact source-byte recognition or explicit
bindings. Console and every physical program entry already use narrow
byte-exact fallbacks — each target's bundled contract source remains accepted
beside its package-owned binding. Relabeling directory authority would not
complete this migration.

The non-std host-services fixture guards against accidentally using repository,
alias, or bundled-library identity as authority. Tests also require stale,
foreign, ambiguous, and unmatched bindings to fail before they affect review.
