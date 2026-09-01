# Optimizer Rollout

This brief owns selection and stabilization policy. The architecture entrance
is [optimizer_architecture.md](../optimizer_architecture.md).

## Build opt-in

The toolchain-provided `build.omg` vocabulary exposes exact variants through
`builder.optimizations.enable(...)`. Selection is duplicate-free, canonical,
versioned, and identity-bearing. Human report emission is a separate explicit
request.

No optimization is enabled by target, build mode, compiler default, environment
variable, or broad level. Empty selection takes the pre-existing compiler path
and must produce the same acceptance, diagnostics, and output.

The empty path is also a construction boundary, not merely a semantic no-op.
It uses ordinary artifact-to-abstract lowering and ordinary target assignment;
it does not construct a verified optimization input, optimization unit, pass
manager, ledger, or optimized-plan projection. Provider-installation replay
has distinct ordinary and explicit-optimizer entrances for the same reason.

`omega-compiler/tests/no_selection_golden.rs` is the executable firewall. Its
small entrance descends into acceptance/diagnostic and native-artifact leaves.
On every supported host it evaluates all four hosted native targets
(`linux_x86_64`, `linux_arm64`, `macos_arm64`, and `windows_x86_64`), compiles each
retained artifact twice, compares raw bytes, and checks reviewed target-local
metadata/digest files under `tests/omega/golden/optimizer/no_selection/`.
UEFI is not included until its physical adapter and publication chain are
implemented.

## Exact-rule release rollback

Native production accepts a repeatable release-tooling overlay:

```text
omega --disable-optimization ControlFlowCleanup \
      --disable-optimization CopyPropagation \
      --target linux_x86_64 main.omg
```

Each value must be one exact source-visible `Optimization` case name. Unknown
and duplicate names reject. A known rule that this build did not select is an
accepted, visible no-op, allowing one fleet-wide kill switch to be applied
idempotently across differently selected builds.

The overlay is deliberately subtractive. `build.omg` remains the authoritative
authored selection and its checked-compilation identity does not change. At
the native-realization boundary the compiler derives:

```text
actually disabled = build selected intersection requested disabled
effective         = build selected minus requested disabled
```

Only the effective set enters optimizer construction, artifact identity, and
native realization. When it is empty, compilation rejoins the ordinary
no-selection path; the four-target golden firewall checks exact semantic,
proof, object-text, image-byte, and metadata parity. The rollback request does
not masquerade as authored selection or alter the catalog.

A nonempty overlay is invalid for `--check` and Terminal-artifact production,
because those products never enter native optimizer realization and therefore
cannot truthfully report the rule as disabled. Rejection occurs before frontend
work or auxiliary output. An empty request retains existing behavior and
produces no rollback receipt.

Successful native production retains a separate in-memory receipt containing
the build selection, requested disabled set, actually disabled set, and
effective set. Successful CLI publication preserves it in the returned report
and prints the exact requested, applied, and effective names; the receipt is
not yet persisted as a file. This provenance is intentionally separate from
the effective optimizer/artifact identity.

The rollback regression enumerates `Optimization::ALL`: each exact name must
subtract only itself, disappear from every phase projection, and become an
idempotent no-op when the same overlay is applied again. Both build preludes
also prove that each authored exact name resolves to the same rollback name.
The hosted-target golden test separately proves that an empty effective set
rejoins the byte-identical ordinary native path.

## Compatibility firewall

While experimental:

- only exact implemented phase compositions are admitted;
- unknown selection and encoding versions reject;
- optimizer construction is skipped for empty selection;
- optimized artifacts retain the complete selection and validation manifests;
- release rollback receipts retain authored, requested, applied, and effective
  exact-name sets without changing the authored selection identity;
- caches bind source, selections, target facts, rule catalog, validators, cost
  model, and relevant workload/profile identities; and
- publication requires the full custody chain.

## Promotion

There is no automatic graduation to an optimization level. An exact rule may
become recommended or default only by a separate owner decision backed by:

- semantic and corruption coverage;
- differential/reference tests;
- deterministic output and bounded-work evidence;
- supported target/OS matrix results;
- compile-time and output-quality measurements; and
- a rollback mechanism that disables that exact rule.

Even after promotion, diagnostics and manifests report exact rule names.

## Experimental search and ML

