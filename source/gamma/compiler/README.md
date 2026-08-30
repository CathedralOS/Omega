# Gamma compiler owner

The canonical compiler owned here accepts Gamma, is implemented in Beta, and
emits platform-independent Alpha tape:

```text
gamma_compiler.beta → gamma_compiler_bytecode.tape
```

The source now exists as an incomplete implementation; the tape does not. Its
retained frontend is the former standalone checker, moved rather than copied,
and its direct Alpha payload/label/fixup substrate is final compiler material.
The adjacent 204-case gate compiles this one source with temporary fixed test
entries, retains all 97 frontend discriminators, and executes the emitter,
runtime containment, frame/value ABI, checked `Int`, compact `Bytes`, sealed
input, resolved-expression, selected-match, and whole-function paths. One
full-source test emitter now covers ordinary and proper-tail calls,
constructors, locals, matches, and general expressions instead of rebuilding
five redundant compiler variants. It also pins malformed whole-function
metadata and exact/adjacent label capacity before payload mutation. Six D19
schema cases cover both profiles, reversed reason declaration order, and
missing/malformed outcome rows without emitting an adapter. The gate publishes
no compiler artifact.

The retained compiler source declares 115 procedures. With the fixed frontend
gate entry, the compiled gate uses 116 of Beta's 256 procedure slots and
compiles to 329,015 bytes. The remaining 719,557 bytes under
Alpha's runnable payload ceiling are a measured implementation budget, not a
Gamma language limit or evidence that every remaining compiler component fits.

`../interp.beta` remains a bounded execution oracle. It may contribute an
isolated lowering/runtime algorithm where economical, but no interpreter loop,
serialized Gamma AST, or second frontend enters the canonical compiler.

The compiler uses a private arbitrary-arity frame ABI and preserves proper tail
calls. Its emitted compiler-application adapter alone supplies sealed input as
Gamma `Bytes`, invokes the typed `main`, and serializes exact success or the
accepted-language edge's failure frame. Fuel and private storage ceilings yield
outer resource outcomes; they never change Gamma meaning.

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

The compiler source is growing one pipeline inside `gamma_compiler.beta`:

```text
sealed source
  -> strict parse and declaration collection
  -> type resolution and checked typed IR
  -> direct Alpha lowering and fixups
  -> complete private payload validation
  -> one publication
```

The compile-time AST/IR is never serialized into the emitted program. No Gamma
evaluator loop, syntax-tag dispatcher, textual Alpha stream, or host-side
assembler participates in the edge.

Under `AlphaBootstrapV2`, the Beta executable has exactly 128 MiB of
source-visible logical raw memory. The frontend
keeps sealed source in `[2 MiB,6 MiB)`, declaration and environment tables in
`[6 MiB,10.5 MiB)`, labels/fixups below 13 MiB, payload instruction starts in
`[126 MiB,127 MiB)`, and AST storage in `[16 MiB,125 MiB)`. The final
`[127 MiB,128 MiB)` is the private one-MiB payload buffer; the runnable Alpha
payload limit is 1,048,572 bytes. Source,
table, arena, fixup, and payload writes are checked before mutation. No output
byte is published until every fixup and the complete payload extent validate.

Those extents describe the `AlphaBootstrapV2` profile selected by D23 and
migrated with the seeds, Beta compiler, checker, outcome tables, and exact-limit
gates. The measured 251,142-byte fixed frontend no longer has to fit the retired
V1 ceiling. The current consolidated 204-case gate remains mandatory, and
ordinary density improvements remain useful rather than a release gate.

The retained front end's four-word syntax nodes retain the exact
zero-based source start in the high bits of their tag word; the 4 MiB source
ceiling and closed tags make that packing exact without reducing AST capacity.
Its first source failure is sticky across the outer byte envelope, parsing,
literal checking, type-name resolution, and typed subexpression traversal.
This is coordinate custody for later absorption, not an oracle-owned diagnostic
format: the final compiler maps it through `GCOUT`'s fixed rejection table and
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
For `DeltaCompilerV1` it additionally validates the two source-owned nominal
types, the exact `Complete`/`Reject` field lists, all 26 nullary rejection
constructors, and the fixed code bijection without using declaration order or
runtime constructor kinds. The physical sealed request, exact profile maxima,
Conformance observations, and complete `GCOUT`/`DCOUT` tables remain
owner-blocked by Q3; no canonical adapter bytes are emitted until those facts
are fixed.

