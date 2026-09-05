# Retained Delta syntax

[parse.gamma](parse.gamma) retains balanced source forms.
[grammar.gamma](grammar.gamma) checks their grammatical roles and returns the
same program on success. Neither phase resolves declarations or types, checks
duplicates, or emits output.

## Representation

[nodes.gamma](nodes.gamma) owns the shared representation and its accessors:

```text
node    = (pair start (pair after (pair child-count children)))
program = (pair top-count top-nodes)
```

`start` and `after` are exact source coordinates, with `after` exclusive. An
atom has child-count `-1` and children `0`. A list has a nonnegative count and
an ordered raw pair spine of child nodes. The program uses the same counted
spine for top-level forms. Counts govern projections; no branch or integer
comparison inspects pair-reference provenance. Names remain exact source
spans, not copied strings or hashes.

The parser runs one tail-driven loop with an explicit immutable stack. Each
frame retains the opening coordinate, parent count, reversed parent children,
and previous stack. Closing a list restores its children to source order once
and appends the completed node to its parent. Nested Delta forms therefore do
not consume one Gamma return context per source nesting level during parsing.

## Syntax storage

[storage.gamma](storage.gamma) owns D30's 114,294,752-byte syntax provision.
This producer charges its actual syntax objects at the selected Gamma
evaluator's 40 bytes per immutable pair. It does not charge a historical
32-byte typed-syntax row for a differently represented S-expression node.

Usage is cumulative: discarded reversed spines and popped parser or grammar
frames are not reclaimed by this evaluator. A scalar travels with parser and
grammar work; restoring a parent scope does not restore an older byte count.
The parser returns a private `(program, used)` carrier, and grammar consumes
it before returning the unchanged ordinary program to census. Phase outcomes
and this custody carrier are not syntax objects. Declaration catalogs,
environments, lowering plans, normalization, and emitted text are separately
owned and do not enter this syntax ledger.

Each allocation group is preflighted before any member is allocated:

| Parser operation | Pairs | Refusal coordinate |
| --- | ---: | --- |
| Open a list | 3 parser-frame pairs | Opening delimiter |
| Retain an atom | 3 node pairs + 1 reversed-spine cell | Atom start |
| Close a list | Child count ordered-spine cells + 3 node pairs + 1 parent-spine cell | Saved opening delimiter |
| Finish the program | Top-level count ordered-spine cells + 1 program root | Source extent |

Grammar charges four pairs for each pending batch. A function's initial three
batches are one 12-pair group, and a let's two batches are one eight-pair
group. Other sites allocate one batch. Shape checks precede those allocation
sites; later role judgments do not run after a refusal. A group anchors at the
construct whose work it schedules; a sibling-sequence batch anchors at the
current child. Successful declaration outcomes carry the updated scalar usage
into the next declaration, so completed work does not reset the ledger.

An over-limit request produces halt/tag 2, resource code 7, source coordinate
space 1, the coordinate above, limit 114,294,752, and the exact cumulative
bytes requested for the complete group. At 40-byte granularity, literal
equality with the limit is impossible; its last 32 bytes cannot hold a pair.
Requested bytes are not rounded down or replaced with `limit + 1`.
All counts derive from admitted source extents or fixed object shapes, and
their products and sums fit Gamma's signed scalar range.

The shared `syntax_reverse_nodes` helper also serves lowering and normalization.
Only its parser calls are charged here; the helper itself does not impose
syntax accounting on unrelated phases. This ledger is not a query of Gamma's
shared heap and cannot relabel physical evaluator exhaustion as DCOUT.

## Grammar and diagnostics

The pipeline completes source-envelope and lexical admission before parsing.
The complete balanced parse precedes grammatical-role checking; grammar
precedes global identity collection. A later unmatched delimiter can therefore
preempt an earlier invalid grammatical role. This is explicit phase ordering,
not a promise to select the globally smallest coordinate across phases.

The parser reports stray closing delimiters and unexpected source end through
syntax code 4. Grammar reports an offending node at its start, a missing
required child at its enclosing closing delimiter, and an extra child at that
child's start. A program without a required function reports source end.
Within a declaration, enclosing shape checks precede its queued children;
queued roles run in source order before the next declaration.

The grammar entrance coordinates a counted pending-work spine instead of
recursively checking nested expressions. Count and spine travel as separate
machine arguments. A pending batch is
`(pair role (pair depth (pair remaining (pair nodes rest))))`; `nodes` reuses
the parser's ordered child spine. The current role runs directly, and only
pending siblings or later roles allocate frames. Successful roles tail-dispatch
the next work without allocating `Complete(work)` results, replacement work
wrappers, or reversed child lists. Shape probes use a private scalar result:
`-1` means success, otherwise the value is the exact rejection coordinate.
Only the completed declaration returns an outcome to the program coordinator;
the public grammar entrance still returns the original program or its failure.
Atom-role predicates also expose their scalar success or anchored failure to
the arm coordinator. A bare constructor pattern or one-child parenthesized
pattern can finish its name-role judgment before directly dispatching its body,
without a pending-body frame. Other patterns retain the binder worklist.
The direct path still enters the ordinary expression-role dispatcher, including
its depth provision; it does not bypass body grammar or change error ordering.
Nonexpression roles use depth zero; expression batches preserve expression
levels and inherited match-arm levels.
A function body starts at level 1; expression children
advance by one, including atoms. Match arm bodies use their enclosing match's
level plus one, without counting arm or pattern wrappers. Declarations,
parameters, and patterns do not themselves consume expression levels.

