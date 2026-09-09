# Gamma body-height normalization

Start at [program.gamma](program.gamma). Its `normalize_program` entrance sits
between complete [Gamma lowering](../lowering/README.md) and
[serialization](../emission/README.md). It consumes and produces the same
[Gamma program representation](../representation/README.md). It does not parse
Delta, repeat frontend judgments, or write receipt bytes.

[expressions.gamma](expressions.gamma) coordinates height budgets and the common
visit/resume machine. [arguments.gamma](arguments.gamma) and
[bindings.gamma](bindings.gamma) retain pending call and let children.
[helpers.gamma](helpers.gamma) owns extraction and generated definitions;
[capture.gamma](capture.gamma) coordinates its independent scoped rewrite.

The selected Gamma evaluator admits at most 255 nested expression lists per
function body. Lowering can exceed that height even when Delta syntax stays
within its separate 1,024-level expression-depth profile. Normalization moves
whole over-height fragments into generated functions, retaining their position
in the original expression's evaluation.

## Height budget

`normalization_height_limit` in [program.gamma](program.gamma) owns the selected
255-list budget. Each function body starts with that budget. A subtree whose
recorded height fits the remaining budget is reused unchanged. Otherwise traversal
descends through its Gamma call or let structure with one less level available
to each expression child.

Rebuilt nodes refresh both expanded height and canonical byte extent through
the ordinary Gamma constructors. Capture renaming therefore cannot retain a
stale spelling width, and extracted helper calls retain their own extents.
Reused immutable nodes preserve both summaries.

At budget one, an over-height fragment reserves a helper name and restarts
normalization under a fresh budget of 255. Descendant helpers finish first.
Capture then renames the completed, height-bounded body, including arguments
to those descendant helpers, before appending its definition. The replacement
call passes only already-bound values. Those arguments have height zero, so
the replacement call has height one. The initial program can also contain lowering's
shared projection definition for wide constructors. Its height-three body
already fits; it is not an extraction helper or an authored Delta function.

No work is moved into a sibling expression or before an enclosing initializer.
The helper call occupies the extracted fragment's exact evaluation position.
Branches remain conditional, earlier arguments still precede later arguments,
and authored computations and traps remain inside the fragment. A call that
replaces a tail-position fragment remains in tail position; the fragment's
original result becomes the helper's result.

### Height and completion argument

The [representation constructors](../representation/gamma/expressions.gamma)
assign atoms height zero and calls/lets `1 + max(child heights)`.
The [expression serializer](../emission/expressions.gamma) emits precisely that
expression-list nesting; declaration and parameter-list wrappers are outside
the body. Immutable reuse preserves the summary, and capture changes atom
identities without changing call/let shape.

These height additions cannot overflow for admitted source. Let `N` be source
bytes, at most 4,194,304. There are at most `N` authored expression starts on
any path. Lowering adds at most one level for an ordinary call/let, at most
`N` for a constructor product, seven for arithmetic, and `4 + 3*N` for a
match: three wrapper lets, at most `N` arm selectors plus their comparison
level, and at most `2*N` pattern lets and field projections. Each is bounded
by `16*N` for nonempty source, giving the deliberately loose bound
`16*N*N <= 2^48`, below signed
64-bit overflow. Shared projection prefixes do not increase path height.

For each visit, order `(height, 255 - budget)` lexicographically. Descending
to an expression child decreases height. Extraction at budget one retains the
original fragment but restarts at budget 255, decreasing the second component.
Finite argument lists and pending frames account for the remaining
visits. Thus the traversal terminates in an unbounded-resource model. By the
same induction, each returned body fits its requested budget: a reused node
already fits, rebuilt children fit budget minus one, and an extracted call has
height one. A helper body is normalized before its definition is appended,
so the bound covers all generated helpers, not only authored functions. Capture
after normalization changes binding identities, not expression-list structure,
and therefore preserves the completed body's height.

This is a source-level audit argument, not a machine-checked certificate or
a guarantee that the selected evaluator has enough allocation/work resources
to finish every admitted compilation. Byte-extent arithmetic is a separate
summary and is not bounded by this height argument.

## Captured bindings

A helper receives only the fragment's free, already-bound local atoms.
Bindings introduced inside the fragment are not captures; global function
names, fixed primitive names, and constants are not local captures either.
Capture discovery follows explicit binding identity rather than equal source
spellings or numeric pair provenance.

Capture lookup checks an existing free-binding mapping before scanning the
local-bound spine. Within a produced function, each identity has one lexical
binder and every reference resolves to that binder. An identity free in an
extracted subtree therefore cannot become bound later in that subtree. Source
identities use distinct declaration starts; lowering-generated markers separate
arithmetic and match binders; extraction uses fresh `c` identities and preserves
internal binders. Saved scopes keep sibling and initializer bindings separate.
The projection helper is a separate definition and capture never follows global
call heads. Shared expression nodes preserve their owning lexical scope.

This invariant permits immediate reuse on a mapping hit. A miss still checks
local binding before making a new mapping. Discovery order, parameter identities,
and generated bytes are unchanged. The rule is not valid for arbitrary synthetic
trees that reuse one identity for a free occurrence and an unrelated local binder.
It adds no index or retained state. Bound references now attempt the mapping
lookup too (including its temporary option pair); the measured benefit is less
repeated name comparison, not a universal allocation or runtime bound.

