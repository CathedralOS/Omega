# Beta compiler

This directory owns the compiler artifact required by the Beta rung:

- `beta_compiler.alpha` is the canonical immediate-predecessor source;
- `beta_compiler_bytecode.tape` is the current platform-independent artifact;
- `validation/` contains only machinery that targets the canonical source or
  its emitted tape;
- `rebuild-artifact.sh` performs exact direct construction;
- `test.sh` owns the focused accepted/rejected language discriminators;
- `artifact_env.sh` installs the admitted tape into the selected Alpha seed.

Construction, testing, and evidence generation do not grant authority by
themselves. One Alpha source directly produces one exact Beta compiler tape.
The validation directory belongs here because the artifact
being admitted owns its validation. Bounded diagnostics can expose regressions,
but acceptance must ultimately terminate in the independently
rooted checker under `source/alpha/checker/`.

## Persisted artifact

`beta_compiler_bytecode.tape` is emitted directly from
`beta_compiler.alpha`. Its complete construction lineage is:

```text
audited Alpha seed + Alpha-written assembler
  -> beta_compiler.alpha
  -> beta_compiler_bytecode.tape
```

`rebuild-artifact.sh --check` reconstructs the tape and compares it
byte-for-byte without changing the repository. `artifact_env.sh` stamps it into
the selected audited Alpha seed. No Beta self-host, textual Alpha output, or
second assembler invocation participates.

The former Alpha-written status reconstructor was deleted after measured proof
work showed that it could not become the selected checked derivation. It was a
parallel assembly semantics, not an admission premise. The exact source/tape
certificate remains open under OWNER Q10 (the exact Alpha-to-Beta edge) in
`OWNER_QUESTIONS.md`.

The committed artifact is 20,977 bytes with SHA-256
`1911fc4f9667081ca96559ee970f07c3359f225c1177b5ed889d55c05a059f0f`.
The byte comparison, not the convenient digest, governs repository identity.

## Current compiler resource profile

The Alpha-written compiler enforces the following fixed private ceilings before
publishing any tape. They bound the implementation accepted by the current
edge; they do not settle OWNER Q8's typed `Complete` / `Reject` / `Incomplete` /
internal-failure carrier. The focused gate therefore requires every refused
adjacent case to return nonzero with empty stdout, without assigning that raw
status its future language-level meaning.

| Resource | Last admitted extent |
| --- | ---: |
| Source byte stream | 1,048,576 bytes |
| Identifier | 64 bytes |
| Shared parenthesis, nested-call, and nested-load depth | 64 |
| Parameters plus function-scoped locals | 64 per procedure |
| Procedures | 128 |
| Call sites | 1,024 |
| States | 128 per procedure; 1,024 total |
| Transitions | 256 per procedure; 1,024 total |
| Emitted runnable Alpha payload | 262,140 bytes |
| Source-visible raw memory | 33,554,432 zeroed bytes |

The generated data stack is separately guarded in `[262144,1048576)` and every
procedure reserves at least its caller-frame word, as specified in
`../CALLING_CONVENTION.md`. The 32,768-row fixup table and 65,536-row internal-PC
table are secondary corruption guards: each row requires emitted reference or
control bytes, so the payload ceiling is binding first. `test.sh` pins practical
source limits at the exact accepted boundary and the adjacent fail-closed case;
it also pins the last valid byte/word raw-memory addresses and generated-stack
containment.

## Retention inventory

| Retained child | Bounded role | Deletion condition |
| --- | --- | --- |
| `validation/` | Exact reachable-artifact structure for this compiler edge. | Delete it when the direct checked source/tape refinement proves the same facts. |

Root files are the one compiler source, one Alpha-tape artifact, one artifact
loader, one exact reconstruction entry point, and one focused language gate.
No separate cold-start, self-host, generated-artifact, or publication owner is
retained.
