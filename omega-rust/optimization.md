# Optimization implementation

The [public optimization contract](../wiki/spec/build/optimizations.md) owns
exact selection, identity phases, semantic preservation, rollback, and policy
authority. This page identifies implementation entrances, not another rule
inventory. The [source-organization audit](../tests/architecture/optimizer_source_organization/README.md)
owns navigation and size checks.

The checked [exact-rule inventory](omega/representations/optimization-core/rules.md)
records current experimental status, applicability, and composition limits.

## Selection and portable publication

[Build evaluation](omega/build/build-evaluation/src/optimization/mod.rs) admits
the exact vocabulary. The compiler's
[build vocabulary](omega/compiler/compiler/src/pipeline/optimization/build_vocabulary/mod.rs)
supplies both preludes from one mapping; its
[checked handoff](omega/compiler/compiler/src/pipeline/optimization/checked_handoff/mod.rs)
retains selection and report requests.
[Rollback settlement](omega/compiler/compiler/src/compiler/optimization/rollback/mod.rs)
computes effective selection before artifact production without changing the
authored selection.

The [Psi X-to-X entrance](psi/pipeline/lowered-psi-to-lowered-psi/src/lib.rs)
consumes complete unsealed `LoweredPsi`, validates both sides, and returns the
only optimization-stage result accepted by Terminal publication. Its
[Psi-local catalog](psi/representations/optimization/src/optimization_selections/catalog.rs)
does not import native target vocabulary. Identity execution and the admitted
dead-pure-scalar pass run here; other named passes reject until ported. The
execution record survives canonical Terminal encoding and independent decoding.

The [post-Terminal abstract phase](omega/pipeline/abstract-operations-to-abstract-operations/src/phase.rs)
joins verified input construction, bounded execution, and independent projection
publication. Its result exposes the current abstract program and replay evidence,
not the executing session or analysis cache. Legacy `Psi` names still occur in
that crate's internal rule machinery; they do not authorize a receiving lowerer
to rerun pre-Terminal passes. The compiler consumes only the pending
post-Terminal selection projection and preserves the complete selection binding.

Structural-call contract validation preserves unrestricted write-only subloans
through exact field/literal-index paths from mutable or write-only parameter
roots. Indexing requires a material root and primitive/record leaf; an existing
reference ABI does not grant readable access. Empty-selection publication retains
the complete abstract plan, not just a matching entry signature. The
[source-produced indexed receiver probe](../tests/native-differential/tests/terminal_psi_indexed_receivers.rs)
checks this admission and executes native caller-owned writes through ordinary
projected references on supported hosts. `WRITE-ONLY-BORROW` in
[TASKS.md](../TASKS.md) retains the acceptance command and remaining stack-pointer
transport and platform coverage.

Abstract publication owns the immutable current plan independently of the
receipt and retained replay inputs. Replay checks the exact schedule, candidate
declarations, applied-versus-skipped disposition, commits, consumed analyses and
facts, and baseline-policy/cost evidence. A decoder, report, or jointly altered
decision log cannot supply missing transformation authority.

## One physical sequence

The [native physical entrance](omega/compiler/native-realization/src/native_pipeline/physical_pipeline/mod.rs)
checks the retained selection, then performs instruction selection, selected
X-to-X execution, allocation, machine construction, and canonical frame
realization once. Mandatory lowering is not an optional optimization.

[Phase admission](omega/compiler/native-realization/src/native_pipeline/physical_pipeline/phase_selections.rs)
rejects selections without an implemented current-data stage before execution.
In particular, retained selected-lowering catalog entries do not imply native
publication support, and retired post-allocation rewrite names cannot select an
alternate emitter. Allocation recovery and layout selection enter their owning
catalogs. The exact executable catalogs, not historical matrices, determine
applicability and composition.

The current selected program and allocation are separate from the evidence of
how they were obtained. Downstream construction consumes those current facts;
replay alone inspects retained transformation history. Do not add a program
representation or a complete emission route for each rule, fixture shape, or
optimization-history combination.

