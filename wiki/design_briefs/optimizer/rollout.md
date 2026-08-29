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
(`linux_x64`, `linux_arm64`, `macos_arm64`, and `windows_x64`), compiles each
retained artifact twice, compares raw bytes, and checks reviewed target-local
metadata/digest files under `tests/omega/golden/optimizer/no_selection/`.
UEFI is not included until its physical adapter and publication chain are
implemented.

## Exact-rule release rollback

Native production accepts a repeatable release-tooling overlay:

```text
omega --disable-optimization ControlFlowCleanup \
      --disable-optimization CopyPropagation \
      --target linux_x64 main.omg
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

Successful native production retains a separate receipt containing the build
selection, requested disabled set, actually disabled set, and effective set.
Publication preserves that receipt, and the command prints the exact requested,
applied, and effective names. This provenance is intentionally separate from
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
model never makes the baseline compiler incomplete.

## Documentation

Release notes name exact supported rules and compositions. `TASKS_OPTIMIZER.md`
tracks implementation status; design choices live in the architecture briefs;
language-semantic blockers alone live in `OWNER_QUESTIONS.md`.
