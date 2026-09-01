# Gamma compiler

This directory owns the compiler artifact required by the Gamma rung:

- `gamma_compiler.beta` is the canonical immediate-predecessor source;
- `gamma_compiler_bytecode.tape` is the current platform-independent artifact;
- `outcomes-v1.tsv` is the closed compiler-boundary code table consumed by the
  focused gate.

That table describes the current `GCOUT` contract; it is not evidence that a
one-customer bootstrap compiler needs a permanent detailed diagnostic ABI.
`BOOTSTRAP-MINIMAL-COMPILER-BOUNDARY` compares this frame and code surface with
the four semantic outcomes actually required, and
`BOOTSTRAP-SIDECAR-RETIREMENT` removes the TSV if no named external consumer
earns a machine-readable registry.

Exact construction and materialization live under `tools/bootstrap/gamma/`.
Focused language and artifact-structure tests live under
`tests/gamma/compiler/`.

Construction, testing, and evidence generation do not grant authority by
themselves. One Beta source directly produces one exact Gamma compiler tape.
The validation directory belongs here because the artifact
being admitted owns its validation. Bounded diagnostics can expose regressions,
but acceptance must ultimately terminate in the independently
rooted checker under `source/alpha/checker/`.

## Persisted artifact

`gamma_compiler_bytecode.tape` is emitted directly from
`gamma_compiler.beta`. Its complete construction lineage is:

```text
audited Alpha seed + direct Beta assembler tape
  -> gamma_compiler.beta
  -> gamma_compiler_bytecode.tape
```

`tools/bootstrap/gamma/rebuild-artifact.sh --check` reconstructs the tape and
compares it byte-for-byte without changing the repository. The adjacent
artifact tool stamps it into
the selected audited Alpha seed. No Gamma self-host, textual Beta output, or
second assembler invocation participates.

The former Beta-written status reconstructor was deleted after measured proof
work showed that it could not become the selected checked derivation. It was a
parallel assembly semantics, not an admission premise. The exact source/tape
certificate remains open under **BETA-GAMMA-COMPOSED-CERTIFICATE** in
`TASKS_BOOTSTRAP.md`. Its one root edge proof is settled to compose bounded
pass-one and pass-two equalities rather than require one compiler-scale
conversion.

The committed artifact is 27,087 bytes with SHA-256
`c03ec97d15e1e2b92876d101e54f47efd110cfc5f25fb6e5d8a148798a6704e5`.
The byte comparison, not the convenient digest, governs repository identity.

## Compiler boundary outcome

Compilation has one closed semantic result:

```text
GammaCompileOutcome =
    Complete(tape)
  | Reject(reason, source_offset)
  | Incomplete(resource, limit, requested, coordinate?)
  | InternalFailure(reason, coordinate?)
```

`Reject` records an observed Gamma-language violation. `Incomplete` records that
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
| 0..7 | magic/version `[FF 47 43 4F 55 54 01 00]` (`FF`, `GCOUT`, version 1, reserved) |
| 8 | outcome kind; must equal the halt tag |
| 9 | coordinate space: 0 none, 1 source byte, 2 emitted-payload byte, 3 internal row |
| 10..11 | zero; reserved |
| 12..15 | closed reason or resource code, little-endian `u32` |
| 16..23 | zero-based coordinate, little-endian `u64`; zero when the space is none |
| 24..31 | resource limit, little-endian `u64`; zero outside `Incomplete` |
| 32..39 | requested amount, little-endian `u64`; zero outside `Incomplete` |

Source coordinates are consumed-prefix boundaries in the compiler's fixed
first-decisive traversal. They range from zero through the exact source extent:
an outer-envelope failure records the offending unconsumed byte; an immediate
scanner/parser/formation failure records the current boundary after all
successfully consumed bytes and trivia; and a failure decidable only after
whole-program validation records the then-current boundary, ordinarily the
source extent. Thus a source coordinate is stable evidence of where the
streaming decision occurred, not a promise that every rejection names the
first byte of a human-selected token. Source-profile refusals use the same
boundary before the refused admission. Emitted-payload coordinates identify
the first unpublishable payload byte, and internal-row coordinates are
zero-based private row indexes. The focused gate pins the numeric coordinate of
every retained failure producer, including the exact trailing LF supplied by
its string-input helpers.

`0xFF` is permanently never an Alpha opcode, so a failure frame cannot be a
runnable payload. Unknown versions, kinds, coordinate spaces, reason/resource
codes, nonzero reserved bytes, noncanonical unused fields, or disagreement
between frame and halt tag reject the boundary observation. `outcomes-v1.tsv`
is the closed version-1 reason/resource table. `tests/gamma/compiler/test.sh` consumes it directly;
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
compiled Gamma program and never to this compiler carrier.

## Current compiler resource profile

