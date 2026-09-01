# Delta compiler owner

The canonical compiler owned here accepts Delta, is implemented in Gamma, and
emits platform-independent Alpha tape:

```text
delta_compiler.gamma → delta_compiler_bytecode.tape
```

The source now exists as an incomplete implementation; the tape does not. Its
retained frontend is the former standalone checker, moved rather than copied,
and its direct Alpha payload/label/fixup substrate is final compiler material.
The adjacent gate compiles this one source with temporary fixed test
entries, retains all 98 frontend discriminators, and executes the emitter,
runtime containment, frame/value ABI, checked `Int`, compact `Bytes`, sealed
input, resolved-expression, selected-match, and whole-function paths. One
full-source test emitter now covers ordinary and proper-tail calls,
constructors, locals, matches, and general expressions instead of rebuilding
five redundant compiler variants. It also pins malformed whole-function
metadata and exact/adjacent label capacity before payload mutation. Seven D19
schema cases cover both profiles, reversed reason declaration order, and
missing/malformed outcome rows without emitting an adapter. The gate publishes
no compiler artifact.

The retained compiler source declares 116 procedures. With the fixed frontend
gate entry, the compiled gate uses 117 of Gamma's 256 procedure slots and
compiles to 333,928 bytes. The remaining 714,644 bytes under
Alpha's runnable payload ceiling are a measured implementation budget, not a
Delta language limit or evidence that every remaining compiler component fits.
Before the production entry and D19 adapters exist, the source also consumes
965 of 1,024 non-builtin call rows, 739 of 1,024 global states, and 586 of 1,024
global edges. D58 keeps those current limits canonical only as a staging
baseline. A roomy noncanonical Gamma compiler completes this source, after which
the full conjunctive demand selects independently provisioned authored-
structure counts with no more than 75 percent occupancy. Derived guards remain
bound to their owners and tape capacity remains D23-owned when the Gamma compiler
profile republishes atomically.

`../interp.gamma` remains a bounded execution oracle. It may contribute an
isolated lowering/runtime algorithm where economical, but no interpreter loop,
serialized Delta AST, or second frontend enters the canonical compiler.

The compiler uses a private arbitrary-arity frame ABI and preserves proper tail
calls. Its emitted compiler-application adapter alone supplies sealed input as
Delta `Bytes`, invokes the typed `main`, and serializes exact success or the
accepted-language edge's failure frame. Fuel and private storage ceilings yield
outer resource outcomes; they never change Delta meaning.

Fixed frame locals use zero-based opaque indexes into the aligned two-word
slots after the frame header. One shared validator and guarded emitter own both
load and store, so resolved variables, lets, and pattern binders reuse
one containment rule. The resolved let seam validates its packed
`(prefix,index)` profile before emitting its initializer, evaluates that
initializer once outside tail position, stores the complete value, and passes
the caller's tail context to its body. D20 assigns source binders and references
to those indexes without active lexical shadowing; the ABI chooses neither.
Resolved parameters use the frame's already-fixed reverse physical order: a
guarded accessor validates the complete prefix, parameter count, and opaque
source-order index, and requires their combined extent to stay inside the
explicit-stack profile before loading one pair. D20 assigns references to those
indexes but does not own or alter their runtime placement.

## Implementation shape

The compiler source is growing one pipeline inside `delta_compiler.gamma`:

```text
sealed source
  -> strict parse and declaration collection
  -> type resolution and checked typed IR
  -> direct Alpha lowering and fixups
  -> complete private payload validation
  -> one publication
```

The compile-time AST/IR is never serialized into the emitted program. No Delta
evaluator loop, syntax-tag dispatcher, textual Alpha stream, or host-side
assembler participates in the edge.

Under `AlphaBootstrapV2`, the Gamma executable has exactly 128 MiB of
source-visible logical raw memory. The frontend
keeps sealed source in `[2 MiB,6 MiB)`, declaration and environment tables in
`[6 MiB,10.5 MiB)`, labels/fixups below 13 MiB, payload instruction starts in
`[126 MiB,127 MiB)`, and AST storage in `[16 MiB,125 MiB)`. The final
`[127 MiB,128 MiB)` is the private one-MiB payload buffer; the runnable Alpha
payload limit is 1,048,572 bytes. Source,
table, arena, fixup, and payload writes are checked before mutation. No output
byte is published until every fixup and the complete payload extent validate.

