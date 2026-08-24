# Design Brief: Build-Time Evaluation — Const Evaluation + Trait Generators

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

Structured proof/static values used by future indexed domains evaluate in this
same sealed world. Their eligibility is narrower than general evaluator output:
each index value must have decidable structural equality and one unique
canonical encoding. The resulting canonical value, not the evaluation trace,
enters a closed domain's semantic identity. Open symbolic index expressions
remain normalized artifact data rather than arbitrary evaluator programs.
Until quotient-backed canonical rationals land, a `Rat` used in an index must
also prove at that site that its denominator is positive, its signed
coordinates are cancelled, and its numerator magnitude and denominator are
gcd-reduced.

Canonical compile-time values retain one atom across generic,
monomorphized, and structural identity. Eligible values are integers, Booleans,
fixed arrays, records, and cases; declaration order determines aggregate
identity. Floating/text values, references, slices, dynamic identities, and
boundary-opaque data are not canonical generic atoms. Indexed qualification and
constrained calls preserve the same closed atom or const-binder identity.
Quotients and constrained records fail closed unless their canonical
representative and required facts are proved at the use site. Arbitrary machine
evaluation never participates in type equality.

Target equivalence is mandatory. Build-time floating-point operations use the
same executable `FloatSemantics` meanings as runtime operations, including
format-specific rounding, classification, conversions, square root, and fused
versus unfused arithmetic. A target-specific realization may refine raw NaN
bits, but the portable promise is equality of `FloatMeaning`; observing
computed NaN payload bits requires a proof or selected realization that fixes
them. Cache identity includes every selected realization and semantic control
state that can affect the result.

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

Escaping mutation is excluded by the semantic-evaluation bridge itself. Every
compiler-materialized argument is converted into a fresh owned interpreter
value graph, the machine instance is freshly instantiated for the invocation,
and the only value crossing back is a recursive value snapshot. Interpreter
cells and references cannot cross that boundary. Local mutation that computes
the returned value is therefore legal, while no mutation can reach compiler
state, another invocation, or runtime state. Augmenting `build.omg` evaluation
uses a separate API that deliberately returns snapshots of its mutated
arguments and is not this hermetic world.

Until proof/build-admissible resource inputs have a sealed admission artifact,
the common gate rejects every declared linear runtime carrier in a reachable
checked body: attached machine data, machine-owned data, state and callable
parameters/results, and local bindings. The judgment uses typed structural
multiplicity, not type names. This is a monotone fail-closed input/resource
rung; it does not claim that the remaining authority, trust, or
resource-admission artifacts exist.

Pre-check evaluation currently has no concrete-argument proof context. The
common gate therefore rejects an authored `requires` premise anywhere in the
reachable machine/callable closure rather than assuming it before the checker
has discharged it. This is the first fail-closed trust-input rung. A later
invocation-sensitive gate may admit the call by supplying the ordinary checked
proof of that exact premise; it must not add a build-time-only proof rule.

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
    fuel_units;
    logical_words_processed;
    aggregate_elements_constructed;
    peak_live_cells;
    result_cells;
}
```

`fuel_units` is ultimately charged by the canonical Terminal Psi fuel schedule
in [`canonical_ir_fuel_and_resource_provisioning.md`](canonical_ir_fuel_and_resource_provisioning.md).
The remaining counts are attributed telemetry rather than interchangeable work
currencies. The evaluator's local schedule charges one unit for each entered
state, executed statement, and evaluated expression. Semantic evaluation and
ordinary interpreted outcomes retain the resulting usage, and equal
invocations reproduce it. This telemetry is not Terminal Psi fuel and cannot
support an IR fixed-work certificate.

Compiler build-machine evaluation retains that precursor usage alongside the
computed build configuration in checked and full compilation reports. It does
not enter `BuildConfig`, terminal semantics, or artifact identity. Once build
machines lower through terminal Psi, the canonical schedule replaces this
precursor count rather than being inferred from it.

Host observation is retained separately from this meter. The granted evaluator
reports whether the filesystem host family was actually invoked; the compiler
joins that fact to the exact statically reachable toolchain service. The
current scoped real-filesystem provider has no replay transcript, so reachable
or realized filesystem use is conservatively `Volatile`. A pure or console-only
build is `Hermetic`, and console-only execution is not supplied real filesystem
authority. Both statement- and value-position dispatch require an exact
canonical toolchain filesystem requirement symbol before the provider is
entered; a package-authored lookalike remains an ordinary unsupported call even
under granted execution. The selected canonical signature then maps to a
closed, explicitly tagged operation identity exhaustively handled by both
filesystem providers. ABI aliases remain distinct. Future rooted transcripts
must account for conditionally absolute `read_link` results and unconditionally
absolute `canonicalize` and `final_path_name_by_handle` results. This Boolean
rung is not a receipt and makes no replayability or source-rebuildability claim.

The usage record carries a schema identity independently from evaluator-step
identity: adding telemetry does not change what one step means. It records
`result_cells` for successful semantic evaluation.
Each returned scalar, unit, text, or aggregate root contributes one cell, and
aggregate fields, case payload values, and array elements contribute their
recursive cell counts. Text byte volume belongs to logical-work telemetry, so
it does not inflate the retained-cell count. Augmenting-machine results sum the
cells in every returned argument. The evaluator computes this count with
checked arithmetic and rejects accounting overflow rather than publishing a
partial record. `logical_words_processed`,
`aggregate_elements_constructed`, and `peak_live_cells` still require execution
and allocator instrumentation.

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
A fixed-IR certificate may remove runtime fuel metering, but its scalar does
not predict the target's worst-cycle path.

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
projects the retained published proposition term and never reruns that search.
Cheap artifact/kernel verification may remain.

That is separate compilation of proof evidence, not an evaluator-cache escape
hatch. A local cache substitutes for work still belonging to the current build
graph; published evidence moves the proved contract into the producer's
artifact.

## Implementation boundary

The reference interpreter already serves constant, domain, layout, wire, and
calling-policy positions under deterministic termination, reach,
suspension/blocking, snapshot, and invocation-specific crash checks. `TASKS.md`
owns the remaining semantic capsule, result/usage identity split, telemetry,
progress policy, const parameters, recursive evaluation, and generator
expansion. This brief defines their contract; tests and Git history record which
individual sites have landed.

## Trait-body and reflection rules

- A trait machine body is its overridable default; there is no `default`
  keyword.
- Reflection iterates only over the conforming `Self` inside its own machine
  body. It grants no visibility, boundary access, compiler access, type
  descriptor, or type-as-value capability. Build-time evaluation still runs on
  admitted values in the sealed semantic world.
- Generated equality follows the current field set. Hand-written equality owns
  its own coverage and does not become substitutable equality; quotient laws
  remain the explicit route for that claim.
- Record destructuring is exhaustive: each field is bound, renamed with `as`,
  or visibly waived with `as _`. It is snapshot sugar for ordinary field `let`
  bindings, supports only copy-eligible implicit bindings, and introduces no
  reference patterns or binding modes. Larger fields are waived or borrowed by
  an explicit ordinary expression.
- Body-shape coverage is not a value proposition and is not exposed as a
  requirable fact. A separate trait-level coverage gate remains deferred unless
  real drift demonstrates a need for code-effect tracking.
- The generator-body unroll syntax remains unsettled. Combiner examples such as
  `all(self.[field] == other.[field])` are illustrative, not approved syntax.
