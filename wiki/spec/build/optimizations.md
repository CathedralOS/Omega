# Optimization selection and validation

Optimization changes a realization while preserving its semantic contract.
This reference defines selection, transformation admission, and reporting;
it does not imply implementation of every rule, phase, or program shape.
[Terminal Psi](../terminal-psi/product.md) owns the portable boundary.

## Exact selection

The build vocabulary exposes exact named selections through
`builder.optimizations.enable(...)`. The selection is duplicate-free,
canonical, versioned, and identity-bearing. Human report emission is a separate
explicit request. No target, environment variable, compiler default, build
mode, or broad `O1`/`O2`/`O3` level implicitly enables an optimization.

Every optimization has exactly one phase owner. The complete selection projects
into closed phase-local sets for checked trees, Psi, abstract operations, target
operations/instruction selection, pre-allocation, post-allocation, and resolved
layout. A stage consumes only its projection; downstream coordinators must not
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

Every phase receives validated input and its exact phase selection, executes
selected rules in canonical order under explicit bounds, independently validates
each applied transformation and the final representation, and returns one typed
stage result with execution evidence. Candidate ordering is deterministic;
fixed-point passes have explicit convergence and iteration budgets.

An empty selection is the same phase's identity execution. Input and output
validation still run; output equals input, with no pass manifests or
transformation records. Empty selection is not a separate compiler or
publication route. Disabling every selected rule yields the same identity-phase
acceptance, diagnostics, and output as an initially empty selection, apart from
the separately retained rollback provenance.

Checked-tree pruning runs only after all authored source has been parsed,
resolved, typed, and checked; deleting unreachable code cannot make invalid
source valid. A root selection that changes the published product retains its
own explicit identity, not one inferred from the remaining declarations.
Shared frontend work precedes target fan-out under the
[target-selection contract](configuration.md).

Terminal Psi is the immutable output of Psi optimization. Publication retains
the exact Psi selection and input/output semantic identities; decoding checks
the recorded output against the published semantics. Proof identities remain
internal validation provenance unless relevant evidence is requested through
[PCC publication](../proofs/publication.md). An absent sidecar waives no ordinary
transformation check. Requested portable evidence must establish the necessary
preservation without producer state. Empty execution may
claim only equal input and output identities. A receiving interpreter or native
lowerer does not rerun Psi passes. A target-constrained build companion may carry
pending post-Terminal selections, excluding earlier phases and bound to the
complete build selection. It is a proposal a receiving lowerer may refuse, not
authority to execute a transformation. A raw portable product leaves subsequent
realization choices to its receiver.

## Observations and resources

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
volatile/placed accesses, cleanup, and transitions are barriers unless an exact
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
are costs, not [logical fuel](../resources/logical_work.md). The Terminal fuel
schedule is external deterministic accounting; exhaustion is an incomplete
consumer outcome, not a source result, and native execution has no implicit fuel
meter. Fixed-fuel certificates use the immutable Terminal subject. Relocating a
source operation retains its exact fuel settlement and provenance at the new
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
any predicted cost; provenance/fuel mapping; and consumed work. Construction
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

An explicitly selected external policy may rank or choose already-declared
validated candidates through a versioned schema. Requests bind source,
selections, target, rule catalog, cost model, and the canonical candidate rows.
The closed response chooses a listed candidate or skips with a reason; it does
not author a rewrite or semantic contract. Recording independently reconstructs
the rows; replay requires exact context and row equality. Missing, malformed,
stale, or mismatched responses reject or use an explicitly selected deterministic
fallback. Cost estimates rank candidates; they cannot establish eligibility or
semantic correctness.

Models never become a baseline compiler dependency or runtime oracle. External
execution requires explicit build authorization and a real sandbox admission;
process groups, resource limits, and deadlines alone do not establish filesystem,
executable, credential, or network isolation. Offline training and evaluation
grant no compiler activation, optimizer replay, or artifact-publication
authority. [Offline policy tooling](../../../omega-rust/omega/tooling/optimization-policy-offline/README.md)
owns its current data formats and commands; broader
[learned search](../../drafts/learned_optimization_policy.md) remains exploratory
background, not a proposed integration or an implementation requirement.

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
production rejects nonempty requests; Terminal production admits Psi-phase
rollback, not post-Terminal names; native production admits its phase-scoped
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