Those extents describe the `AlphaBootstrapV2` profile selected by D23 and
migrated with the seeds, Gamma compiler, checker, outcome tables, and exact-limit
gates. The measured 251,142-byte fixed frontend no longer has to fit the retired
V1 ceiling. The current consolidated 206-case gate remains mandatory, and
ordinary density improvements remain useful rather than a release gate.

The retained front end's four-word syntax nodes retain the exact
zero-based source start in the high bits of their tag word; the 4 MiB source
ceiling and closed tags make that packing exact without reducing AST capacity.
Its first source failure is sticky across the outer byte envelope, parsing,
literal checking, type-name resolution, and typed subexpression traversal.
This is coordinate custody for later absorption, not an oracle-owned diagnostic
format: the final compiler maps it through `DCOUT`'s fixed rejection table and
publication boundary. D19's selected generated-program application profile is
a separate concern.

Closed keyword, builtin, and builtin-type recognition shares one exact
packed-ASCII matcher rather than five dedicated procedures and an unrolled
`bytes_*` suffix tree. The matcher still checks the following identifier
boundary; focused controls keep builtin and keyword prefixes available as
ordinary user spellings. This is compiler-size engineering, not a lexical rule.
Top-level `data` lookahead now uses that same bounded matcher rather than a
second hand-unrolled spelling check. Declared type and constructor names also
share one predicate because D16 gives them identical capitalization and
builtin exclusions; D20 keeps their semantic namespaces separate.

Static match coverage now survives recursive body checking over the same
nominal type. After typing every arm once, the checker replays only the already-
resolved patterns under one fresh epoch before deciding duplicate and exhaustive
coverage; nested matches can no longer overwrite an outer match's live rows.
Equivalent pattern-failure states were merged so this repair does not revise
Gamma's fixed global-edge profile. The adjacent gate pins the same-type nested
case directly.

D20 global collection now rejects the exact later duplicate independently in
the type, constructor, and function namespaces before resolving declaration
types. One bounded open-addressed pass per namespace reuses the match-coverage
scratch, verifies every hash collision with exact source spelling, and retains
the earliest conflicting source offset across namespaces. This avoids the
quadratic declaration scan exposed by the 32,768-function capacity canary.
The active local environment rejects a parameter, `let`, pattern field, or
catch-all binder before mutation when its spelling is already active; existing
push/pop boundaries preserve initializer, sibling-branch, and sibling-arm
scope. Checked ordinary calls, constructor applications, and constructor
patterns retain their exact one-based function or constructor table identity
directly in the source AST; zero remains reserved for unresolved/builtin nodes.
The same lexical environment now assigns source-order parameter indexes and
fixed local slots to parameters, lets, pattern fields, catch-alls, and every
variable reference. Disjoint scopes reuse local slots, while each function
retains its return type, maximum live-local count, and exact parameter count in
one packed profile word. That 16-bit field admits at most 65,535 simultaneously
live locals; active parameters may exhaust the shared lexical environment
earlier. Attempting the adjacent local fails through the private resource state
before publishing an environment/AST slot or a wrapped profile. Source
variables, lets, ordinary calls, constructor applications, and selected matches
consume those identities directly in `lower_expr`.

Whole-function emission validates every retained function row and exact
parameter spine, preflights label capacity, and publishes every source-order
function label before emitting any body. It then defines bodies in source order,
installs each packed frame profile, lowers the body in tail position, and emits
the common return-frame epilogue. D19 remains the separate owner of profile
selection, the generated PC-zero application adapter, result framing, and final
publication.

D19 schema admission now resolves the exact `main` signature for both profiles.
For `EpsilonCompilerV1` it additionally validates the two source-owned nominal
types, the exact `Complete`/`Reject` and D31/D34 storage-refusal field lists,
all 26 nullary rejection constructors, and the fixed code bijection without using
declaration order or runtime constructor kinds. D30 fixes the physical sealed
request, exact profile maxima, Conformance observations, and complete
`DCOUT`/`ECOUT` tables. `admit_dcreq` now implements D33's bounded request half:
it obtains and validates the fixed header, selects profile 1 or 2, refuses an
oversized declaration before body work, requires the exact admitted body and
EOF, and only then validates Delta source bytes. It retains the selected
profile and private outcome kind/code/space/coordinate/limit/requested fields.

