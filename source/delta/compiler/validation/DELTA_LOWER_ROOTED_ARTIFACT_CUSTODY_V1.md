# Delta lower-rooted artifact custody, version 1

This contract joins an already verified Delta assembly-publication receipt to
one exact unsigned Darwin ARM64 Mach-O executable. It closes **byte custody,
container identity, exact-command realization replay, and terminal receipt
binding**. It is not source-to-artifact refinement or compiler authority. The
receipt discloses exact lower-rung, realization-host, and target admissions,
but does not claim correctness for Apple clang, the Apple linker, an SDK,
libSystem, or the compiler runtime.

The schema is `omega.delta-lower-rooted-artifact-custody.v1`, the publication
ID is `delta.compiler.darwin-arm64-executable.candidate.v1`, and the claim is
deliberately limited to
`candidate_lower_rooted_executable_realization_replay_custody`.

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

`generate` and `verify` instantiate this command with the exact supplied
absolute clang and linker paths; ambient `PATH` lookup rejects. `SDK_ROOT` is
the parent of the supplied absolute `SDKSettings.json` path.
They copy the already captured assembly bytes unchanged into a private
temporary directory, use a separate temporary output, allow no stdin, bound
the command to 300 seconds, and require status zero plus byte-empty stdout and
stderr. The replayed output must independently pass the V1 Mach-O validator and
equal the captured candidate byte for byte. Inputs are captured again after
the command and must still equal their pre-replay observations.

The compiler driver remains an ambient admitted producer. Recording its exact
inputs makes the build reproducible and prevents a result from being silently
cross-paired with another toolchain. It does not establish that the toolchain
is correct.

Every already-limited artifact-custody input is acquired as one bounded snapshot
from one open descriptor. The verifier reads at most the declared ceiling plus
one byte, compares descriptor identity and extent before and after the read,
and then cross-pairs the descriptor with the post-read path identity and
extent. Assembly-dialect and Mach-O validation consume the same captured bytes
used for their retained length and digest; validation and identity cannot come
from separate reads of a changing path.

## Reconstructed custody

[`lower_rooted_artifact_custody_v1.py`](lower_rooted_artifact_custody_v1.py)
reconstructs all of the following before emitting or verifying a receipt:

1. the complete V1 assembly receipt from its canonical source image, lower-rung
   tools, template, closed Gamma program, two raw executions, decoded assembly,
   and diagnostics;
2. byte equality between the assembly identity in that receipt and the
   assembly supplied to realization;
3. the exact identities of the Mach-O bytes, clang driver, linker, SDK settings,
   libSystem stub, compiler-runtime archive, and empty process streams;
4. execution of the literal V1 command with the captured assembly, exact
   supplied tool paths, and a fresh temporary output;
5. successful empty-diagnostic replay, independent validation of the replayed
   Mach-O, and byte equality with the candidate; and
6. the bounded target/container profile below.

The receipt's `reconstruction` member retains deterministic identities for the
replay input, output, empty streams, command profile, and target summary. A
handcrafted Mach-O can exercise `observe` and the container validator, but it
cannot receive this reconstruction-bearing receipt without being reproduced by
the declared command.

The same member binds the checked reconstruction profile
`delta.lower-rooted-executable-reconstruction.v1`. Its closed obligation list
states exactly what this verifier reconstructs:

1. the complete assembly publication;
2. its source and lower-rung inputs;
3. the published-assembly cross-pair;
4. the realization observation;
5. one execution of the literal realization command;
6. byte equality of the replayed and candidate executables; and
7. stability of every realization input across replay.

`status: checked` applies only to those custody/replay obligations. It does not
apply to Delta semantics or source-to-artifact refinement.

## Terminal receipt subjects and admissions

The terminal receipt surfaces, rather than merely leaving implicit behind the
parent receipt digest, the exact identities needed by later refinement:

- `assembly_publication` retains the rederived parent receipt ID and digest,
  canonical source snapshot and image, emitted assembly, and assembly target;
- `assembly` retains the exact realization input;
- `artifact` retains the observed candidate executable;
- `reconstruction.artifact` retains the independently replayed executable; and
- `target` plus the executable target summary retain the selected ABI,
  configuration, bounded container facts, deployment target, SDK version, and
  dynamic dependency closure.

The parent digest still binds the complete elaboration, packed Gamma program,
two executions, raw results, diagnostics, and lower-rung inputs. `generate` and
`verify` rederive that complete parent from its evidence; copying the selected
subjects into this receipt does not replace or abbreviate that check.

`admissions` then makes the remaining trust scopes explicit and binds their
exact identities:

- `hosts.lower_rung` copies the rederived assembly receipt's complete toolchain
  and authority-role disclosures. Its scope is assembly-publication
  reconstruction only.
- `hosts.realization` copies the exact clang, linker, SDK-settings, libSystem,
  and compiler-runtime identities observed around replay. Its scope is exact
  identity and replay only, not tool correctness.
- `target` copies both the strict assembly target and validated executable
  target summary. Its scope is closed dialect/container validation only, not
  Delta semantic refinement.

Every copied identity is reconstructed from supplied evidence and compared as
part of the domain-separated receipt. A self-consistent digest with a source,
assembly, replayed executable, obligation, host admission, or target admission
from another evidence set rejects.

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

The verifier has no default evidence paths. `observe` validates supplied
identity/container evidence without running clang; `generate` and `verify`
both execute the replay described above:

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

The adjacent canonical initial-realization runner has no discovery defaults:

```text
realize-delta-artifact-v1.py DESTINATION ASSEMBLY CLANG LINKER \
  SDK_SETTINGS LIBSYSTEM_STUB COMPILER_RUNTIME
```

Every argument must be absolute, `DESTINATION` must be absent, and its parent
must already exist. The runner executes the same literal V1 command with no
stdin in a private sibling directory, captures status, elapsed milliseconds,
stdout, and stderr, and requires the existing `observe` command to accept the
result. Assembly and all five realization-tool identities are captured before
the command and must still match both the observation and a post-observation
snapshot. It then atomically renames the directory into place with an exclusive
no-replace operation. Failure or a destination race removes the private
directory and publishes nothing. A successful directory contains exactly
`delta-compiler`, `realization.stdout`, `realization.stderr`, and
`realization-observation.json`; tool discovery and later custody receipt
generation remain explicit caller responsibilities.

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