[capture/bindings.gamma](capture/bindings.gamma) compares a source binding's
declaration start, or a generated binding's marker and identity number, in
distinct categories. It records each free binding once, in first-occurrence
order. [capture/lets.gamma](capture/lets.gamma) excludes a let's binder from its
initializer scope and includes it only in its body.
[capture/calls.gamma](capture/calls.gamma) visits arguments in order with the
same surrounding scope. [capture/result.gamma](capture/result.gamma) returns
matching parameter and argument lists without reordering their mapping.

Each captured value receives a fresh helper-parameter identity. References in
the helper body are rewritten to those parameters, while call arguments retain
the original bindings at the extraction site. This avoids accidental capture
and Gamma's active-local name conflicts without copying or evaluating a
captured value's initializer again. Pair-bearing values flow through ordinary
Gamma arguments with their existing provenance.

Helper names use `$hN`; capture parameters use `$cN`. Both allocate identities
from one program-wide counter. Helper names are allocated in extraction order,
while completed helper definitions follow the authored definitions in
deterministic completion order. A helper extracted inside another helper can
therefore precede it in the definition list without changing either identity.

### Capture allocation ownership

Capturing before splitting repeatedly rebuilds the still-oversized descendant
body. The [full-width source control](../../../../tests/delta/normalization/README.md#full-width-allocation-control)
exposes why that ordering cannot cover the selected Delta profile: one parameter
and 65,535 pattern binders exactly fill the active-local provision. Its source
and syntax storage fit their independent provisions.

For a chain of 65,535 binding lets, each fresh 255-level budget can descend at
most 254 let-body edges before extraction. Ignoring enclosing wrappers only
weakens the bound. The first 257 extractions therefore recapture at least
`sum(j=1..257, 65,535 - 254*j) = 8,421,633` lets. Each rebuild allocates two
payload pairs and four node pairs through `gamma_let` and `gamma_node`:
50,529,798 pairs, already beyond the selected 40,265,318-pair arena before
counting capture frames, initializers, checking, or lowering. This is a
source-level allocation lower bound, not a measured exhaustion observation.

Splitting first removes that repeated descendant rebuild. Each helper captures
its own completed body and references to deeper helpers, without traversing
their definitions. This does not establish a whole-producer linear bound:
height-bounded bodies can still contain broad arguments and large capture sets,
and all preceding phases still consume cumulative storage. It adds no allocator,
resource ledger, representation, or profile.

### Static validation-environment bound

The selected Delta frontend permits at most 65,536 simultaneously active
parameters, let bindings, and pattern bindings. Lowering preserves their scopes.
Only [checked arithmetic](../lowering/arithmetic.gamma) and
[matches](../lowering/matches.gamma) add local binders: three per arithmetic
expression, three per payload match, or one per nullary match. Products, calls,
and field projections introduce no binders; pattern lets correspond to authored
bindings. Each active generated wrapper belongs to an expression on one source
nesting path, whose admitted depth is at most 1,024. Thus an unnormalized body's
active environment is conservatively bounded by
`65,536 + 3 * 1,024 = 68,608` bindings.

Extraction preserves that bound. Its distinct parameters replace a subset of
the bindings already active outside the fragment; they do not accompany a
second copy of that environment. Bindings introduced inside the fragment retain
their scopes and are not parameters. This mapping is injective and composes
through nested extraction. An inner helper captures original bindings before
the outer helper is closed; outer capture rewrites its call arguments, not its
global head or completed definition. Sibling scopes and let initializers do not retain
bindings introduced only in another child.

Gamma validates each function independently, restoring the environment after
each let and resetting it between functions. Every authored or extracted body
therefore fits its 131,072-row validation environment. This is a conservative
source-level audit argument, not a claim that 68,608 is attainable or a checked
edge certificate. It does not bound aggregate bindings during non-tail runtime
calls. Fixed runtime and adapter definitions are separate from the transform
and must fit independently. Lowering's shared projection definition likewise
fits independently with two parameters and no local binders.

## Phase and receipt boundaries

`prepare_admitted_source` continues to return the pre-normalization plan.
The private lowering-height diagnostic therefore observes the same expanded
heights, including heights above 255. Normal compilation calls
`normalize_program` before `emit_checked_program` publishes any receipt bytes.
The fixed byte helpers and profile adapters retain their existing text.

If every authored body already fits 255, no helper or rewritten subtree is
needed and the existing receipt stays byte-identical. Over-height programs
receive generated definitions as part of the same ordinary Gamma program;
this is not another evaluator, runtime representation, or application profile.

## Remaining resources

Body-height normalization does not establish complete Gamma-profile admission.
Generated helpers are ordinary declarations. The evaluator's function table
fits every source admitted by its request extent; payload preflight therefore
also bounds generated function storage. Calls
outside tail position can add live contexts to its separate 256-context limit,
and plan construction, captures, and execution consume finite immutable storage.
The transform does not introduce a new language refusal, increase a selected
profile bound, or manufacture compiler-owned resource evidence.

Exact body heights, capture behavior, evaluation and trap order, existing
receipts, and actual Epsilon reconstruction remain validation obligations.
The [normalization gate](../../../../tests/delta/normalization/README.md)
compares production-plan observations with authored expectations, then compiles
and executes the generated Gamma. The separate
[lowering-plan gate](../../../../tests/delta/lowering-plan/README.md)
retains its pre-normalization measurements.
Compiler-owned resource/internal DCOUT publication and the complete Delta edge
remain open even when every generated body satisfies the nesting bound.
