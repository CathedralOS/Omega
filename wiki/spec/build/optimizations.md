# Optimization selection and validation

Optimization changes a realization while preserving its semantic contract.
This reference defines selection, transformation admission, and reporting;
it does not imply implementation of every rule, phase, or program shape.
[Psi](../terminal-psi/product.md) owns the portable boundary.

Optimization may rely on producer-established facts only for trusted input under
[Psi admission](../terminal-psi/product.md#publication-and-separate-consumption).
A supplied certificate or canonical encoding does not authorize that trust.
Interpreter containment does not authorize optimizing untrusted guest claims
into omitted checks or lowering them to native code.

## Exact selection

The build vocabulary exposes exact named selections through
`builder.optimizations.enable(...)`. The selection is duplicate-free,
canonical, versioned, and identity-bearing. Human report emission is a separate
explicit request. No target, environment variable, compiler default, build
mode, or broad `O1`/`O2`/`O3` level implicitly enables an optimization.

Every optimization has exactly one phase owner. The complete selection projects
into closed phase-local sets for checked trees, Psi, abstract operations, target
operations/instruction selection, pre-allocation, allocation recovery,
post-allocation, and resolved layout. A stage consumes only its projection; downstream coordinators must not
rescan the global selection to construct another schedule. Projection retains
the complete build-selection identity so evidence from different builds cannot
be silently recombined. Psi-local pass identities do not import target-specific
Omega optimization vocabulary.

Each stage has one ordered catalog. An exact entry binds its source name,
rule and validator identities, policy, required analyses, invalidations, work
budgets, and applicability. Target independence is explicit. Target-specific
selection checks applicability before dispatch and identifies the exact rule,
required architecture, and actual architecture on rejection. Independent rule
validation checks applicability again. Custody coordinators consume the selected
entry, not a duplicate name-to-rule table.

Unknown selection or encoding versions, unsupported targets, and unsupported
compositions reject rather than dropping a rule or guessing compatibility.
Catalog presence is not evidence of universal program-shape or publication
support. Mandatory normalization, legalization, allocation, and realization
remain necessary whether or not optional profitability rewrites are selected.

## Phase and product boundaries

An optimization normally consumes and produces the same representation. A new
representation requires a different semantic vocabulary, invariant set, or
product boundary; optimization history alone is not such a boundary.

Every phase receives its input and its exact phase selection, executes
selected rules in canonical order under explicit bounds, and returns one typed
stage result with execution evidence. Candidate ordering is deterministic;
fixed-point passes have explicit convergence and iteration budgets.

Each applied transformation and the final representation keep an independent
validator. Under [verification placement](../../../omega-rust/pipeline.md#verification-placement), set by the owner in
`b1423f5385d` (2026-10-04), the corpus runner and the tests run those
validators; the default compile path does not re-validate the input or output
that the same compile just produced. Explicit
[validator selection](command_line.md#independent-stage-validators) runs requested
applicable validators inside a compilation and reports their actual coverage;
it does not change optimization selection or required admission checks.

An empty selection is the same phase's identity execution: output equals input,
with no pass manifests or transformation records, and the validators accept it
as the identity. For Psi that identity is literal: no passes run, the
lowered-form check does not run a second time, and the default path is
unchanged ([optimization ordering](../terminal-psi/product.md#optimization-ordering)).
Empty selection is not a separate compiler or
publication route. Disabling every selected rule yields the same identity-phase
acceptance, diagnostics, and output as an initially empty selection, apart from
the separately retained rollback provenance.

Checked-tree pruning runs only after every declaration of the check's
[validation subjects](validation_subjects.md) and their required dependency
closure has been parsed, resolved, typed, and checked; deleting unreachable
code cannot make invalid source valid within that declared scope. A root selection that changes the published product retains its
own explicit identity, not one inferred from the remaining declarations.
Shared frontend work precedes target fan-out under the
[target-selection contract](configuration.md).

Terminal publication follows the Psi phase and seals the checked module as
immutable Terminal Psi
([optimization ordering](../terminal-psi/product.md#optimization-ordering)).
The order is structural elaboration, the lowered-form check of the
unoptimized program, the selected Psi passes in one canonical order, the same
check of the optimized program, then publication. The unoptimized check
decides source validity; a pass cannot make an invalid program valid. The
published module carries the verdict and evidence of the check that judged
it. Passes do not transport that evidence. An empty selection publishes the
unoptimized check's module unchanged.

Publication retains the exact Psi selection and the input and output semantic
identities in the
[optimization execution](../terminal-psi/encoding.md#optimization-execution)
record. Decoding checks the recorded output identity against the published
semantics. Those identities are not the lowered-form verdict. Proof identities
remain internal validation provenance. Empty execution may claim only equal
input and output identities. A receiving interpreter or native lowerer does
not rerun Psi passes or the lowered-form check.

Every Psi pass consumes and produces well-formed lowered Psi in the same
representation. The checker has no knowledge of which passes ran and has no
pass-specific or combination-specific branches or adapters. A pass whose
output the second check rejects is a compiler defect, not a source diagnostic.
The check does not establish behavior preservation. Per-pass validators and
Psi-versus-native agreement do that work, in the corpus runner and the tests,
not on the default compile path
([verification placement](../../../omega-rust/pipeline.md#verification-placement)).

A nonempty Psi selection still fails closed. Publication refuses with
`no product route yet: selected Psi optimizations have no accepted evidence transport`.
That diagnostic is the existing refusal. The withheld route is the second
check, and passes still do not transport the unoptimized verdict.

A target-constrained build companion may carry pending post-Terminal
selections, excluding earlier phases and bound to the complete build selection.
It is a proposal a receiving lowerer may refuse, not authority to execute a
transformation. A raw portable product leaves subsequent realization choices to
its receiver.

## Observations and resources

A [home-layout transformation](../layouts/plans.md#home-layout-realization) may
change physical storage while preserving the semantic contract. This includes
compact case encodings without a separate tag. Applicable layout commitments,
queries, recasts and aliasing observations constrain the change alongside ABI
contracts. All consumers must agree on the selected realization; a later pass
cannot invalidate an admitted cast, change an already-fixed layout observation,
or impose new source mutation restrictions to enable its encoding.

A transformation preserves every observation admitted by the program and its
selected boundary contracts:

- returned values, state transitions, and observable trap kinds and ordering;
- service/effect, atomic, volatile, placed-memory, and cleanup ordering;
- ABI-visible calls, arguments, results, clobbers, exit/unwind behavior, and
  externally visible storage;
- ownership, borrow, alias, and address-stability restrictions; and
- provenance and debug roots required by the selected reporting contract.

[Effects](../language/effects.md), [observations](../terminal-psi/observations.md),
[calls and outcomes](../terminal-psi/calls_and_outcomes.md), and
[ownership](../terminal-psi/ownership.md) define these meanings. An unused result
or empty reach alone does not prove purity. Purity is a closed classification
reconstructed from operation semantics. Calls, traps, services, atomics,
volatile/policy-governed accesses, cleanup, and transitions are barriers unless an exact
rule proves the proposed change preserves their contract.

Operation identity retains width, signedness, domain/provider, and arithmetic
policy. Exact operations retain the same discharged proof/check obligation;
wrapping, saturating, and trapping policies are not interchangeable. Shift-count
rules and fused versus unfused floating-point operations remain distinct. NaNs,
payloads, signed zero, infinities, and rounding follow the named operation.
There is no ambient fast-math switch. A lossy family would require a separately
accepted source-visible semantic contract declaring its observable differences,
exact rule identity, and tests.

Instruction count, code size, register pressure, compiler work, and elapsed time
are costs, not [logical work](../resources/logical_work.md). The Terminal work
schedule is external deterministic accounting; exhaustion is an incomplete
consumer outcome, not a source result, and native execution has no implicit work
meter. Fixed-work certificates use the immutable Terminal subject. Relocating a
source operation retains its exact work settlement and provenance at the new
realization site; mutable IR cannot invent loop charges. An evaluator of
optimized IR needs an explicit accounting contract before activation.

Final spill-inclusive frame demand remains bound to the exact optimized,
allocated, and realized artifact. Equal Terminal semantics alone cannot justify
reusing physical demand evidence. [Stack provisioning](../resources/storage.md)
must establish backing before execution; a size comparison alone does not.
Physical access validation and required target setup remain mandatory. Private
spills may change native resource demand, never the source `crashes` ceiling.
The complete final artifact still satisfies
[machine-state realization](machine_state_evidence.md).

## Evidence and control flow

Accepted proof obligations and ownership/borrow facts may justify check removal,
arithmetic identities, non-aliasing, field/variant irrelevance, cleanup or
transition reachability, and terminating or bounded loop transformations.
Candidates and receipts retain the exact consumed fact identities. Evidence
cannot disappear before its last dependent transformation or diagnostic reader.

Qualification rosters retain exact canonical paths, carriers, and establishment
lineage through transformations and lowering. Root shape or carrier equality
cannot manufacture a field qualification. Calls and returns preserve their
exact source and result contracts. At an actual control-flow join, an output
retains only qualifications carried by every incoming occurrence through valid
lineage. CSE/GVN distinguish unequal rosters unless a named transformation forms
their common intersection and independently revalidates every use. Inventing a
qualification is unsound; losing one may cause rejection, but fail-closed
narrowing alone is not evidence of an exact-preserving Terminal transformation. See
[structural access](../terminal-psi/structural_access.md) and
[authority](../resources/authority.md).

Analyses use explicit entry, exit, exceptional, cleanup, and transition edges.
Suspension is an interprocedural state of the exact call, not an invented local
successor. Cyclic components retain verifier-derived topology, loop-carried
arguments, ownership fixed points, and their distinct ranked, bounded, or
unranked progress evidence under [control flow](../terminal-psi/control_flow.md).
An analysis receipt does not itself authorize execution, a rewrite, a work
certificate, or publication. Transformations must preserve or independently
reconstruct the cycle and ranking evidence their consumers require.

## Atomic candidates and independent validation

Analyses are immutable products keyed by validated input revision and declared
dependencies. A rule states what it consumes and invalidates. After a commit,
each affected product is either preserved by proof or recomputed; stale facts
cannot be reused merely because their cached value remains available.

A candidate binds source and selection identities; exact rule, candidate, and
affected region; required analyses and typed facts; immutable patch or plan;
any predicted cost; provenance/work mapping; and consumed work. Construction
does not mutate the input. Independent validation reconstructs preconditions,
applies the plan to a fresh value, validates the result, and issues an
identity-bound receipt. Only then may the manager commit the replacement.
A matcher or producer cannot attest to itself by serving as its own validator.

Validation reconstructs representation invariants, exact rule preconditions,
stage accounting/invalidation/provenance, and source-to-lowered contracts at
their owning boundaries. Differential tests compare reference and transformed
execution where an interpreter or executable oracle exists; test agreement is
not the definition of semantic equivalence or substitute publication authority.

Surviving and synthesized values, instructions, blocks, edges, and bytes retain
roots identifying source constructs, preserved operations, applied rules, and
justifying facts, sufficient for required diagnostics and reports. Publication
still needs the full validation chain binding child identities and rejecting
detached, reordered, truncated, trailing, stale, or cross-source evidence.
Encoding, provenance, and cost estimates alone grant no publication authority.

## Decisions and nonauthoritative policy

Machine-readable decision rows bind input, candidate, rule, verdict, consumed
analyses/facts, validator, budget, and usage. Publication rejoins each validated
candidate declaration to its selected pass, complete rule contract, exact input
revision, manifest evidence, and baseline-policy row. An applied row binds
exactly one transformation commit; a skipped row binds none. Coordinated
rewriting manifest facts, predicted costs, and policy records cannot replace
this independent reconstruction. Human reports project these rows, not the
reverse.

Selection policies rank or choose already-declared validated candidates,
separately from candidate construction and independent validation. Explicitly
supplied decisions use a versioned schema; their contexts bind source,
selections, target, rule catalog, cost model, and the canonical candidate rows.
Each decision chooses a listed candidate or skips with a reason; it does
not author a rewrite or semantic contract. Recording independently reconstructs
the rows; replay requires exact context and row equality. Missing, malformed,
stale, or mismatched supplied decisions reject or use an explicitly selected
deterministic fallback. Cost estimates rank candidates; they cannot establish
eligibility or semantic correctness.

The current requirement is structural readiness for a possible future policy,
not an ML implementation. Keep deterministic selection usable without a model.
Do not build a trainer, training corpus pipeline, model evaluator, or inference
integration in the Rust reference compiler. A future learned policy belongs to
separately justified work on the Omega-written product compiler and would choose
through these same candidate and validation boundaries, not a second compiler
path. [Learned search](../../drafts/reference/learned_optimization_policy.md) remains
exploratory background, not a current implementation requirement.

## Release rollback and promotion

The repeatable `--disable-optimization <ExactName>` overlay subtracts exact
case-sensitive source names without changing the authored build selection or
its checked-compilation identity. Unknown and duplicate names reject. A known
unselected name is an accepted visible no-op, subject to the requested product's
phase restriction.

```text
applied   = authored selection intersection requested disabled
effective = authored selection minus requested disabled
```

Settlement occurs before the selected stages execute. Effective selection feeds
their execution and resulting artifact identities; a separate receipt retains
authored, requested, applied, and effective sets. The overlay cannot add rules,
change the catalog, or masquerade as source selection. An empty request creates
no rollback receipt; an empty effective set executes the ordinary identity phases.

A product cannot claim rollback for stages it does not execute. Check-only
production rejects nonempty requests; Terminal production admits checked-tree
and Psi-phase rollback, not post-Terminal names; native production admits its phase-scoped
overlay. Invalid requests reject before frontend work or auxiliary output.
Successful publication retains rollback provenance separately from effective
optimization identity and reports exact requested, applied, and effective names.

Caches bind source, selections, target facts, catalogs, validators, cost model,
and any relevant workload/profile identities. Promotion is not automatic: an
exact rule may become recommended or default only through a separate owner
decision supported by semantic/corruption, differential/reference,
determinism/bounded-work, target/OS, compile-time/output-quality, and exact-rule
rollback evidence. No current default is implied. Promotion never creates a
broad optimization level; diagnostics and manifests continue naming exact rules.
