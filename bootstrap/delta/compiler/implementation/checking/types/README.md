# Retained-node expression typing

The entrance [types.gamma](../types.gamma) checks function bodies in retained
declaration order, after the entire declaration-resolution phase succeeds.
It checks each retained function/body coordinate against its typed metadata,
then returns `Complete` or the unchanged source failure. Profile validation and
emission run only after every body accepts.

This is the body checker, not an additional source preflight. It replaces the
recursive source-coordinate checker and its cursor/type result pairs. Grammar
has already established node roles and structural child positions. Source reads
here inspect retained atom spans for exact names and builtin classification;
they do not reconstruct expression or pattern boundaries.

## Control and ownership

[expressions.gamma](expressions.gamma) owns two mutually tail-calling operations:

```text
typing_visit(node, locals, depth, frames, globals)
typing_resume(type, depth, frames, globals)
```

A continuation is `(pair kind (pair payload previous))`. Depth governs stack
projections, and each concept owns its payload. An empty continuation stack
returns `Complete(type)`. A source rejection returns its failure value directly;
ordinary visit/resume transitions do not allocate phase outcomes.

| Owner | Continuations and retained facts |
| --- | --- |
| [branches.gamma](branches.gamma) | `if` condition and true branch retain the node and outer locals; the false branch retains its node and the expected branch type |
| [bindings.gamma](bindings.gamma) | A let initializer retains its node, annotation type, body node, and extended locals; pattern binding retains separate outer and extended environment roots |
| [calls.gamma](calls.gamma) | Each argument retains the application, actual node spine/count, expected type spine/count, result type, and outer locals |
| [matches.gamma](matches.gamma) | A subject retains the match and outer locals; each arm body retains the remaining arms, subject owner, outer locals, coverage trie/count, preceding arm type, and constructor count |
| [identities.gamma](identities.gamma) | Builtin type identities and source-node rejection anchoring |

Local environments and coverage tries are immutable. An initializer sees the
outer environment, while its body sees the extended root. Parent continuations
already retain their own roots, so a let body needs no restoration frame.
Every match retains its own coverage trie; nested matches cannot overwrite an
outer match's coverage. Each arm begins with the saved outer locals, allowing
disjoint arms to reuse binder spellings.

Function signatures and resolved constructor metadata supply ordered type
spines. Calls and pattern binding consume those retained types without
re-resolving declaration annotations. Counts govern every type-spine projection.
Builtin signatures use the same argument-checking loop as ordinary calls.

## Diagnostic traversal

These are explicit compiler traversal rules. They are not a universal
smallest-source-offset order, and D33's schema-category priority does not rank
body diagnostics.

- Function bodies are checked in declaration order. A declared result mismatch
  is reported only after its body otherwise types successfully.
- A call resolves its head first. Applicable expected arguments are checked
  left to right, with each expression checked before its type comparison.
  Missing arguments reject when an expected argument is unavailable; extra
  arguments reject after the expected arguments have completed. Extra argument
  expressions are not typed. There is no upfront count comparison that
  preempts an earlier applicable argument failure.
- An `if` checks its condition, true branch, and false branch in order. The
  condition must be `Int`, and both branches must synthesize the same type.
- A body `let` resolves its annotation, checks the binder conflict, checks its
  initializer in the outer environment, compares the initializer's type, then
  checks its body. Declaration parameters retain their separate documented
  binder-before-own-annotation traversal.
- A match checks its subject, then each arm's constructor lookup, nominal
  owner, payload arity, duplicate case, binders, body, and arm-result agreement.
  Exhaustiveness is checked after all accepted arms. Pattern binders are
  visited in order: a name in the saved outer environment is an active conflict;
  otherwise a name already introduced by the same pattern is a duplicate binder.

Body failures use Reject tag 1, Delta-source coordinate space 1, and zero
resource fields:

| Code | Meaning | Anchor |
| --- | --- | --- |
| 9 | active local conflict | Later let or pattern binder name |
| 10 | duplicate pattern binder | Later repeated binder name in that pattern |
| 11 | unknown type | Let annotation token |
| 12 | unknown constructor | Constructor token in an atom, application, or pattern |
| 13 | unknown function | Application head token |
| 14 | unknown local | Value atom token |
| 15 | type mismatch | Offending argument, initializer, condition, false branch, later arm body, function body, or scrutinee; a wrong-owner pattern uses its constructor token |
| 16 | arity mismatch | Application start, bare constructor atom, or pattern start |
| 17 | duplicate match case | Later constructor token |
| 18 | nonexhaustive match | Match expression start |

Malformed retained metadata and impossible continuation states remain internal
assertions, not guessed source rejections. Generated Delta execution traps are
separate observations and are never remapped into these compiler diagnostics.

## Remaining boundaries

Explicit continuations remove source-nesting-dependent Gamma return contexts
from expression typing. Their ordinary immutable pair allocations still belong
to the selected Gamma evaluator's finite resources. This does not implement
compiler-owned resource accounting or canonical internal-failure publication.

The [retained-node emitter](../../emission/README.md) consumes the completed
typing judgment under
[D114](../../../../../../wiki/architecture/bootstrap_chain/decisions.md#d114--delta-emission-consumes-the-completed-static-preflight).
Its own explicit continuations remove recursive compiler-expression descent,
but generated Gamma bodies still face the selected evaluator's independent
255-list nesting bound. Exact receipts and the actual Epsilon customer remain
validation gates; retained-node typing and emission do not close the entire
Delta bootstrap edge or its resource/internal outcomes.