The ordinary frontend retains every closed rejection class from
`invalid_syntax` (4) through `nonexhaustive_match` (18) at its decisive Delta
source coordinate. Frontend exhaustion retains resource codes 2 through 9;
the emitter projects label, fixup, and payload exhaustion to codes 10 through
12 and its present metadata, label/fixup, and replay contradictions to internal
classes 2 through 4. The remaining semantic work is to retain and order schema
codes 19/20/21, add internal classes 1/5/6 with their eventual
producers, check all embedded tables, emit the two PC-zero adapters, and publish
no canonical artifact until the projections and exact/adjacent gates agree.

## D30 physical application profiles

`DCREQ` V1 is `[44 43 52 45 51 01 00 00]`, followed by one little-endian
`u32` profile ID, one little-endian `u32` Delta-source length, the exact source
bytes, and exact EOF. Profile 1 is `ConformanceBytesV1`; profile 2 is
`EpsilonCompilerV1`. Both generated applications admit 4,194,304 input bytes.
Conformance admits 4,194,304 successful output bytes, while Epsilon compilation
admits AlphaBootstrapV2's 1,048,572-byte tape maximum. These application facts
do not derive from this compiler's separate 4-MiB source buffer.

The compiler edge uses `DCOUT` magic `[FF 44 43 4F 55 54 01 00]`; the
generated Epsilon-compiler edge uses `ECOUT` magic
`[FF 44 43 4F 55 54 01 00]`. Both retain the common 40-byte failure frame and
halt tags 0 through 3. `DCREQ` validation obtains the fixed header, validates
magic/version/reserved bytes and the profile, then enforces the selected
profile's declared source provision before reading the body and one exact-end
probe. Unknown profile anchors at request byte 8; source exhaustion anchors at
byte 12 with the declared length as `requested`; admitted body truncation or a
trailing byte is `malformed_request` at the first missing or extra request byte.
This bounded order prevents a four-byte length from forcing unprovisioned input
consumption. The retained compiler and focused gate implement this ingress,
including both profiles, first-missing/first-differing coordinates, the exact
and adjacent 4-MiB source boundary, and exact-end precedence over source-byte
diagnosis. DCOUT frame publication is deliberately not part of ingress.

After the ordinary frontend succeeds, schema reasons use category priority 19
missing entry, 20 wrong present entry, then 21 nominal profile schema. Missing
facts use coordinate space `none`; a wrong `main` anchors at its declaration
name; present malformed profile members anchor at their declaration or
constructor name. Within one category absence precedes located defects, then
located defects use their earliest coordinate. Code 21 is legal only for
`EpsilonCompilerV1`; `ConformanceBytesV1` has no additional nominal schema.

The embedded profile and outcome constants project exactly to:

- `profiles-v1.tsv`;
- `conformance-observations-v1.tsv`;
- `dcout-v1.tsv`; and
- `ecout-v1.tsv`.

The gate must compare every row rather than merely parse those files. They are
not inputs read by the completed offline compiler. `dcout-v1.tsv` deliberately
publishes authored semantic rejection classes instead of the frontend's private
parser states or its historical one-bit `INVALID_TYPE`. Its
`profile_context` column uses `unselected` before a valid profile exists and
otherwise enumerates the permitted profile IDs. The originating request and
frame are checked together; a detached DCOUT frame cannot validate this column
because it does not repeat the profile ID. `ecout-v1.tsv` retains D17 codes 1
through 26 unchanged.

These are the current D30/D33 contracts, not a presumption that the completed
bootstrap compiler should expose a multi-profile service and detailed public
diagnostic ABI. `BOOTSTRAP-MINIMAL-COMPILER-BOUNDARY` must settle that shape
before the production adapters are completed. The four Delta TSVs are then
deleted under `BOOTSTRAP-SIDECAR-RETIREMENT` unless a named non-test external
consumer requires a registry.

The compiler's exact V1 resource limits are:

