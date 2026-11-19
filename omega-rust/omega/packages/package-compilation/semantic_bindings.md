# Consumer semantic bindings

[src/semantic_bindings.rs](src/semantic_bindings.rs) defines the fixed roles
a package declaration can be bound to, and their normalized schema keys; the
[toolchain/library contract](../../../../wiki/spec/packages/toolchain.md) sets
their authority. Each role is a narrow signature, not a general protocol for
any nominal type.

| Role | What must match exactly, and who uses it |
| --- | --- |
| Console exit | Package, boundary declaration, normalized service schema and complete selected-plan digest. Only the matched declaration is classified as Process. Physical `HostedExitProcessI32` realization is a separate Linux x86-64/AArch64 and macOS AArch64 catalog entry; catalog admission does not imply instruction-selection support. |
| `ProcessExit` exit | Package, normalized service schema and complete selected-plan digest for the canonical core `ProcessExit` boundary requirement. The toolchain's trait and requirement have no package identity; the binding fixes only the package's provider type and plan. Shares the `HostedExitProcessI32` realization. |
| `FilesystemHostService` | Package, boundary declaration and complete schema; no plan digest or generated provider. Review may propose a reached same-named declaration and show its checked schema as inert input for writing policy. Final checking uses the exact accepted binding. |
| Physical program entries | Package, the target's application declaration and its physical-entry schema; one role per contract package: `UefiX64ProgramEntry`, `MacosArm64ProgramEntry`, `MacosX64ProgramEntry`, `LinuxX86_64ProgramEntry`, `LinuxArm64ProgramEntry`, `WindowsX64ProgramEntry`. Each key omits the two target-computed calling-plan fields (`calling_plan_report_fingerprint` and `calling_plan_commitment`) because the target replays the semantic and physical plans itself; there is no second ABI authority. A role binds only its own contract package; other targets reject it. |

The two process-exit roles bind through `new` with a selected provider plan
digest. `FilesystemHostService`, `TimeHostService` and the six program-entry roles
bind through `new_service`, with no plan digest or generated provider.

Filesystem permissions are written against the current 50-method schema in
six portable facets (`ContentRead`, `ContentWrite`, `MetadataQuery`,
`DirectoryEnumeration`, `NamespaceMutation`, `MetadataMutation`), taken from
the fourteen-class terminal-authority vocabulary. The
[filesystem policy](../../../../wiki/spec/build/permissions.md#portable-filesystem-control-and-lifecycle-authority)
governs each mechanism. A missing entry is not an empty grant.
Classification keeps exact requirement and mechanism identities; narrowing a
release needs checked occurrence flow and a release contract. A known class
set outside the policy fails containment; missing classification
fails realization. Neither changes source service reach or makes valid Terminal Psi
malformed.

Package-aware interpretation receives an opaque routing token for the exact
resolved declaration and checked program instance; provider authority comes
separately, from interpreter options. The standalone filesystem fallback
recognizes the exact bundled source, not service or method names.

Ordinary std review uses resolver snapshots and its declared alias. It
rejects non-core `omega::language::*` shortcuts with dependency guidance. Only the real core directory is excluded from the
package overlap check; the repository's std directory may be an ordinary
package. Standalone std/alloc provenance is temporary, until each consumer recognizes
exact source bytes or has explicit bindings.
Console and every physical program entry already use narrow byte-exact
fallbacks: each target's bundled contract source is accepted beside its binding. Relabeling directory authority would not finish this migration.

A repository, alias or bundled-library identity never acts as a binding.
Stale, foreign, ambiguous and unmatched bindings fail before they affect
review.