Recording, training, and policy evaluation are outside ordinary compilation.
Replaying a policy is explicit and identity-bound. Absence or failure of a
model never makes the baseline compiler incomplete. The current Psi schema v2
is recordable and deterministically replayable, including exact analysis and
proof/fact evidence. A library-only offline boundary now admits canonical V2
logs into an independently validated, identity-bound corpus and partitions
whole source identities into deterministic training, evaluation, and
regression groups. It has no compiler/process/build dependency. Training,
and independently replayed evaluation/regression reports now have one tiny
deterministic cost-threshold reference implementation with strict identities
and codecs. It measures recorded-action agreement only. Explicit offline
`capture`, `train`, `evaluate`, and `regression` commands apply those same
strict admission and independent-replay boundaries and publish new artifacts
without overwriting existing paths. Evaluation and regression are distinct
commands with fixed splits. They run no compiler, external model, or process.
Checked regression manifests, meaningful measured objectives, external models,
and sandboxed compiler-side policy execution remain experimental work and are
not part of ordinary builds.

The neutral `omega-bounded-process` tooling boundary is only a prerequisite for
external execution. It centralizes structured command preparation, concrete
resource limits, bounded capture, deadlines, and process-container cleanup,
and is already shared by resolver execution and Git acquisition. Unix process
groups and Windows Jobs do not provide filesystem, executable, credential, or
network isolation. A dormant compiler-private adapter now owns exact
request/response/stderr/aggregate caps, deadlines, action-only response
matching, and explicit fail-closed or recorded-baseline settlement. It is
compiled only for tests or the named experimental feature and requires an
opaque verified-sandbox invocation that has no production constructor.
External optimizer policy therefore remains unavailable until a real platform
sandbox backend can construct that capability and an explicit build-level
selection authorizes the exchange.

## Deterministic differential corpus

`tests/native-differential/tests/optimizer_corpus.rs` is the small corpus
entrance. It owns only manifest admission, exact case replay, and target-lane
dispatch; `optimizer_corpus/{generator,manifest,psi,selected_machine,native}.rs`
descends into deterministic input generation, checked-in corpus identity,
valid Terminal-Psi construction and interpretation, and target-specific
machine oracles and host execution.

The V2 manifest fixes the format, generator, seed, case count, all artifact
shapes, target lane restrictions, and a SHA-256 digest of every generated
record. Each of the 64 records is run twice. Equality covers optimizer and
workload identities, pass and pre/post-physical manifests, commits, the
transformation ledger, register-home and machine-plan custody, selected bytes,
and resolved layout. The generated expected value is checked independently by
the Terminal interpreter on both conditional paths and then by an x86-64
decoder or AArch64 shortest-materialization validator. Set
`OMEGA_OPTIMIZER_CORPUS_CASE=<ordinal>` to replay one record with its format,
seed, inputs, expected values, and target restrictions visible.

The two original lanes deliberately use related, independently interpreted
artifacts per
target lane. The wrapping-add artifact exercises Psi SCCP; the immediate-leaf
artifact exercises selected-machine materialization. They are not presented as
one end-to-end optimized carrier: dead-source removal currently erases the
constant-definition provenance required by selected lowering, while retaining
the dead sources leaves an unsupported source shape. A later admitted carrier
may compose those lanes without weakening either validator.

V2 also adds a third, same-artifact host-native lane for exact integer
materialization. The Terminal interpreter evaluates both Boolean paths of an
immediate-leaf artifact. That exact artifact then crosses target lowering,
selection, allocation, the host's named post-allocation materialization rule,
encoding, and resolved layout. A tiny host harness links the resulting function
and calls both paths, requiring each U64 result to equal the interpreter result.
This is the first executable same-artifact lane; it does not imply float, trap,
atomic, placed-memory, cleanup, transition, unwind, or non-host execution
coverage.

## Documentation

The versioned [V1 release inventory](../../releases/optimizer_exact_rules_v1.md)
names every exact rule, phase, applicability, status, rollback command, and
supported composition. The adjacent
[rollback runbook](../../releases/optimizer_rollback.md) owns operational
disable, verification, log-capture, and restoration steps. A separate
`optimizer_rollout` architecture test derives the exact vocabulary from its
canonical source and rejects inventory drift, invalid status, inexact rollback,
and promotion without a completed owner-reviewed per-rule record.

`TASKS_OPTIMIZER.md` tracks implementation status; design choices live in the
architecture briefs; language-semantic blockers alone live in
`OWNER_QUESTIONS.md`.
