# Design Brief: Build-Time Evaluation — Const Evaluation + Trait Generators

Current design as of 2026-07-27. This brief resolves former owner question 5.
Omega uses hermetic **semantic evaluation** for constants, proofs, plans, and
trait generators. It has no `comptime`, macro, or `#run` keyword.

Do not confuse this facility with [`build.omg`](build_and_package_model.md).
`build.omg` is build orchestration in a build-host world with explicitly
injected capabilities. Semantic evaluation is the compiler's target-semantic,
hermetic evaluator. They happen before runtime for different reasons and
have different admissibility and reproducibility contracts.

## Governing laws

> Evaluating an eligible runtime-capable machine during compilation produces
> exactly the value that executing the same machine, with the same arguments,
> selected conformances/providers, and target semantics, would produce on the
> target.

Evaluation changes *when*, never *what*. There is no `is_build_time()`
predicate or other phase observation. A proof-only machine may have no runtime
representation, but it follows the same core value semantics and cannot observe
that the compiler is evaluating it.

> The position requests evaluation. The concrete invocation's complete
> normalized contract decides whether evaluation is legal.

No declaration marker restates that judgment.

## Two pre-runtime execution worlds

| Property | Semantic evaluation | `build.omg` orchestration |
|---|---|---|
| Purpose | constants, proofs, plans, generators | dependencies, target selection, staging |
| World | selected target semantics | build-host services |
| Reach | hermetic | explicit scoped capabilities |
| External input | compiler-materialized values | provider operations and receipts |
| Result | value/evidence consumed by compilation | `Build`, staged artifacts, lock inputs |
| Reproducibility | semantic cache identity | operation ceiling + realized receipts |

`build.omg` may read files or use an admitted network provider. A proof machine
cannot. The handoff is explicit:

```text
build.omg observation
  -> staged value/artifact plus receipt
  -> recorded build input
  -> compiler materializes an ordinary value
  -> semantic evaluation consumes that value
```

There is no route from an ambient host observation directly into a proof,
layout, type, or constant.

## Target-semantic world

The evaluator process runs on the host but interprets the selected target's
Omega world. Its sealed semantic input includes:

- language/evaluator semantics version;
- target primitive semantics required by the program;
- selected conformance and provider-plan identities;
- normalized machine implementations; and
- explicit compiler-materialized arguments.

It cannot observe host filesystem, environment, clock, randomness, network,
pointer width, floating-point behavior, or process state. Target facts used as
ordinary data arrive as explicit values or selected requirement inputs. The
sealed target capsule is an evaluator/cache input, not a general source-visible
`BuildWorld`.

Target equivalence is an acceptance requirement. Build-time `f32`/`f64`
arithmetic evaluates the same executable `FloatSemantics` functions that define
the target operation contracts. Constant/runtime twin canaries cover rounding
boundaries, subnormals, overflow/underflow, signed zero, infinities, NaN
semantics, and fused-versus-unfused operations. Computing target `f32` through
host `f64` without an exact equivalence proof is a compiler bug.

The base promise is equality of `FloatMeaning`, not arbitrary NaN payload bits.
A build-time raw-bit observation of a computed possibly-NaN result requires
proof of non-NaN, canonicalization, or an exact raw-NaN refinement from the
selected target realization. Cache keys include that selected realization and
its semantic control-state identity wherever the refinement matters.

## Admission uses the complete invocation contract

An empty service-reach row is necessary and nowhere near sufficient.
Suspension, blocking, failure/control outcomes, authority, trust, escaping
mutation, and termination are independent contract axes.

The compiler specializes the selected machine contract at the concrete
arguments and available facts, then requires:

- ordinary checked termination;
- empty runtime service reach;
- no possible suspension or blocking;
- no unhandled failure, trap, or abort route for the demanded value;
- no runtime authority acquisition/consumption;
- no escaping runtime mutation; and
- only proof/build-admissible trust and resource inputs.

