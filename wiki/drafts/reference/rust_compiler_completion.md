# Rust compiler completion

Finish the Rust reference compiler against the accepted language and toolchain
contracts, then resume Omega self-hosting. All six gates below must pass on the
same source revision across the four hosted targets. This document specifies the
checks; it does not claim they pass.

This is release validation, not the test scope for every pull request. Routine
repairs follow the focused [sample loop](../../../AGENTS.md#the-sample-loop).
Use existing crate tests, kept fixtures and sample tooling. Do not recreate a
fixture corpus, persistent results ledger or second task board.

## Scope

The product compiles authored `.omg` source through the lowered-form checker and
Psi publication to hosted native execution on Linux x86-64, Linux AArch64,
macOS AArch64 and Windows x86-64.

The accepted surface is defined by the [specification and guide](../../README.md),
not whatever the compiler happens to accept. Unsupported constructs are honest
implementation refusals, not a reduction in the promised completion surface.
A narrower release scope requires an explicit owner decision. Unaccepted
proposals are not required features.

Freestanding EFI, optimization quality, the bootstrap chain and the Omega-written
compiler are separate work. They cannot compensate for a failed hosted gate and
do not silently become prerequisites for it. The Rust implementation remains an
independent comparison compiler after self-hosting resumes.

## Release matrix

| Gate | Required result | Evidence source |
| --- | --- | --- |
| `RC-REPOSITORY` | Formatting, lint, compilation and architectural dependency rules pass. | Repository commands below. |
| `RC-SOURCE-SEMANTICS` | Accepted features have exercising positive and negative tests. Valid source reaches its promised stage; invalid source fails at the intended obligation. | Workspace library and integration tests and the fixtures they actually read. Missing coverage remains open. |
| `RC-BUILD-AND-PACKAGES` | Build, immutable inputs, package identity, review, resolution and compilation handoff agree without hidden authority or name-based reconstruction. | Package/build tests and the relevant workspace integration tests. |
| `RC-NATIVE-MATRIX` | Required targets produce independently validated calling/ABI behavior and machine/object/image bytes, with matching-host native observations. | Target and independent-validator tests, plus native/Psi sample runs on each required host. |
| `RC-DIAGNOSTICS` | Rejections report stable, actionable diagnostics rather than panic, an unrelated unsupported construct or accidental acceptance. | Diagnostic/rejection tests and the sample gate's pinned rejection results. |
| `RC-REPRESENTATIVE-PROGRAMS` | Maintained samples check; authored hosted entries produce native products; documented deterministic exit/output oracles pass. | Unfiltered `sample-gate` on each required host, with result categories inspected. |

One run may supply several gates. Passing available tests does not establish a
feature they never exercise. Add or repair focused coverage at its owner rather
than creating another repository-wide inventory. Retain the actual commands and
selected test sets with the release evidence; an empty or filtered run cannot
stand in for full coverage.

## Commands

Run from a clean checkout with the pinned toolchain and the host budget in
[testing prerequisites](../../../AGENTS.md#commands). These are existing commands,
not a new corpus runner.

Repository checks:

```text
mbx xtask fmt --check
mbx clippy --workspace --all-targets -- -D warnings
mbx check --workspace --all-targets
mbx xtask check-boundary
```

Library and integration tests:

```text
mbx nextest run --workspace --no-fail-fast
```

Do not limit the release run to `--lib`: compiler integration tests own source,
publication and diagnostic controls too. Nextest does not run doctests. Package
and Build checks, including the separate doctest run, are:

```text
mbx nextest run -p build-declarations -p source-acquisition -p resolver-execution --no-fail-fast
mbx test --doc -p build-declarations -p source-acquisition -p resolver-execution
```

On each matching hosted runner:

```text
mbx xtask sample-gate
```

Unset filters and retain the discovered sample set. The existing `--out` option
may write a requested result file with that run's evidence. `checked`,
`no-oracle` and `not-host-runnable` are not native execution passes. A library or
check-only sample needs its declared check result; a runnable sample with an
oracle needs actual execution. An intended-invalid program must reach its
stated rejection, not just fail for some other reason.

These commands exercise maintained coverage. They do not turn unexpected skips,
unavailable validators or missing accepted features into successful evidence.

## Integration and trust checks

Complete original customers rather than isolated demonstrations. Allocation and
collection delivery must establish backing, create and lend elements, end loans,
handle failed growth, dispose of contained values and return backing. The
[allocation contract](../../spec/resources/allocation.md) and its existing task
own this lifecycle; there is no second collection API here.

Early computations use the ordinary checked evaluation route with their actual
dependencies. A syntax evaluator or convenient pass order cannot replace a
missing constant, selected operation or target fact. See
[semantic evaluation](../../spec/language/evaluation.md).

The [automatic proof floor](../../spec/proofs/contracts.md#required-automatic-discharge)
is part of source conformance. Its goals need no hint or increased search
allowance. Exercise the specified `--floor-only` behavior; missing required
reasoning or an unimplemented option leaves the relevant gate open.

Publication repairs exercise a separately consumed artifact without the
producer's live source objects. Preserve exact applications, borrowed subjects,
writes and evidence dependencies. Negative controls alter the relevant claim or
association and fail at its owning boundary. Native/Psi agreement cannot detect
source-meaning errors already shared by both routes; expected-result, rejection
and applicable independent-validator tests remain necessary.

A repair should generalize across ordinary uses of the same operation. Choose a
small witness exposing the mechanism's assumption, such as a loan forwarded
through a generic helper and a published contract. Do not make a separate
producer or exhaustive placement matrix for every source shape.

Release-facing examples must agree with current rules and have owning tests for
the behavior they advertise. Invalid examples need the advertised rejection.
A documentation edit is not execution evidence.

## Required platform runs

| Runner | Product identity | Required native observation |
| --- | --- | --- |
| Linux x86-64 | `linux_x86_64` | Execute emitted ELF x86-64 programs. |
| Linux AArch64 | `linux_arm64` | Execute emitted ELF AArch64 programs. Permitted emulation must name the emulator and version. |
| macOS AArch64 | `macos_arm64` | Execute emitted Mach-O AArch64 programs. |
| Windows x86-64 | `windows_x86_64` | Execute emitted PE x86-64 programs. |

Cross-target byte generation does not replace matching-host execution. A
platform-specific test may be inapplicable elsewhere; a missing runner,
unavailable engine, unexpected ignored test, timeout or exhausted resources
leaves a required result open. State expected exclusions beside the result,
not in an averaged pass percentage.

## Closure rule

All six gates close only for one clean revision with the required hosted runs.
Record the commit, pinned toolchain, OS/architecture, commands, selected tests,
results, elapsed time and expected exclusions with that release's evidence.
Keep measurements tied to their actual source and host. Git holds superseded
runs and decisions, not current instructions.

A regression reopens its gate. A prohibitively slow, flaky or routinely
resource-exhausting gate stays open until repaired or replaced by equal or
stronger deterministic coverage. Do not silently narrow the accepted surface,
weaken a check or mark an unavailable check passed.

Current work stays in [TASKS.md](../../../TASKS.md). Delete this temporary plan
after closure, retaining the maintained tests and completed product's evidence
with their owners.