| Resource | Last admitted extent |
| --- | ---: |
| Delta source | 4,194,304 bytes |
| Total type rows, including `Int` and `Bytes` | 65,536 |
| Constructor rows | 65,536 |
| Function rows | 32,768 |
| Active environment rows | 65,536 |
| Coverage rows | 65,536 |
| Syntax arena | 114,294,752 bytes / 3,571,711 nodes |
| Parse depth | 1,024 |
| Simultaneously live local slots | 65,535 |
| Labels | 65,536 |
| Fixups | 116,508 |
| Emitted runnable Alpha payload | 1,048,572 bytes |

Generated applications separately use a 15-MiB explicit Delta stack and a
112-MiB immutable heap. The shared generated-program observation block is 248
InternalFailure, 249 AuthoredTrap, 250 StackExhausted, 251
MemoryContainmentViolation, 252 HeapExhausted, 253 InputExtent, and 254
OutputExtent. Alpha's illegal-instruction trap remains 132; 255 is unassigned
and noncanonical. `interp.gamma` predates this block and its private statuses 252
through 255 remain oracle-only until that oracle is deleted.

An emitted Delta program uses this Alpha-memory profile:

```text
[0, 1 MiB)         canonical stamped Alpha tape region
[1 MiB, 16 MiB)    downward Delta activation/argument stack
[16 MiB, 128 MiB)  upward immutable value/Bytes heap
[128 MiB, 256 MiB) Alpha hidden-return-stack allowance
```

Every private limit is an outer resource profile, not a Delta validity rule.
The compiler measures the minimum generated frame and helper nesting needed to
keep the explicit stack, heap, tape, and hidden return stack disjoint.

Delta values use two words `(kind, payload)`: `Int` carries all signed 64 value
bits in `payload`; `Bytes` points to an immutable descriptor; and an algebraic
value uses its resolved constructor tag plus a pointer to two-word fields.
Nullary constructors allocate no field vector. Functions return through
`r0/r1`. Arbitrary-arity arguments occupy two-word slots in the explicit Delta
stack rather than Gamma's four argument registers.

The executed algebraic substrate copies already-resolved constructor arguments
from that stack into source-order immutable field vectors and checks private
field-pointer extent/alignment before loads. Odd field counts round to a
32-byte heap row, preserving the compact `Bytes` descriptor alignment across
mixed allocations. The adjacent probe covers 600 fields, nested and nullary
values, malformed private pointers, and the exact heap edge. Source tag 7 maps
its retained one-based constructor table identity to the disjoint runtime kind
range starting at two, then uses the resolved-constructor seam and the call
seam's single guarded argument routine rather than retaining a second list
verifier. Its focused failure discriminator also keeps a malformed child
lowering distinct from successful zero arity. Selected-match lowering uses the
same identity-to-kind map and the already-retained pattern slots. It validates
the complete forward arm and field metadata before lowering the scrutinee,
tests constructor kinds in source order, copies fields into fixed frame slots
in source order, and retains the complete value pair for a catch-all. The
executed bridge limits the heap to one algebraic row, proving a nonnull
scrutinee is evaluated exactly once; it also pins nullary and payload arms,
sibling slot reuse, unselected traps, catch-all rematching, proper-tail arm
calls, malformed metadata with zero payload, sticky failure, and byte-identical
reconstruction.

Generated code treats `r249`/`r250` as caller-clobbered fixed-offset address
scratch, reserves `r252` for the downward stack pointer, `r253` for the current
frame base, `r254` for the upward heap pointer, and `r255` for the heap limit.
Central emitter helpers own the scratch pair so repeated field/frame accesses
do not duplicate address-building logic or tape. Each frame retains the previous
base, caller's pre-argument cursor,
fixed local slots, and two-word parameters at its high end. A mandatory 16-byte
header bounds live Alpha return addresses before hidden-stack/heap overlap;
tail relocation preflights its complete target and copies pairs high-to-low
before mutation. The runtime initializer fixes the canonical endpoints. Directly
emitted heap and stack reservation helpers reject negative, overflowed, and
adjacent-out-of-range requests before mutation and transfer to one
caller-supplied terminal failure label. The adjacent gate executes the emitted
Alpha payload at both exact boundaries and one byte beyond each, and separately
checks negative and arithmetic-wrap requests; no helper relies on Alpha's
undefined out-of-range memory behavior.