This is invocation-sensitive. A generally trap-capable `divide` may evaluate at
`divide(10, 2)` when the nonzero obligation is discharged. A position demanding
`divide(10, 0)` rejects before evaluation and names the undischarged route and
call chain. If the evaluator nevertheless reaches a forbidden terminal route,
it reports the trace as a checker/accepted-assumption consistency failure.
Handled result sums remain ordinary values.

Trait requirements pin the public floor. A conformance cannot grow an
incompatible axis unnoticed; it fails at the conformance declaration.

## Ordinary termination, not compile-time totality

Semantic evaluation requires the existing `terminates` guarantee. There is no
second compile-time completion concept:

```text
TerminationGuarantee = NoGuarantee | Terminates
```

An acyclic checked body supplies a local `Terminates` summary without source
annotation. A cyclic body supplies `terminates by ...`. An open or separately
compiled contract writes `terminates;` when consumers may rely on it. This is
the normal infer-when-closed, declare-when-open split.

Termination proves eventual completion, not affordability. A deliberately
hours-long terminating proof is legal.

## Deterministic work observation and optional policy

The evaluator maintains a deterministic usage record independent of host speed,
thread scheduling, optimization, and target cycle cost. The initial normalized
record should retain canonical counts rather than one permanently weighted
scalar:

```text
EvaluationUsage {
    evaluation_steps;
    logical_words_processed;
    aggregate_elements_constructed;
    peak_live_cells;
    result_cells;
}
```

This meter supports three policies without becoming program semantics:

1. live progress attribution;
2. deterministic large-work warnings; and
3. optional root-selected hard ceilings.

The root may raise or remove ceilings. Dependencies may publish expected usage
but cannot grant themselves more. Evaluation code cannot inspect remaining
work, branch on policy, catch exhaustion, or request an increase. Exhaustion is
a build-resource error, never divergence, a failed termination proof, or a
machine result.

Progress reporting names the stable invocation, sponsor package, accumulated
usage, and largest active call path. Wall-clock elapsed time may be displayed
but never affects admission or accounting. Parallel scheduling changes neither
the canonical counts nor aggregate verdict.

Runtime WCET and target instruction cost remain a different resource theory.

## Result caching and usage accounting

Semantic identity and accounting identity are separate:

```text
ResultKey =
    normalized implementation closure
  + arguments
  + selected conformances/providers
  + target semantic capsule
  + evaluator semantics version

UsageRecord =
    ResultKey
  + usage-schema version
  + canonical usage counts

PolicyCharge =
    interpret(UsageRecord, selected cost policy)
```

Changing accounting weights must not invalidate a result computed under
unchanged semantics. If a future policy needs counts an older usage schema did
not retain, usage may need remeasurement; the semantic result remains valid.

When a cache hit substitutes for an evaluation in the current build graph, it
receives the recorded logical charge. Warm and cold builds therefore make the
same hard-ceiling decision. Merely linking an already-built dependency does not
charge the historical cost of producing that artifact.

## Published evidence is not a consumer cache

An expensive producer-side proof may publish carrierless selected-conformance
evidence under the
[law-bearing relation model](law_bearing_relations_and_quotients.md). The
producer performs the witness search while building its artifact; a consumer
opens the published proposition contract and never reruns that search. Cheap
artifact/kernel verification may remain.

That is separate compilation of proof evidence, not an evaluator-cache escape
hatch. A local cache substitutes for work still belonging to the current build
graph; published evidence moves the proved contract into the producer's
artifact.

## Current state and implementation sequence

- Constant positions, const-generic leaves, machine-backed domain facts, and
  layout/wire/calling policy sites already use the reference interpreter.
- Canonical service reach plus recursive suspension/blocking summaries are
  checked. Authority, trust, termination, abnormal-outcome, resource, and
  escaping-mutation axes still need to complete the common admission floor.
- The implementation currently calls its positive normalized termination
  variant `EventualTerminal`; migrate snapshots, artifacts, diagnostics, and
  code to the settled `Terminates` vocabulary.
- Add the target semantic capsule and split semantic result keys from canonical
  usage records.
- Add constant/runtime target-equivalence canaries, with float operations as
  the sharpest customer.