The selected compiler profile fixes the following private ceilings before tape
publication. The compiler enforces every row below, including state-block
participation in the combined syntax-depth budget. These ceilings bound the
implementation accepted by the current edge. Exceeding any independently
reachable row below is `Incomplete`, never invalid Gamma source. The focused
gate checks the exact resource code, limit, requested amount, and canonical
failure framing at each adjacent refusal.

| Resource | Last admitted extent |
| --- | ---: |
| Source byte stream | 1,048,576 bytes |
| Identifier | 64 bytes |
| Combined state-block, parenthesis, nested-call, and nested-load syntax depth | 64 |
| Parameters plus function-scoped locals | 64 per procedure |
| Procedures | 256 |
| Non-builtin procedure call-reference rows | 1,024 |
| States | 128 per procedure; 1,024 total |
| Transitions | 256 per procedure; 1,024 total |
| Emitted runnable Alpha payload | 1,048,572 bytes |

These are compiler-profile ceilings, not Gamma language limits. State nesting
remains a recursive language form. Parentheses, calls, loads, and state bodies
consume one combined checked recursion budget because they compose on the same
Alpha parser call path; exhaustion reports `Incomplete` rather than rejecting
otherwise valid Gamma or exhausting the Alpha return stack.

D23 selects the one-MiB stamped-hole profile: the private
payload buffer admits exactly 1,048,572 raw bytes, and the procedure table
admits 256 rows. The rebuilt compiler tape, seeds, checker, downstream emitters,
and exact/adjacent gates now share that profile. The closed `GCOUT` v1
reason/resource identities do not change; their producer-owned numeric limits
do.

D58 governs the next private-table revision without changing the current
canonical limits prematurely. A roomy noncanonical compiler first stages the
complete `delta_compiler.gamma`; the completed source then selects each
independently provisioned authored-structure count as the least power of two
with measured occupancy no greater than 75 percent. Procedures, global calls,
global/per-procedure states and edges, derived initialization storage, labels,
fixups, tape, and maximum execution work are measured conjunctively rather than
projecting call pressure onto the other dimensions. Derived guards remain bound
to their owners and tape capacity remains D23-owned. The resulting memory map,
compiler tape, admission subject, and exact/adjacent gates publish atomically.

The generated data stack is separately guarded in `[1048576,2097152)` and every
procedure reserves at least its caller-frame word, as specified in
`../CALLING_CONVENTION.md`. Generated programs receive 134,217,728 zeroed bytes
of source-visible raw memory biased at physical byte 4,194,304. Their data-stack
and raw-memory containment failures are runtime statuses 250 and 251, not
compiler `Incomplete` results. D30 preserves those meanings in the chain-wide
generated-program block: 248 InternalFailure, 249 AuthoredTrap, 250
StackExhausted, 251 MemoryContainmentViolation, 252 HeapExhausted, 253
InputExtent, and 254 OutputExtent. A generated profile need not produce every
row. Alpha's VM trap remains 132, and 255 is deliberately unassigned and
noncanonical rather than aliasing a shell's projection of `-1`.

The private compiler map keeps the expanded procedure/initialization tables in
`[5 MiB,5,556,417)`, the payload buffer at 16 MiB, the internal-PC table at
20 MiB, and the fixup table at 24 MiB. The 116,508-row fixup table is dominated
by the shortest 9-byte direct-reference encoding. The 262,144-row internal-PC
table conservatively admits one identity per four emitted bytes. Both remain
secondary corruption guards: reaching either while the payload bound still
holds is `InternalFailure`, not an advertised resource refusal.
When D58 lands, changed count tables and their derived initialization rows move
to a new aligned private region above the fixup table rather than repacking this
dense low-memory block. Global-state maximum work receives an executed gate
because the current name collector scans prior global rows; per-procedure state
and edge limits separately own initialization/reachability fixed-point work.
The compiler test pins practical source limits at the exact accepted boundary and the
adjacent fail-closed case. Single-site temporary compiler mutations lower each
otherwise dominated invariant and positively exercise all six closed
`InternalFailure` producers without adding production test hooks. The gate also
pins the last valid byte/word raw-memory addresses and generated-stack
containment.

## Retention inventory

| Retained child | Bounded role | Deletion condition |
| --- | --- | --- |
| `gamma_compiler.beta` | Canonical Beta-written compiler source accepting Gamma. | Replace only atomically with its artifact and checked relation. |
| `gamma_compiler_bytecode.tape` | Canonical platform-independent compiler artifact. | Replace only with a green exact reconstruction from the canonical source. |
| `outcomes-v1.tsv` | Current manually maintained projection of compiler boundary codes. | Delete under `BOOTSTRAP-SIDECAR-RETIREMENT` if detailed codes have no named external consumer; otherwise replace only with the selected synchronized contract. |

The retained files are exactly one compiler source, one Alpha-tape artifact,
one boundary code table, and this owner document.
No separate cold-start, self-host, generated-artifact, or publication owner is
retained.