[Allocation](omega/pipeline/selected-instructions-to-register-homes/README.md)
owns homes and pressure recovery.
[Machine emission](omega/backend/machine-emission/README.md) owns frame
realization, fragment emission, and text placement. Target ISA crates own
instruction alternatives, effects, encoders, and decoders. Object construction
and callable/image admission remain later owners; an encoding receipt does not
replace their checks.

## Catalogs and independent replay

An owning stage has one exact ordered catalog, a small execution join, and
separate proposal and validation implementations. A selected descriptor retains
the rule, validator, policy, analyses, invalidations, budgets, and applicability;
custody coordinators must not reproduce its name-to-implementation schedule.
Each pass may order its local rules, but a family map below it is not a second
enablement point.

Analyses and candidates bind the current revision and exact fact identities.
Committing a rewrite requires independent reconstruction, result validation,
and analysis invalidation or preservation evidence. Neutral models and canonical
encoding can be shared; a validator must not call the producer's matcher,
transformation helper, or fact-producing algorithm to attest to that result.
Shared predicates and primitives are not a shared output-producing decision
procedure.
Shared mechanics belong at the nearest genuine semantic owner, not under one
sibling rule merely because it was implemented first.

The bounded countdown machinery illustrates a private restriction, not a
language limit. Its
[ranked-cycle validation](omega/pipeline/abstract-operations-to-abstract-operations/src/validation/context/mod.rs)
reconstructs component/ranking evidence. Only authenticated guard-zero and
decrement-one relocation to the canonical preheader suffix may normalize the
otherwise frozen component; provenance and fuel settlements remain unchanged.
The separate ranked-rewrite join must still authorize and validate application.
Analysis custody alone is neither execution nor general loop-motion authority.

Ordinary Natural-ranked and unranked cycles instead retain exact verified-source
components and freeze the complete cyclic function, including prefixes and exits.
Unranked safety verification supplies no ranking certificate or fixed-work bound.
Raw cyclic units remain insufficient input; receiving legalization replays the
same source-bound topology before ordinary native selection.

[Offline policy tools](omega/tooling/optimization-policy-offline/README.md)
consume recorded decisions without creating a second catalog or compiler
execution path. Cost estimates and reports describe choices; their identities
cannot replace semantic, translation, or publication evidence.

## Validation when extending a stage

Exercise exact selection and empty identity execution; wrong target, unsupported
composition, malformed source, and stale evidence; independent one-field and
cross-rule corruption; deterministic ordering and each exhausted work budget;
and actual downstream replay. A codec round trip or isolated applied-rule test
does not establish compiler-generated application or publication support.
Keep positive, refusal, corruption, and compatibility fixtures beside the rule
or boundary they test. Use differential execution only on genuinely supported
target/host paths and distinguish two related artifacts from a same-artifact
end-to-end comparison.

## Operational rollback

Keep `build.omg` unchanged and supply one repeatable, case-sensitive argument
per affected exact rule. This command works in PowerShell and POSIX shells:

```text
omega --disable-optimization ControlFlowCleanup --target linux_x86_64 main.omg
```

Check target and composition support before attributing failure to a rule.
Unknown or duplicate names reject; a known unselected name is a visible no-op.
Check-only requests reject rollback. Programmatic Terminal publication supports
only its Psi-phase overlay; native publication settles its applicable phases.
All routes execute the same identity stages when the effective selection is empty.

After successful CLI publication, capture the printed receipt with the build log:

```text
optimizer rollback: requested=[ControlFlowCleanup]; applied=[ControlFlowCleanup]; effective=[]
```

`applied` is the authored/requested intersection; `effective` is authored minus
requested. Verify every intended selected rule is applied and absent from the
effective set. An unselected rule has no applied member. The in-memory report
retains the receipt; the CLI does not currently publish a separate receipt file.
Empty effective selection must match ordinary identity-phase semantics and
artifact output apart from explicit rollback provenance.

Before deployment, run the affected target/workload checks and retain command,
log, compiler identity, target, and output digest. Restore only the affected
argument after semantic/corruption, differential, determinism, target,
measurement, and rollback coverage passes; do not replace exact selection with
a broad optimization level. [Promotion evidence](omega/representations/optimization-core/promotions/README.md)
remains a separate owner-reviewed requirement.