Directly emitted signed-add, subtract, multiply, divide, and remainder helpers
use a private scalar ABI through `r0` and transfer every arithmetic overflow,
zero divisor, and `INT64_MIN / -1` case to the supplied terminal failure label
before Alpha can trap. General lowering moves that scalar into the `Int` payload
in `r1` and restores kind `0` in `r0`. Their executed probe covers ordinary
negative division/remainder, both
addition and subtraction overflow directions, multiplication overflow and the
valid `INT64_MIN * 1` edge, and both exceptional division/remainder classes.

The retained `lower_expr(expr, tail_position)` dispatcher currently consumes
already checked trees with literals, variables, lets, ordinary calls,
constructor applications, all seven primitive operators, `if`, and the six
closed `Bytes` forms. It emits nested evaluation left-to-right, spills
intermediates through the guarded explicit stack, calls the checked helpers,
and reconstructs `(kind,payload)` results. Conditions lower non-tail; both arms inherit the
caller's tail position before call lowering gives that bit executable behavior.
Condition lowering evaluates once and branches before either arm, so an
unselected trap-bearing arm remains unexecuted. Its adjacent gate feeds real
Delta declarations through the canonical parser and type checker, executes 31
emitted Alpha tapes for arithmetic, comparisons, nested and spill-surrounded
conditionals, every `Bytes` operation, balanced stack restoration, and
contained failures, then checks two repeated raw payloads byte-for-byte. This
is general expression-dispatch material; no partial Delta compiler or subset
artifact is published.

Resolved local and let lowering reuse the fixed-frame ABI. Source tag 1 decodes
the retained parameter/local kind and zero-based runtime index; source tag 4
converts the retained one-based slot identity into the current function's fixed
frame profile. A local loads one complete pair from that slot. A let
prevalidates that slot before emission, lowers its initializer non-tail exactly
once, retains the pair, and lowers its body with the incoming tail-position bit.
The focused executable bridge carries a `Bytes` initializer through a real
48-byte frame, reads it from the source-resolved local in a `bytes_get`,
restores the root frame, rejects malformed prefix/index profiles with zero
payload, and reconstructs the same tape twice. The ABI makes no additional
source-spelling or scope choice.

Ordinary calls use Alpha `call`/`ret`. A tail call first evaluates arguments
exactly once from left to right into temporary stack slots, relocates them
overlap-safely into the replacement frame, restores the original caller frame,
and jumps to the callee. The retained resolved-call seam now consumes the
canonical source-order argument list, spills complete values, and selects that
ordinary or tail transfer from an opaque callee label and fixed frame prefix.
It prevalidates the forward arena list and every fixed extent before emitting
argument code; malformed private metadata cannot loop, wrap frame arithmetic,
or leave a partial candidate payload.
Its compact executed payload carries mixed `Bytes`/`Int` arguments through both
paths from a source tag-5 node, reads both parameters through the guarded
resolved accessor and source tag-1 node, and returns
with the root stack and frame restored. `if`, resolved lets, and every selected
match arm preserve their incoming tail-position bit, so terminating tail
recursion grows neither Delta activations nor Alpha's hidden return stack. The source connection
uses the retained one-based function identity, the compiler-owned label table,
and the callee's packed maximum-live-local profile. The two-phase whole-function
emitter populates the complete table before lowering any caller, so forward,
mutual, and self calls share the same path. No resolved AST is serialized or
executed.

`Bytes` uses a compact immutable rope/view representation with closed descriptor
kinds `EMPTY`, `LEAF(pointer,length)`, `CONCAT(left,right,total_length)`, and
`SLICE(base,start,length)`. Concatenation is constant-time after checked length
addition. Under D21 that addition operates on stored logical lengths and traps
before allocation when the exact sum exceeds `INT64_MAX`; successful descriptors
store the exact sum. The focused runtime probe doubles a valid singleton rope
to logical length `2^62`, then distinguishes the adjacent overflow trap from a
malformed-descriptor internal failure and actual heap exhaustion; every failure
retains the pre-operation heap cursor. Slicing validates the complete signed range; indexed access
descends iteratively. An application adapter preflights an entire returned rope and
output extent before replaying it to stdout, preventing partial artifacts.

