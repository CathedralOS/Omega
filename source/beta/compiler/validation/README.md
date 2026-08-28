# Beta compiler validation

This directory owns validation of the exact `bc.beta` source and persisted
`artifacts/bc.tape`. Its responsibilities are physically separate:

| path | role |
| --- | --- |
| `admission/bc-artifact-structure.sh` | Alpha-rooted instruction framing, reachable direct targets, procedure regions, call/return shape, and seed payload bounds |
| `admission/bc-block-control.sh` | canonical whole-source/artifact maximal-observation reconstruction for `B_bc1` |
| `admission/obligations/` | Alpha modules used to assemble the bounded exact-subject checkers |
| `admission/witnesses/` | untrusted witness producers; these cannot select or replace either admitted subject |
| `selfhost.sh` | fixed-point reconstruction from the persisted Alpha-rooted compiler artifact |
| `resource-boundaries.sh` | exact `B_bc1` compiler resource ceilings and deterministic checked failures |
| `malformed-progress.sh` | bounded fail-closed progress for unsupported tokens in persisted and self-built compilers |
| `stress/` | curated/generated instruction-refinement and differential suites; useful evidence, never another lattice rung |

Only the two commands under `admission/` run on the default Beta lattice edge.
The commands under `stress/` are directly runnable optional cross-checks.

## Canonical whole-compiler obligation

`admission/bc-block-control.sh` has one mode and one input format:

```text
u32_le source_length
exact bc.beta bytes
u32_le tape_length
exact bc.tape bytes
canonical control witness
canonical call-bounds witness
```

The shell owns and packages the exact repository source and tape. The Python
mapper and call-bounds analyzer are untrusted witness producers; they cannot
substitute either subject. Four format-level negative controls retain the same
canonical checker programs while changing source bytes, tape bytes, framing,
or witness extent. All must reject with empty output.

The canonical conjunction is split across fourteen Alpha executables because
the audited seed accepts at most 262,140 tape bytes. Responsibility-specific
`.alpha` modules are concatenated only into those bounded programs. The
programs cover source/artifact control and effect custody, frame and stack
shape, memory-site classification, expression and statement composition,
bounded emitters, parsing/resource outcomes, and the final greatest-fixed-point
maximal observation. Responsibility-specific decoded-region descriptors share
one parameterized effect census for exact call, return, write, store, and
raw-byte-access policy. Selected local, memory, primitive, and push rows likewise
share one canonical exact-table decoder instead of tranche-local copies. The
same responsibility-neutral owner decodes compiler-generated push, pop,
saved-frame prologue, optional frame-allocation, parameter-store, and epilogue
macros once and returns their checked instruction starts and exclusive
successors. It also owns the generated root prelude, contiguous call-pop
sequences, pop-before-store shape, and return-relative epilogue lookup. Frame,
effect, memory, expression composition/primitive, and stack-table consumers
pass independently reconstructed semantic identities and PC/register/slot
facts to those decoders instead of embedding another instruction-byte copy,
macro length, or successor calculation. A shared fail-closed
procedure resolver likewise maps independently scanned source procedure IDs to
the unique checked entry-block PC. All semantic consumers now use that identity
for the selected `emit_dec`, `emit_pop_into`, `emit_push`, `gen_sum`,
`gen_expr`, `gen_stmts`, and `parse_proc` entries and adjacent procedure
boundaries instead of pinning those 79 entry PCs. Intra-procedure block and
call-site identity decoding has one reusable fail-closed owner. Event identity
is now source-row-free: the checker scans its independently reconstructed event
table for procedure, block, kind, exact ordinary-call target name, arity,
checker-owned ambient height, and exact decoded emit bytes. A unique lookup must
find exactly one complete key; an ambiguous lookup additionally requires an
exact same-key cardinality and lexical occurrence. Neither discriminator comes
from the witness, and the full scan must finish before its already-validated PC
can be selected. Reserved read/write/emit/return kinds are reconstructed only
after their exact source identifiers match. `emit_dec` and the adjacent
`emit_proc_prologue`/`emit_param_store` fixed-decimal family are complete
consumers: their block, transition, call, local, primitive, push,
explicit-return, synthetic-return, and region-boundary checks no longer repeat
82 artifact-PC literals or source event rows. The two identical `emit_dec`
calls and newline emits in `emit_param_store` are selected as occurrences zero
and one of exact cardinality two. Call continuations come from an identified
transition or the canonical nine-byte call fallthrough, and epilogues are
reconstructed relative to a row-free return identity or next procedure entry.
The `emit_prelude` ordered trace likewise uses five unique literal keys rather
than rows; `main.ready` resolves the former procedure 41 entry pins by identity.
`gen_stmt` is the next complete control/effect
consumer: its 16 blocks, 14 transitions, 27 calls, nine explicit returns, and
synthetic return now remove another 126 artifact-coordinate occurrences in
favor of procedure/block identities, canonical call fallthroughs, and
return-relative epilogues; its calls and returns no longer accept event rows.
Its coupled data and meaning modules required no coordinate change. Primitive
composition similarly obtains literal or binary
extents from the primitive-row owner; binary extents compose the shared pop
successor with the local opcode tail rather than repeating total macro widths.
Other intra-procedure consumers remain to migrate. A fifth
negative control mutates `emit_dec`'s witness event PC while retaining the exact
source and artifact and proves that witness coordinates cannot select semantic
identity. A sixth swaps the two valid same-key `emit_param_store` call PCs and
proves that exact-cardinality occurrence identity retains ordered continuation
custody. The final ROOT tape is 81,940 bytes for the current
exact subjects, SHA-256
`31a09b73275765af165ccb12b5693f964368751bccd96868e85f66cd25e1acd6`.

Historical focus modes, per-mutation checker-source permutations, local green
receipt caches, and mutation-only mapper outputs were removed. Git history is
their archive. The retained command reconstructs every canonical prerequisite
on every run and completes in tens of seconds rather than depending on cached
host state.

Run it directly from any working directory:

```sh
sh source/beta/compiler/validation/admission/bc-block-control.sh
```

## Authority and limits

The ROOT conjunction is unique lower-rung executable evidence for exact
maximal-observation equality over finite `B_bc1` inputs, including typed
resource outcomes and coinductive divergence. It is not yet a certificate
accepted by the universal `source/alpha/checker/artifacts/check.tape`:
the current proof language has no encoded Alpha/Beta simulation relation or
settled coinduction rule. Accordingly the short chain manifest continues to
disclose complete Beta source/artifact admission as open rather than promoting
this executable reconstruction by pedigree.

`stress/refinement.sh` separately derives Beta and Alpha symbolic meanings for
curated and generated program families and asks the rooted checker to validate
their equivalence. See [`stress/REFINEMENT.md`](stress/REFINEMENT.md) for that
narrower claim and its exact unsupported cases.

The shared parser and concrete reference interpreter live under
`source/beta/reference/`. Validation may consume them as untrusted
reconstruction or differential machinery; neither defines Beta. Beta's
canonical runtime meaning remains [`../../SEMANTICS.md`](../../SEMANTICS.md).
