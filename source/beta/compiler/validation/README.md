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
share one canonical exact-table decoder instead of tranche-local copies. Their
five witness-PC families also share one bounded ingestion owner: each thin
wrapper fixes its exact row count, source-block table, and destination table
before any untrusted PC is read; the owner then requires an exact decoded
instruction start inside the independently reconstructed block extent. Family
parsers and semantic validators remain separate. The same
responsibility-neutral owner decodes compiler-generated push, pop,
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
custody. Memory identity now returns a checked PC from the bounded source
row/block/kind/width tuple instead of accepting a semantic caller's artifact
coordinate. `gen_stmts` rejoins its loads to independently checked address
literals and its stores to the shared pop-before-store macro; a seventh control
swaps two valid same-block word-load PCs and rejects. `gen_expr` now uses the
same memory-owner rejoins for its complete memory family. The synthesized
`__write_str` helper resolves from the exact main-prelude successor and mapper
cell; effect custody owns its sole exhaustive body check, while event and
summary consumers use its returned relative sites. The final ROOT tape is
82,921 bytes for the current exact subjects, SHA-256
`b3e41553ac5b52117bd17cf8028a6882fbc16729cd91f69bbd400e53edfc2731`.
`gen_emit`'s three identical newline events now use checker-owned exact
cardinality and lexical occurrence rather than source rows. An eighth control
swaps the first two valid witness PCs and is rejected by the label-emitter
owner's artifact-order rejoin. The following memory consumers now resolve
their load/store sites through the same checked identities: `cmp_op`,
`count_lets`, `declare`, and `emit_ident`. Expression call/leaf/`gen_expr` and
fixed-keyword procedure boundaries use procedure, call-fallthrough, and
epilogue identities. Label/reference and statement-emitter modules derive
their helper/literal layout from checked emit events instead of retaining a
second absolute layout. Expression-call rules, identifier indexing, expression
resource handling, fixed-keyword tables, `let`, `parse_char`, `gen_stmt` data,
`gen_factor`, `gen_emit`, `gen_state`, `gen_store`, and both `gen_to` modules now
resolve their shifting semantic sites through those same owners. Repeated
`gen_to` calls and emits use exact complete-key cardinality/occurrence, its word
loads rejoin checked address literals, and its CUR store rejoins the canonical
pop-before-store shape. The remaining cursor, label, slurp, parse/data,
ranged-store/resource, root-observation, and statement-label consumers now use
the same checked memory identities and load/store rejoins. No semantic consumer
calls the transitional coordinate-taking memory adapter.
The next compact localization tranche covers expression rules,
declaration/expect shapes, small statement/summary edges, `main.ready`, and the
root cleanup join. The latter now consumes the checked program-prelude owner
instead of independently restating the root call and halt bytes.
Parse-number/output-prefix/params-control, lookup/name-equality/operator,
fixed-keyword, `gen_emit` summary, and the main-loop/slurp bridge now consume the
same identities. In particular, source emit keys use verifier-owned literal
bytes—not artifact runtime string pointers—and root callers share the canonical
program-prelude owner.
Expression call-control/factor-data/leaf/levels/`gen_expr` and the
classifier/`cmp_op`/`count_lets` families are localized as well. Their repeated
calls use explicit occurrence cardinality, fixed emits are selected by
verifier-owned literal keys, and decoded scan bounds derive from procedure or
block identities.
The final bounded-emitter, emit-cmp, string-body, `gen_stmts`, literal-skip, and
whitespace tranche closes semantic identity localization. Same-key direct
writes now have a shared occurrence/cardinality identity. The only remaining
coordinate-taking calls are inside the three low-level owners or the two checks
whose coordinates were already returned by an independent semantic identity.

One live-count-derived procedure inventory proves that the checked block tables
form one total ordered partition: procedure IDs are contiguous, every procedure
has one entry followed only by state blocks, and all 359 rows are consumed.
It publishes only process-local first/exclusive block rows and entry/exclusive
artifact PCs. Existing procedure-entry and block-range queries rejoin that
structural product; frame, effect, memory, stack, and meaning remain separate.
Forty-seven consumers have dropped census-only
literal block listings and redundant private span scans while retaining every
PC-producing identity and semantic graph theorem. Exhaustive effect and fixed
emitter scans read their canonical validated live counts rather than freezing
a second, easily stale universe size.

The 47 expression-census callers also bind their primitive, binary-push,
argument-push, and store-push intervals through four cumulative boundary
tables built after those exact source tables pass validation. This removes a
full 829-row primitive scan and 407-row push scan from every query while
leaving primitive and push semantics with their existing owners. Four internal
mutation teeth independently perturb the terminal boundary of each family and
require the constant-time query to reject.

The 57 direct effect-census calls use the same construction discipline for
local-access, memory-site, transition, and event row intervals. Four more
prefix tables replace 80,320 repeated row visits per full consumer traversal;
their owner checks source-block ordering, exact terminal counts, restored
positive lookup, and one mutation tooth per family. Local kinds and slots,
memory joins, transition meaning, event keys and occurrences, artifact PCs, and
decoded opcode/effect policy remain separate semantic obligations.

The whole-artifact frame summary retains one canonical per-PC reachability,
relative-depth, frame-kind, and saved-frame product. Ranged-store transfer
rejoins its selected slurp/declare sites through checked accessors to that
product instead of running a second, weaker fixed point. Its operand/value
classification, exact row joins, and stack custody remain separate obligations.

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
settled coinduction rule. [`OWNER_QUESTIONS.md`](../../../../OWNER_QUESTIONS.md)
Q18 owns that language decision. Accordingly the short chain manifest continues to
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