The dormant sealed-input reader is reusable emitted runtime machinery, not an
application adapter or a profile choice. It accepts a compiler-supplied maximum,
reads stdin exactly once into a flat `LEAF`, and atomically commits its aligned
descriptor and heap cursor only after EOF and full extent validation. Input
extent and heap extent transfer to distinct adapter-owned terminals while
containment remains independently compiler-owned. Its seven ordinary runtime
paths plus one malformed-private-heap path pin empty and binary input, exact
and adjacent source/heap limits, zero capacity, unchanged heap publication on
either resource failure, internal containment, no output, and byte-identical
reconstruction. D19 now fixes the
two closed profiles, their selected entries, result validation, and wire
ownership, while D30 fixes their exact maxima and physical boundaries, D31
adds the checked application-static-storage resource, and D34 fixes its
bounded witness and deterministic attribution. Adapter
emission remains implementation work.

The direct emitter owns byte/word append, label definition, and
`{payload_offset,label_id}` fixup rows. Branch and call targets remain private
placeholders until all code exists; duplicate or missing labels and out-of-range
targets are sticky internal failures. After resolution, an independent replay
partitions the complete payload under Alpha's closed instruction widths and
requires every encoded direct target to land on its rebuilt instruction-start
map. Generated Delta tapes are instruction-only: jump-skipped inline data is not
an alternate payload path. Exact source-to-tape refinement remains open. Checked `Int` lowering
branches explicitly around overflow, division-by-zero, signed-division-overflow,
and invalid byte/range operations so required diagnostic publication never
depends on falling into an uncatchable Alpha trap.

D20's declaration/binder resolver, source joins, and profile-neutral
whole-function label/body emission are implemented. D19's exact source-owned
schemas and reason-code bijection, D30's physical profile facts, and D33's
canonical request ingress are implemented. Ordinary rejection codes 3 through
18 and compiler-resource codes 1 through 12 are retained with their exact
coordinates and quantitative fields; emitter-internal classes 2 through 4 are
also retained. Schema diagnosis, remaining internal classes,
both adapters, and final publication still gate the tape. No incomplete slice
authorizes a subset compiler or blocks the settled parser, private target ABI,
runtime helpers, direct emitter, or profile-independent lowering described
above.

D58's final measurement occurs only after every item above is present in this
canonical source. The current 965-row call measurement and adjacent refusal are
retained as a pre-completion baseline, not promoted into a guessed 2,048-row
published profile. Emitter-plan consolidation may proceed when it independently
reduces source and proof complexity; it cannot replace the measurement or hide
adapter behavior outside Gamma source.

Any future validation placed here must reconstruct the exact
Gamma-source-to-Alpha-tape edge for `delta_compiler.gamma`. Generic evidence,
external interpreter execution, and host-side source lowering do not belong in
this owner.

The implementation order is tracked in
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).

## Retention inventory

| Retained file | Canonical role | Deletion condition |
| --- | --- | --- |
| `delta_compiler.gamma` | Sole Gamma-written Delta compiler source; currently owns the strict frontend and direct Alpha payload/fixup substrate. | Replace only atomically with another implementation of the same ruled edge. |
| `profiles-v1.tsv` | Current projection of DCREQ profile IDs, limits, and entries. | Delete if the selected bootstrap boundary has one fixed production entry; otherwise replace atomically with the selected profile contract. |
| `dcout-v1.tsv` | Current Delta-compiler diagnostic projection. | Delete if detailed codes have no named external consumer; otherwise replace atomically with the selected wire contract. |
| `ecout-v1.tsv` | Current generated Epsilon-compiler diagnostic projection. | Delete if detailed codes have no named external consumer; otherwise replace atomically with the selected adapter contract. |
| `conformance-observations-v1.tsv` | Current generated-program observation mapping shared by the profile. | Move the retained semantic facts into execution semantics and delete the sidecar unless a named external consumer requires it. |

## Deletion condition

Delete any future file or child subtree that does not reconstruct or implement
`delta_compiler.gamma → delta_compiler_bytecode.tape`;
replace this owner only atomically with a changed, explicitly ruled topology.
