# Optimization implementation

The [public optimization contract](../wiki/spec/build/optimizations.md)
defines exact selection, identity phases, semantic preservation, rollback and
policy authority. This page says where the code starts; it is no rule inventory. The [pipeline architecture](pipeline.md#discoverability-architecture)
governs source layout; it sets no file-size limits.

The [exact-rule inventory](omega/foundation/optimization-core/rules.md)
records each rule's experimental status, applicability and composition limits.

## Selection and portable publication

[Build-value admission](omega/packages/package-compilation/src/build_execution/build_value/optimization.rs)
accepts the exact names. Shared Build declarations put the
[optimization vocabulary](omega/packages/build-declarations/src/prelude/optimizations.rs)
into the toolchain prelude from one mapping. The
[checked result](omega/compilation/products/src/checked_compilation.rs)
keeps the whole evaluated Build, including selection and report requests.
[Rollback settlement](omega/foundation/optimization-core/src/rollback.rs)
validates exact names, subtracts rolled-back rules and produces the receipt.
The compiler uses the result before producing artifacts, without
changing the authored selection; product-specific phase admission stays in
the compiler. No compiler-local optimizer dispatcher wraps the transform.

The [Psi X-to-X entrance](psi/compilation/pipeline/07_lowered-psi-to-lowered-psi/src/lib.rs)
consumes complete unsealed `LoweredPsi`, records the carrier's Terminal Psi
identity before and after the selected passes, and returns the only
optimization-stage result accepted by Terminal publication. Its
[Psi-local vocabulary](psi/foundation/optimization/src/lib.rs)
does not import native target vocabulary and names nine members. The
[entrance's exhaustive dispatch](psi/compilation/pipeline/07_lowered-psi-to-lowered-psi/src/psi_optimization.rs)
executes five of them here, in canonical order: control-flow cleanup, sparse
conditional constant propagation, copy propagation, global value numbering,
and dead pure scalar elimination. Proof-check elision, state specialization,
representation specialization, and loop-invariant scalar motion have empty
arms in that dispatch; they execute in the post-Terminal
[rule catalog](omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/rules.rs).
The execution record retains the exact selection, including members that run
no rule in this stage. This stage runs no separate per-selection before/after
verifier: the corpus observes each pass's rewrite, following
[verification placement](pipeline.md#verification-placement).
Copy propagation collapses a scalar block parameter bound to the same
resolved value on every inventoried incoming
edge, substituting the resolved source at direct scalar uses and dropping the
matching edge-argument positions. Global value numbering removes an
unconditionally-total scalar operation that repeats a surviving operation with
the same kind and resolved operands in a dominating position — earlier in its
own block or in a block that dominates it — and substitutes the canonical
survivor, the first match in reverse postorder, at every direct scalar use.
Sparse conditional constant propagation folds a goal-free scalar leaf
operation whose scalar operands all resolve to known literals into the
matching integer or Boolean constant in place, iterating to a fixed point
while keeping every operation, result, and value identity; ranked machines
and static reach bindings stay frozen; inside a proof-bearing module a
rewrite survives only while the reconstructed proof question is unchanged —
a rewrite the question cannot carry verbatim refuses, leaving the complete
closure as authored until proof-context transport lets the question move
with it — and propositions, proof projections, suspension frontiers, ranking
evidence, and recorded source-call joins keep the exact identities they name.
In the post-Terminal stage, proof-check elision removes a checked operation
only when it carries an accepted obligation fact and either its result is unused
and the operation is pure, or it matches a cataloged exact scalar identity
(such as identity, divide-by-one, multiply-by-zero, self subtraction, or
remainder by one); any other checked operation keeps its check. The
execution record survives canonical Terminal encoding and independent decoding.

The [post-Terminal abstract phase](omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/lib.rs)
builds verified input, runs within a work budget and publishes an
independent projection. Its result exposes the current abstract program and
replay evidence, not the running session or analysis cache. Old `Psi` names
in that crate's rule code do not let a later lowerer rerun pre-Terminal
passes. The compiler reads only the post-Terminal selection and keeps the
whole selection attached.

Structural-call contract validation keeps unrestricted write-only subloans
through exact field/literal-index paths from mutable or write-only parameter
roots. Indexing needs a material root and primitive/record leaf; an existing
reference ABI does not grant read access. Empty-selection publication keeps
the complete abstract plan, not just a matching entry signature.
`WRITE-ONLY-BORROW` in [TASKS.md](../TASKS.md) holds the acceptance command
and remaining stack-pointer transport and platform coverage.

Abstract publication owns the read-only current plan apart from the receipt
and stored replay inputs. Replay checks the exact schedule, candidate
declarations, applied/skipped outcome, commits, analyses and facts used, and
baseline-policy/cost evidence. A decoder, report or edited decision log
cannot supply missing authority for a transformation.

## One physical sequence

The [native pipeline](omega/compilation/native-compiler/src/pipeline.rs)
checks the stored selection, then runs instruction selection, selected
X-to-X rewrites, allocation, machine construction and canonical frame
realization once. Required lowering is not optional optimization.

It first rejects selections no implemented stage serves, without repeating
stage-owned rule admission. A selected-lowering catalog entry does
not imply native publication support, and retired post-allocation rewrite
names cannot choose another emitter. [Selected optimization](omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/selected_optimization.rs),
[allocation](omega/compilation/pipeline/05_selected-instructions-to-register-homes/src/register_allocation.rs),
and [layout optimization](omega/compilation/pipeline/09_resolved-layout-to-resolved-layout/src/phase.rs)
admit their own exact catalog selections when their phase runs; errors
return through the stage result. The exact executable catalogs, not old
tables, decide applicability and composition.

The current selected program and allocation are kept apart from the
evidence of how they were produced. Later stages use current facts; only
replay reads stored history. Do not add a program representation or
emission path per rule, fixture shape or optimization-history combination.

[Allocation](omega/compilation/pipeline/05_selected-instructions-to-register-homes/README.md)
owns register homes and pressure recovery.
[Machine emission](omega/compilation/backend/machine-emission/README.md) owns
frame realization, fragment emission and text placement. Target ISA crates
own instruction alternatives, effects, encoders and decoders. Object
construction and callable/image admission happen later; an encoding receipt
does not replace their checks.

## Catalogs and independent replay

A stage that owns rules has one exact ordered catalog, a small runner, and
separate proposal and validation code. A selected catalog entry holds the
rule, validator, policy, analyses, invalidations, budgets and applicability;
coordinators must not repeat its name-to-implementation mapping. Each pass
may order its own rules, but a family map below it is not a second place to
enable them.

Analyses and candidates are tied to the current revision and exact fact
identities. Committing a rewrite needs an independent rebuild, a result
check, and evidence that each analysis was invalidated or still holds.
Neutral models and canonical encoding may be shared; a validator must not
call the producer's matcher, rewrite helper or fact-producing algorithm to
vouch for its result. Shared predicates and primitives are not shared
output-deciding code. Shared code belongs with the nearest real semantic
owner, not under whichever sibling rule came first.

The bounded countdown code shows an implementation restriction, not a
language limit. Its
[ranked-cycle validation](omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/ranked_cycles_validation.rs)
rebuilds the component and ranking evidence. Only verified guard-zero and
decrement-one relocation to the canonical preheader suffix may change the
otherwise frozen component; provenance and work accounting stay unchanged.
The separate ranked-rewrite step must still authorize and validate it.
The analysis alone neither runs a rewrite nor permits general loop motion.

Ordinary Natural-ranked and unranked cycles instead keep exact verified
source components and freeze the whole cyclic function, prefixes and exits
included. Unranked safety checking gives no ranking certificate or fixed
work bound. Raw cyclic units are not enough input; legalization replays the
same source-bound topology before ordinary native selection.

Keep candidate building, policy choice and independent validation separate.
The Rust reference compiler uses deterministic policies; discrete candidates
and explicit decision records leave room for a future learned policy in the
Omega-written product compiler. This constrains structure; it is not a Rust
training, corpus, evaluation or model-integration project. Cost estimates rank choices; neither they nor a future model
can replace semantic, translation or publication evidence.

## Validation when extending a stage

Test exact selection and empty identity execution; wrong target,
unsupported composition, malformed source and stale evidence; one-field and
cross-rule corruption; deterministic order and each exhausted work budget;
and real downstream replay. A codec round trip or one applied-rule test does
not show the compiler applies or publishes the rule. Keep positive,
refusal, corruption and compatibility fixtures beside the rule or boundary
they test. Use differential execution only on really supported target/host
paths, and tell two related artifacts apart from a same-artifact end-to-end
comparison.

## Operational rollback

Leave `build.omg` unchanged and pass the repeatable, case-sensitive flag
once per affected rule. This command works in PowerShell and POSIX shells:

```text
omega --disable-optimization ControlFlowCleanup --target linux_x86_64 main.omg
```

Check target and composition support before blaming a rule. Unknown or
duplicate names are rejected; a known unselected name is a visible no-op.
Check-only requests reject rollback. Programmatic Terminal publication
supports only its Psi-phase overlay; native publication settles the phases
it uses. With an empty effective selection, every path runs the same
identity stages.

After successful CLI publication, capture the printed receipt with the build log:

```text
optimizer rollback: requested=[ControlFlowCleanup]; applied=[ControlFlowCleanup]; effective=[]
```

`applied` is the authored/requested intersection; `effective` is authored
minus requested. Check that every rule meant for rollback is in `applied`
and absent from `effective`. An unselected rule never appears in `applied`.
The in-memory report keeps the receipt; the CLI writes no receipt file yet. An empty effective selection must match ordinary identity-phase
semantics and artifact output, apart from the recorded rollback.

Before deploying, run the affected target/workload checks and keep the
command, log, compiler identity, target and output digest. Drop the
rollback flag only after semantic/corruption, differential, determinism,
target, measurement and rollback tests pass; do not replace
exact selection with a broad optimization level. [Promotion evidence](omega/foundation/optimization-core/README.md#exact-rule-promotion-records)
remains a separate owner-reviewed requirement.
