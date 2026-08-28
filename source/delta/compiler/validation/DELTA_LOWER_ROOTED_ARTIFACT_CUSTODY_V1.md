# Delta lower-rooted artifact custody, version 1

This contract joins an already verified Delta assembly-publication receipt to
one exact unsigned Darwin ARM64 Mach-O executable. It closes **byte custody and
container identity only**. It is not source-to-artifact refinement, compiler
authority, or an admission of Apple clang, the Apple linker, an SDK, libSystem,
or the compiler runtime.

The schema is `omega.delta-lower-rooted-artifact-custody.v1`, the publication
ID is `delta.compiler.darwin-arm64-executable.candidate.v1`, and the claim is
deliberately limited to `candidate_lower_rooted_executable_identity_only`.

## Direct realization command

The executable is produced directly from the exact assembly bytes accepted by
the assembly receipt. The V1 command profile is:

```text
apple-clang
  -arch arm64
  -isysroot SDK_ROOT
  -fuse-ld=LINKER
  -mmacosx-version-min=11.0
  -Wl,-no_uuid
  -Wl,-no_adhoc_codesign
  -o ARTIFACT ASSEMBLY
```

The command must write empty stdout and stderr and exit zero. No wrapper may
rewrite the assembly or executable. A runner may locate the selected clang,
linker, SDK, libSystem stub, and compiler-runtime archive, but their exact bytes
are inputs to the observation and receipt; a generic tool name or version
string is insufficient.

The compiler driver remains an ambient admitted producer. Recording its exact
inputs makes the build reproducible and prevents a result from being silently
cross-paired with another toolchain. It does not establish that the toolchain
is correct.

## Reconstructed custody

[`lower_rooted_artifact_custody_v1.py`](lower_rooted_artifact_custody_v1.py)
reconstructs all of the following before emitting or verifying a receipt:

1. the complete V1 assembly receipt from its canonical source image, lower-rung
   tools, template, closed Gamma program, two raw executions, decoded assembly,
   and diagnostics;
2. byte equality between the assembly identity in that receipt and the
   assembly supplied to realization;
3. the exact identities of the Mach-O bytes, clang driver, linker, SDK settings,
   libSystem stub, compiler-runtime archive, and empty process streams; and
4. the bounded target/container profile below.

The Mach-O validator requires a 64-bit ARM64 `MH_EXECUTE` image with macOS 11.0
minimum deployment, exactly the `NOUNDEFS`, `DYLDLINK`, `TWOLEVEL`, and `PIE`
header flags, `LC_MAIN` inside `__TEXT,__text`, the required
`__PAGEZERO`, `__TEXT`, `__DATA`, and terminal `__LINKEDIT` segments,
`__DATA,__bss`, `/usr/lib/dyld`, and exactly
`/usr/lib/libSystem.B.dylib`. It rejects UUIDs, code signatures, encrypted text,
wrong targets, executable-stack or other extra header policy, malformed
command/section ranges, an implicit stack request,
extra dynamic libraries, unknown load commands, overlapping or out-of-order
segments, link-edit payloads outside the terminal `__LINKEDIT` extent, and
artifacts over 64 MiB. The closed V1 load-command vocabulary additionally
permits the bounded symbol/dynamic-symbol tables, dyld-info, source-version,
function-start, and data-in-code metadata emitted by the declared command
profile; each permitted command has an exact structural check rather than an
ignore path. Section custody is likewise closed over the compiler's `__text`,
`__const`, and `__bss` plus the command profile's reviewed ARM64 stubs, pointer
tables, and linker data. Unknown or executable substitute sections, retained
relocations, invalid stub metadata, section/load-command aliasing, and
overlapping file or virtual ranges reject. This vocabulary does not freeze the
SDK version: SDK version is allowed to vary, is recorded in the validated
target summary, and the exact SDK settings identity is bound. Individual
realization-tool inputs are bounded at 512 MiB; SDK settings and the libSystem
stub are bounded at 64 MiB.

Canonical observations and receipts are key-sorted JSON with two-space
indentation and one final LF. Malformed or cross-paired evidence returns 251;
resource overflow returns 252; rejection writes no stdout bytes. Receipt hashes
use the domain `omega.delta-lower-rooted-artifact-custody.v1\0`, followed by the
u64 little-endian compact-projection length and canonical compact JSON with the
digest field omitted.

## Commands

The verifier has no default evidence paths and does not run clang:

```text
observe STATUS ELAPSED_MS ASSEMBLY ARTIFACT STDOUT STDERR \
  CLANG LINKER SDK_SETTINGS LIBSYSTEM_STUB COMPILER_RUNTIME

generate ASSEMBLY_RECEIPT REALIZATION_OBSERVATION ASSEMBLY ARTIFACT \
  STDOUT STDERR CLANG LINKER SDK_SETTINGS LIBSYSTEM_STUB COMPILER_RUNTIME \
  ASSEMBLY_JOIN_ARGUMENTS...

verify ARTIFACT_RECEIPT \
  ASSEMBLY_RECEIPT REALIZATION_OBSERVATION ASSEMBLY ARTIFACT \
  STDOUT STDERR CLANG LINKER SDK_SETTINGS LIBSYSTEM_STUB COMPILER_RUNTIME \
  ASSEMBLY_JOIN_ARGUMENTS...
```

`ASSEMBLY_JOIN_ARGUMENTS` are the same explicit manifest, location sidecar,
tool tapes, template, closed Gamma program, observations, raw outputs,
assemblies, diagnostics, and role roots consumed by
`lower_rooted_assembly_publication_v1.py generate|verify`. Thus possession of a
self-digested assembly receipt without its evidence cannot mint executable
custody.

## Open semantic refinement

Every receipt carries:

```text
status: open
reason: authoritative_delta_v1_semantics_subject_not_published
```

The repository currently has an executable Delta-to-Gamma meaning
implementation, but no authoritative Delta v1 language/operational-semantics
subject from which an artifact-aware verifier can reconstruct the proposition
that `source/delta/compiler/main.alp` and this Mach-O executable must satisfy. Treating the
translator implementation as both the semantics and its own refinement witness
would be circular.

Settling that semantic subject is a language-design decision. Once it exists,
building the target-semantics reconstruction, observation profile, exact
obligation ledger, certificates, negative controls, and Alpha-checker join is
engineering work. Until that separate direct refinement passes, this receipt
cannot replace the provisional compiler artifact or authorize it to build
`omega₀`.
