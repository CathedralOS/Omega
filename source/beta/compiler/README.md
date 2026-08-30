# Beta compiler

This directory owns the compiler artifact required by the Beta rung:

- `beta_compiler.alpha` is the canonical immediate-predecessor source;
- `beta_compiler_bytecode.tape` is the current platform-independent artifact;
- `outcomes-v1.tsv` is the closed compiler-boundary code table consumed by the
  focused gate;
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
certificate remains open under OWNER Q5 (the exact Alpha-to-Beta edge) in
`OWNER_QUESTIONS.md`.

The committed artifact is 26,751 bytes with SHA-256
`c54307cc855f72d6ab64a320237826fd9eff46307dc817727a3671ded9a747a1`.
The byte comparison, not the convenient digest, governs repository identity.

## Compiler boundary outcome

Compilation has one closed semantic result:

```text
BetaCompileOutcome =
    Complete(tape)
  | Reject(reason, source_offset)
  | Incomplete(resource, limit, requested, coordinate?)
  | InternalFailure(reason, coordinate?)
```

`Reject` records an observed Beta-language violation. `Incomplete` records that
the selected compiler exhausted a private profile before deciding the remaining
source; it is neither acceptance nor rejection. `InternalFailure` records a
contradiction in the compiler itself and grants no artifact authority. Parser
phase numbers are private implementation state and never identify an outcome.

The Alpha realization uses the halt word only as a case tag: `0` is `Complete`,
`1` is `Reject`, `2` is `Incomplete`, and `3` is `InternalFailure`. These values
survive both Alpha's 32-bit halt observation and a shell's low-byte projection.
On `Complete`, stdout is exactly the runnable Alpha payload and receives no
prefix; the seed-stamping envelope supplies its length outside this compiler
boundary. On every non-complete outcome, stdout is exactly one canonical
40-byte diagnostic frame:

| Bytes | Meaning |
| --- | --- |
| 0..7 | magic/version `[FF 42 43 4F 55 54 01 00]` (`FF`, `BCOUT`, version 1, reserved) |
| 8 | outcome kind; must equal the halt tag |
| 9 | coordinate space: 0 none, 1 source byte, 2 emitted-payload byte, 3 internal row |
| 10..11 | zero; reserved |
| 12..15 | closed reason or resource code, little-endian `u32` |
| 16..23 | zero-based coordinate, little-endian `u64`; zero when the space is none |
| 24..31 | resource limit, little-endian `u64`; zero outside `Incomplete` |
| 32..39 | requested amount, little-endian `u64`; zero outside `Incomplete` |

`0xFF` is permanently never an Alpha opcode, so a failure frame cannot be a
runnable payload. Unknown versions, kinds, coordinate spaces, reason/resource
codes, nonzero reserved bytes, noncanonical unused fields, or disagreement
between frame and halt tag reject the boundary observation. `outcomes-v1.tsv`
is the closed version-1 reason/resource table. `test.sh` consumes it directly;
the gate may decode the producer's fields but cannot define or repair them.

Success is intentionally not self-describing. Its integrity rests jointly on
complete first-pass validation, byte-count agreement with the private replay,
successful fixup resolution, an exact-length publication loop, Alpha's total
`write`, and `halt 0` occurring only after that loop returns. A gate stages all
stdout privately and publishes it as an artifact only after observing
`Complete`; partial output followed by a trap or nonzero halt never becomes a
tape.

The compiler reports the first decisive event in its fixed traversal. A known
language violation is `Reject`; exhaustion before a verdict is `Incomplete`;
and validation/replay disagreement or a supposedly impossible internal
condition is `InternalFailure`. Runtime statuses 250 (generated data-stack
exhaustion) and 251 (generated raw-memory violation) belong to execution of the
compiled Beta program and never to this compiler carrier.

## Current compiler resource profile

The selected compiler profile fixes the following private ceilings before tape
publication. The compiler enforces every row below, including state-block
participation in the combined syntax-depth budget. These ceilings bound the
implementation accepted by the current edge. Exceeding any independently
reachable row below is `Incomplete`, never invalid Beta source. The focused
gate checks the exact resource code, limit, requested amount, and canonical
failure framing at each adjacent refusal.

| Resource | Last admitted extent |
| --- | ---: |
| Source byte stream | 1,048,576 bytes |
| Identifier | 64 bytes |
| Combined state-block, parenthesis, nested-call, and nested-load syntax depth | 64 |
| Parameters plus function-scoped locals | 64 per procedure |
| Procedures | 128 |
| Non-builtin procedure call-reference rows | 1,024 |
| States | 128 per procedure; 1,024 total |
| Transitions | 256 per procedure; 1,024 total |
| Emitted runnable Alpha payload | 262,140 bytes |

These are compiler-profile ceilings, not Beta language limits. State nesting
remains a recursive language form. Parentheses, calls, loads, and state bodies
consume one combined checked recursion budget because they compose on the same
Alpha parser call path; exhaustion reports `Incomplete` rather than rejecting
otherwise valid Beta or exhausting the Alpha return stack.

The generated data stack is separately guarded in `[262144,1048576)` and every
procedure reserves at least its caller-frame word, as specified in
`../CALLING_CONVENTION.md`. Generated programs receive 33,554,432 zeroed bytes
of source-visible raw memory. Their data-stack and raw-memory containment
failures are runtime statuses 250 and 251, not compiler `Incomplete` results.

The 32,768-row fixup table and 65,536-row internal-PC table are secondary
corruption guards: each row requires emitted reference or control bytes, so the
payload ceiling is binding first. Reaching either while the payload bound still
holds is therefore `InternalFailure`, not an advertised resource refusal.
`test.sh` pins practical source limits at the exact accepted boundary and the
adjacent fail-closed case; it also pins the last valid byte/word raw-memory
addresses and generated-stack containment.

## Retention inventory

| Retained child | Bounded role | Deletion condition |
| --- | --- | --- |
| `validation/` | Exact reachable-artifact structure for this compiler edge. | Delete it when the direct checked source/tape refinement proves the same facts. |

Root files are the one compiler source, one Alpha-tape artifact, one boundary
code table, one artifact loader, one exact reconstruction entry point, and one
focused language gate.
No separate cold-start, self-host, generated-artifact, or publication owner is
retained.