Before an expression's grammar judgment, level 1,025 produces D30
`Incomplete(parse_depth)`: halt/tag 2, resource code 8, Delta-source space 1,
the expression's start, limit 1,024, and requested 1,025. Balanced parsing still
completes first, so the retained node already exists when the level is checked.
Pending frames may also already exist; the limit does not preflight their allocation.
Earlier pending grammar failures therefore keep their order,
and the limit check precedes the over-limit node's own grammar failure.
The subordinate files own these roles:

| Owner | Roles |
| --- | --- |
| [work.gamma](grammar/work.gamma) | Worklist operations; value-name, nominal-name, and type atoms |
| [declarations.gamma](grammar/declarations.gamma) | Program order, data/function shapes, parameter lists and parameters, constructor declarations |
| [expressions.gamma](grammar/expressions.gamma) | Expression atoms, heads, `if`, annotated `let`, `match`, and applications |
| [patterns.gamma](grammar/patterns.gamma) | Two-child match arms and constructor patterns |

Names obey the grammar's capitalization and reserved-word rules. Type atoms
admit `Int`, `Bytes`, or a syntactically valid nominal name; whether a nominal
declaration exists is a later question. Pattern binders are validated as names,
without checking conflicts or resolving constructor payloads.

Ordinary functions, constructors, arithmetic operators, `eq`, `lt`, and the
closed `bytes_*` forms retain variable argument counts here. Their semantic
arity checks remain with body checking; this phase does not turn those failures
into syntax code 4. Bare arithmetic tokens are not expression atoms. Constructor
patterns likewise retain their authored binder counts for later arity checks.

## Consumers and remaining work

The retained program is consumed, not discarded after validation:

- Global collection traverses declaration and constructor nodes, preserving
  exact source owners, counts, and declaration-order tags. Raw function rows
  retain their function node.
- Declaration resolution consumes retained parameter, result-type, and
  constructor-field nodes against the completed global census. Its typed
  metadata keeps the downstream format. Unknown declaration types produce
  code 11 at that node's start; repeated parameters produce code 9 at the later
  name. Declarations, constructors, and fields are visited in authored order.
  Each parameter's conflict check precedes its own annotation, all parameters
  precede the result, and all declarations precede body checking. Complete or
  failure outcomes propagate through these retained-node traversals.
- Body checking consumes retained expression and pattern children, with exact
  names and type coordinates carried through its complete/failure outcomes.
  It starts from each function's resolved parameter environment and preserves
  immutable local environments across disjoint scopes. Explicit continuations
  retain pending child results and environments; visits and resumptions are
  tail transitions rather than recursive source walks. Constructor metadata
  retains ordered resolved field types, so calls and patterns do not rescan
  payload annotations.
- Lowering traverses retained declarations, expressions, and pattern children.
  Its shared visit/resume machine uses explicit continuation payloads instead
  of recursive source-coordinate walks. It completes an expanded Gamma plan
  with binding identities and expression-list heights. Atom spans still supply
  exact authored names and admitted numeric text; emission prints that plan
  without selecting Delta lowering rules.

The [emitter](../../emission/README.md) remains after the complete static
preflight, as required by
[D114](../../../../../../wiki/architecture/bootstrap_chain/decisions.md#d114--delta-emission-consumes-the-completed-static-preflight).
Other compiler-owned resource/internal failure propagation and successful
generated-program admission throughout the Delta depth profile remain separate
work. The selected Gamma evaluator caps generated bodies at 255 nested lists;
lowering may add wrappers. A separate
[normalization pass](../../normalization/README.md) uses the recorded expanded
heights to extract over-height fragments. That body-height transform does not
close the separate helper-count, runtime-context, storage, or compiler-owned
resource/internal boundaries. The `parse_depth` refusal is not evidence of
those later properties.

The [boundary contract](../../boundary/README.md#body-traversal-and-coordinates)
records body diagnostic order and coordinates. Declaration parameters check
conflicts before their annotations; body `let` checks its annotation first and
checks the initializer in the outer environment. Calls consume expected
arguments in order, and matches defer final coverage until every arm succeeds.

The complete grammar worklist also precedes census `function_rows` provision.
That later logical count admits 32,768 fresh authored functions, with duplicate
lookup before each provision; it does not count syntax nodes or generated
Gamma helpers. Its refusal is not a syntax error or a syntax-arena measurement.

The explicit parser stack and grammar worklist use ordinary Gamma pairs.
Gamma's immutable arena accounts for cumulative allocations and does not
reclaim abandoned construction spines. The selected 1,024-level compiler
profile is not a Delta language nesting limit or a new Gamma primitive.
Selected evaluator exhaustion remains an evaluator-owned failure; expression
depth accounting does not translate it into guessed compiler resource codes or
establish full Delta resource conformance. Syntax-byte provision has its own
source-owned accounting above; it does not account for every compiler phase.