An emitted Gamma program uses this Alpha-memory profile:

```text
[0, 1 MiB)         canonical stamped Alpha tape region
[1 MiB, 16 MiB)    downward Gamma activation/argument stack
[16 MiB, 128 MiB)  upward immutable value/Bytes heap
[128 MiB, 256 MiB) Alpha hidden-return-stack allowance
```

Every private limit is an outer resource profile, not a Gamma validity rule.
The compiler measures the minimum generated frame and helper nesting needed to
keep the explicit stack, heap, tape, and hidden return stack disjoint.

Gamma values use two words `(kind, payload)`: `Int` carries all signed 64 value
bits in `payload`; `Bytes` points to an immutable descriptor; and an algebraic
value uses its resolved constructor tag plus a pointer to two-word fields.
Nullary constructors allocate no field vector. Functions return through
`r0/r1`. Arbitrary-arity arguments occupy two-word slots in the explicit Gamma
stack rather than Beta's four argument registers.

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
Gamma declarations through the canonical parser and type checker, executes 31
emitted Alpha tapes for arithmetic, comparisons, nested and spill-surrounded
conditionals, every `Bytes` operation, balanced stack restoration, and
contained failures, then checks two repeated raw payloads byte-for-byte. This
is general expression-dispatch material; no partial Gamma compiler or subset
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
recursion grows neither Gamma activations nor Alpha's hidden return stack. The source connection
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
descriptor and heap cursor only after EOF and full extent validation. Its seven
ordinary runtime paths plus one malformed-private-heap path pin empty and
binary input, exact and adjacent source/heap limits, zero capacity, internal
containment, no output, and byte-identical reconstruction. D19 now fixes the
two closed profiles, their selected entries, result validation, and wire
ownership; exact profile maxima and adapter emission remain implementation
work.

The direct emitter owns byte/word append, label definition, and
`{payload_offset,label_id}` fixup rows. Branch and call targets remain private
placeholders until all code exists; duplicate or missing labels and out-of-range
targets are sticky internal failures. After resolution, an independent replay
partitions the complete payload under Alpha's closed instruction widths and
requires every encoded direct target to land on its rebuilt instruction-start
map. Generated Gamma tapes are instruction-only: jump-skipped inline data is not
an alternate payload path. Exact source-to-tape refinement remains open. Checked `Int` lowering
branches explicitly around overflow, division-by-zero, signed-division-overflow,
and invalid byte/range operations so required diagnostic publication never
depends on falling into an uncatchable Alpha trap.

D20's declaration/binder resolver, source joins, and profile-neutral
whole-function label/body emission are implemented. D19's exact source-owned
schemas and reason-code bijection are implemented; Q3's physical profile facts,
both adapters, and final publication still gate the tape. No incomplete slice
authorizes a subset compiler or blocks the settled parser, private target ABI,
runtime helpers, direct emitter, or profile-independent lowering described
above.

Any future validation placed here must reconstruct the exact
Beta-source-to-Alpha-tape edge for `gamma_compiler.beta`. Generic evidence,
external interpreter execution, and host-side source lowering do not belong in
this owner.

The implementation order is tracked in
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).

## Retention inventory

| Retained file | Canonical role | Deletion condition |
| --- | --- | --- |
| `gamma_compiler.beta` | Sole Beta-written Gamma compiler source; currently owns the strict frontend and direct Alpha payload/fixup substrate. | Replace only atomically with another implementation of the same ruled edge. |
| `test-frontend.sh` | Adjacent bounded gate for the retained frontend, source/resource guards, exact emitter substrate, and executed runtime-containment payloads. | Delete or reduce when exact source-to-tape validation subsumes every named discriminator. |

## Deletion condition

Delete any future file or child subtree that does not reconstruct, implement,
or efficiently test `gamma_compiler.beta → gamma_compiler_bytecode.tape`;
replace this owner only atomically with a changed, explicitly ruled topology.
