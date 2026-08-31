# Delta rung

This directory owns the Delta language, its Gamma-written compiler, and
adjacent source-to-Alpha-tape validation.

[`LANGUAGE.md`](LANGUAGE.md) is the normative Delta v1 contract fixed by D17.
It is self-contained: a compiler, sample corpus, historical implementation, or
Omega document cannot amend Delta by acceptance.

D22, D36, and D51 fix deterministic declaration identity beneath that contract:
grammar-selected owner, machine, member, state, and local scopes; one pre-type
scoped duplicate census; no active local shadowing; legal disjoint-state and
member/local spelling reuse; free unqualified machines; mandatory receivers on
owner-qualified data machines; disjoint constructor and receiver-method
namespaces; and no authored machine bodies on boundary owners. D24 completes that census with transition-arm binder scope,
same-phase `DuplicateName`/`InvalidBoundary` ordering, and unique-owner
classification.
D31 completes type formation and D34 completes its realizability report:
positive array lengths, zero-field records, mixed-data rejection, exact
storage-only `u8` and `never`/view/`Console` placement, structural diagnostic
anchors, and profile-owned bounded-witness static-storage refusal.
D37 fixes body/control candidate dependency as a complete premise DAG,
including value/place/resultless/`never` joins and exact relational/projection
anchors. D52 fixes resultless machine/constructor arguments at their authored
argument-expression start and closes the distinct-reason coordinate audit.
D53 fixes five local block-exit effects, checks every state without reachability
analysis, and gives falloff and post-`never` constructs exact delimiter anchors.
D38 fixes `.as_slice` as a once-evaluated, allocation-free full view of a
place-valued fixed array; views and non-place array temporaries are not accepted
receivers.

## Canonical edges

```text
Gamma-written Delta compiler
  └─ gamma_compiler.tape ─▶ delta_compiler_bytecode.tape

Delta-written Omega compiler D
  └─ delta_compiler.tape ─▶ omega0_compiler_bytecode.tape
```

The first artifact accepts Delta. The second accepts Omega. They are different
compilers and must not both be called “the Delta compiler.”

## Contents and migration

- [`FEATURE_LEDGER.md`](FEATURE_LEDGER.md) records Delta feature rationale and
  change control.
- `compiler/` owns the in-progress `delta_compiler.gamma`, its eventual
  canonical Alpha tape, and refinement evidence. The retained source currently
  contains final compiler material through complete parsing, the D22/D24
  source-shaped identity census including D36's receiver restriction and the
  superseded case/machine collision registry, D31 structural type formation,
  the source-backed resolution catalog, ordered local-value resolution, exact scalar and
  aggregate value/place facts, one generalized callable ledger, direct
  qualified applications, grouped/unqualified machine applications and
  discarded postfix-statement category admission in settled non-continuation
  contexts, the superseded special receiver-scoped `self` carrier, named-data
  receiver applications, exact sealed-`Console` receiver applications, and
  explicit transition state applications with state/machine collision
  rejection and separate state completion custody, transition subject,
  resolved-case, complete payload-binder, and retained sum-coverage facts,
  settled field/index/slice projection failures, D37 scalar and argument-
  `never` category joins, let/assignment/assert and explicit-return relations,
  first-following-statement terminal flow, and symbolic Alpha encoding.
  D50 fixes bare-state-transfer spelling, D51 retires static qualified
  machines plus special `self` resolution, D52 fixes resultless-argument
  anchoring, and D53 fixes local block exits. Their implementation remains
  alongside final body/control checking, owner-blocked entry and transition-
  pattern/coverage diagnostics, the remaining D37 control/terminal
  premise DAG, D38 executable controls, storage realization/lowering,
  `main`, and publication are incomplete, so it exposes no compiler artifact
  yet. The
  current fact pass does enforce D38's contextual receiver/result relation and
  separate array-view extra-call rejection.

The superseded Beta-written Delta-to-Gamma bridge and Darwin-native publication
tree, including the restricted Delta-written Darwin compiler prototype, are
deleted. Git history is sufficient; no compatibility owner replaces them.
The associated 43-file corpus was also deleted: it had no runner and mixed
native-backend slices, retired proof scripts, demonstrations, and unresolved
language proposals. A compact positive/negative suite will be derived from D17
and owned by the real compiler edge instead of restoring that corpus.

## Boundaries

- Delta is independent of Omega even when spelling overlaps.
- The Delta compiler is written in Gamma and emits exact Alpha tape directly.
- Delta-written `D` implements full Omega and may generate a slow,
  conservatively lowered `omega₀` tape.
- All fixed capacities are source-visible bounds, explicit profile parameters,
  or private budgets whose exhaustion is `Incomplete` and publishes no tape.
- Shell and Python may invoke tests or stamp tapes. They may not parse, lower,
  manufacture semantic evidence, or become compiler stages.
- The Rust compiler remains a comparator, not a producer in the canonical
  sequence.

Active work is tracked in
[`../../TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md).

## Retention inventory

| Retained child | Canonical role | Deletion condition |
| --- | --- | --- |
| `compiler/` | The sole owner of the in-progress Gamma-written compiler accepting Delta and its exact Alpha-tape edge. | Replace only atomically with the admitted immediate-predecessor compiler edge. |

The root retains only the normative contract, its feature/change ledger, and
this owner map. Proposed programs without a compiler or a contract-derived test
role are not retained as tests.
