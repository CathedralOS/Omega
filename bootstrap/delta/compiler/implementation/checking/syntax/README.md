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

The grammar entrance coordinates a counted worklist instead of recursively
checking nested expressions. Each work entry pairs a role with a retained
node. The subordinate files own these roles:

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
- Body checking and emission traverse retained top-level forms. They skip
  data declarations without rescanning their bodies and verify that each
  source-based function traversal ends at its retained `after` coordinate.

Expression typing and expression emission still read checked source
coordinates; they have not yet migrated to retained child nodes. Emission
remains after the complete static preflight, as required by
[D114](../../../../../../wiki/architecture/bootstrap_chain/decisions.md#d114--delta-emission-consumes-the-completed-static-preflight).
Body-local annotation types and conflicts, unknown value names, semantic
arity/type/match diagnostics, and
later compiler resource/internal failure propagation remain separate work.

The explicit parser stack and grammar worklist use ordinary Gamma pairs.
Gamma's immutable arena accounts for cumulative allocations and does not
reclaim abandoned construction spines. This implementation adds no new language
nesting limit or Gamma primitive. Selected evaluator exhaustion remains an
evaluator-owned failure until compiler-owned resource accounting and canonical
DCOUT publication are implemented; retained syntax does not establish full
Delta resource conformance.