- Add deterministic progress reporting before optional warning/ceiling policy.
- Generalize trait-generator expansion; migrate `Equatable` and `Hashable` off
  hand-written compiler synthesis.
- Complete const type parameters and `terminates by`-gated recursive
  proof/constant evaluation.

Touches: symbol-resolved const queries, the interpreter's semantic-evaluation
entry point, normalized contract admission, resolved-to-typed generator
expansion, validation of facts derived from results, cache/provenance artifacts,
and build-progress reporting.

## Trait-body and reflection rules

- **No `default` keyword.** A trait machine either has a body or it does not: body
  present = the default, conformance may override. Zero keywords. Trait-machine
  bodies use the ordinary `machine` spelling.
- **Same visibility, earlier clock.** The splice (`self.[field]`) grants nothing the code couldn't
  already touch — the expanded body runs as one of Self's own machines, and
  there is no iterate-all-types anywhere: splice only quantifies over Self,
  inside a trait Self CHOSE to conform to. Conformance is the consent.
  Build-time evaluation runs admitted machines on VALUES (no type tables, no
  compiler internals, no boundary I/O). Contrast
  recorded: proc macros (arbitrary build-time I/O over token streams), Zig
  comptime (type objects), UHT (external parser) are amplified contexts;
  ours is not one. Reflection = iteration + splice, NEVER descriptors —
  a FieldInfo carrying a type would be a type-as-value on the runtime side
  of the universe fence.
- **Equality tiers:** conformance-generated comparison follows field changes
  by construction because the splice quantifies over the current members. A
  hand-written `equals` wins and remains responsible for its own field
  coverage. The prover never treats custom equality as substitutable equality;
  quotient laws are the separate mechanism for that claim.
- **Record destructuring direction:** `let`-position record destructuring is
  new
  surface, wanted (it proved itself on the canonical-equals sample):
  `let Player { health, gold as g, cached_stats as _ } = self;` —
  bare name binds, `as` renames (colon REJECTED: `field: x` already means
  a type annotation — a pun; `->` REJECTED: saturated, and patterns live
  inside arms), `as _` waives (bind to nothing, visibly ignored).
  **Exhaustive by law**: every field bound or waived — the record twin of
  the landed no-silent-fall-through rule on sums; adding a field breaks
  every pattern that must now decide. Arm-position record binding shares
  the grammar. v1 binding semantics may restrict to [copy]-eligible
  fields (exactly Equatable's existing prerequisite list). Canonical
  hand-written equals opens with the exhaustive destructure — chapter
  convention, NOT enforced.
- **Parked**: a trait-level shape gate ("conformances must open with an
  exhaustive destructure") — held until real drift proves convention
  insufficient. The exhaustive<T>-as-requirable-fact idea was examined
  and rejected on the stratum boundary: our facts are about VALUES;
  "this body read every member" is a fact about CODE, and requiring it
  would need per-field read-effect tracking — the same boundary that
  killed rewrite-registration-as-magic and macros.
- **Binding semantics:** a destructure
  is pure SUGAR for field lets — `let Player { health, gold as g } = self;`
  is exactly `let health = self.health; let g = self.gold;` — and therefore
  inherits the landed let semantics wholesale: **snapshot** (the
  let-capture fix exists because folding across an intervening write was a
  miscompile; copy-vs-view is OBSERVABLE under mutation, not an
  optimization question). For scalars the "copy" is the load the body was
  about to do anyway; the backend folds it when nothing intervenes and
  materializes it exactly when the snapshot is load-bearing. Consequences:
  NO reference-patterns or binding modes, ever (Rust's match-ergonomics
  axis — their multi-year confusion generator — never opens; also
  `&self as Player` cannot mean pattern matching, `as` in expression
  position is the recast). Big fields cannot be silently copied: v1 binds
  [copy]-eligible fields only (exactly Equatable's prerequisite list);
  anything larger is waived (`as _`) or borrowed explicitly by hand
  (`let items: &[Color; 3] = &self.items;` — landed spelling).
- **Still open (the one undecided item):** the unroll spelling for
  generator bodies — the combiner form (`all(self.[field] ==
  other.[field])`) is recommended, unruled.
