# 0006: Author-invoked assertions and build diagnostics

Status: open design proposal. No assertion spelling, core API, provider scheme,
build setting, new crash cause, or implementation experiment is approved.
The alternatives below are design exercises, not implemented or tested features.

## Settled boundary

[Admission and runtime diagnostics](../spec/proofs/contracts.md#admission-and-runtime-diagnostics)
is settled independently of this RFC: admissions require no runtime checking.
There is no mandatory automatic instrumentation or replacement obligation to
write checks by hand. Runtime checkability does not determine admission;
diagnostic selection does not change grants or assumption reports. Tests may
expose violations but do not establish admission or universal correctness.

This RFC does not restore the former blanket runtime-tripwire requirement.
Choosing no new assertion facility remains a valid outcome. Any future proposal
for automatic checking needs its own concrete customer and justification.

## Customer and scope

Application and library authors want to write diagnostic conditions near the
behavior they test. An application author may want one root build choice to
enable those authored sites throughout rebuilt dependencies, without alternate
API names or repeatedly editing published crash clauses.

This is not runtime reflection or execution of the proof system. The question
is how ordinary Boolean evaluation and conditional failure should interact with
ownership, provider selection, published contracts, and artifact identity.
Assertions do not arise from inspecting arbitrary `requires` or `ensures`.

Governing contracts:

- [Service reach and crash ceilings](../spec/language/effects.md).
- [Argument evaluation and outcomes](../spec/terminal-psi/calls_and_outcomes.md).
- [Provider selection](../spec/build/provider_selection.md).
- [Build configuration](../spec/build/configuration.md).
- [Ownership and cleanup](../spec/language/ownership.md).
- [Artifact observations](../spec/terminal-psi/observations.md).

## Current pieces and limits

Omega has ordinary Boolean control flow, explicit `crash Trap`, guarded crash
contracts, and provider selection. Private checked bodies may infer effects;
published interfaces and requirements retain their own ceilings. Permission on
an application entry does not authorize a callee to violate its interface.
An unconditional `crashes Trap` permits failure but does not require it, so a
nontrapping implementation can still fit that ceiling.

Ordinary arguments evaluate before invocation. Static machine arguments exist
for named operations, including the [task interface](../spec/build/task_runtime.md),
but that does not establish complete support for a generic predicate-taking
assertion API. No first-class Omega assertion construct or shipped assertion
helper was identified in the source inspection for this discussion.
[Epsilon assertions](../../bootstrap/4_epsilon/LANGUAGE.md) are a separate
bootstrap language feature and confer no approval on Omega syntax.

The existing crash causes are `Trap` and `Abort`. `Assertion` below is a
candidate semantic cause, not a current source name. Equal native trap bytes
would not make differently attributed semantic outcomes interchangeable.

## Evaluation and ownership choices

All examples in this RFC are schematic, not accepted Omega syntax.

| Shape | Disabled behavior and trade-off |
| --- | --- |
| `assert(predicate())` | A no-op callee still receives an eagerly evaluated Boolean. Independently valid optimization may remove computation, but omission is not inherent in the call. |
| `assert_with<Check>(&value, index)` | A disabled implementation does not invoke `Check`. Input preparation still occurs; a named predicate and explicit inputs add source verbosity. |
| Compiler-recognized assertion expression | Build elaboration can omit predicate evaluation directly and retain source attribution, but requires a defined special operation. |

A delayed predicate need not move its subject: scalars can be copied and larger
values borrowed synchronously. A named machine does not require a runtime
closure or heap allocation. Borrow formation and lifetime checking still apply.
Passing an owned value transfers its obligation even to a no-op callee; required
cleanup cannot be silently erased. Expensive or effectful argument construction
still occurs unless placed inside the delayed computation.

Normal optimization may remove unused copies, references, empty calls, and
pure terminating computations when their observable behavior is preserved.
It cannot erase effects, divergence, permitted crashes, or ownership obligations
merely because an assertion implementation ignores its inputs. No measurement
or implementation claim of zero overhead is made here.

## Contract and build alternatives

### A. Ordinary checked core machine

An assertion can be a checked library machine that returns on true and crashes
on false, publishing its guarded crash contract. It is not inherently a boundary
service and requires no new trust admission. Diagnostic output would have its
own ordinary service and authority requirements.

This reuses existing control flow and optimization, but crash possibilities
propagate into published library contracts. A Boolean-true precondition would
instead require proof before calling and would not express runtime validation.

### B. Selected checking and no-op implementations

The root build could select ordinary implementations under one fixed
requirement. To admit both, that requirement must permit diagnostic failure
without promising that normal return establishes the predicate. The no-op
implementation returns even when the predicate is false.

Provider selection neither changes the requirement nor grants runtime authority.
If the requirement permits `Trap`, the published ceiling remains even with a
no-op selected. If it forbids `Trap`, the checking implementation cannot add it.
An admitted provider does not remove that mismatch. A reporting backend may be
external, but selecting it does not solve predicate omission or crash ceilings.

### C. Separate debug entry or API variants

A `debug_main` wrapper can publish a broader entry ceiling while sharing a body.
It cannot widen the shared body's library interfaces. Requiring `debug_read`,
`debug_parse`, and similar variants throughout dependencies causes source and
interface duplication. Keeping an unconditional `crashes Trap` on every relevant
public API avoids toggling clauses but deliberately weakens those contracts.

### D. Build-wide additional Trap allowance

A new root policy could add `Trap` permission to effective contracts throughout
one compilation variant before checking and optimization. Child builds within
that closure would inherit the root's policy, not independently widen it.

This avoids handwritten clauses without privileging assertions, but also permits
ordinary explicit traps previously rejected at no-crash or more narrowly guarded
interfaces. It does not convert Exact arithmetic to Trapping arithmetic, justify
invalid accesses, or prove an unfulfilled obligation. Extending the policy to
arbitrary `reaches` services additionally raises capability and dependency issues:
a permission ceiling cannot create authority.

This is a proposed change to effective-contract formation, not current provider
selection behavior. It loses contract precision throughout the selected closure.

### E. Distinct diagnostic Assertion cause

A new `Assertion` cause could have trap-like termination behavior while remaining
distinct in semantic traces and reports. A root policy would permit that cause
through effective contracts without handwritten per-API ceilings. `Trap` and
`Abort` would keep their ordinary coverage rules.

Two source-access choices remain open:

- Only an exact compiler-recognized core failure primitive may originate the
  cause; user-authored wrappers call it. This restricts attribution but requires
  special primitive identity and handling.
- User-authored implementations may write `crash Assertion`. Summaries retain
  that cause, while the build supplies the allowance to trait and API contracts.
  This is more compositional but makes the diagnostic designation author-chosen;
  it cannot certify that the failure was exclusively a Boolean assertion.

Source unnameability is not required merely to distinguish the cause. Nor does
omitting a handwritten trait clause mean the effective contract omits failure.
A fixed requirement that truly excludes diagnostic failure still rejects it.

With a no-op implementation selected, disabling the cause should require the
resulting executable closure to exclude it. It must not turn an arbitrary
terminal `crash Assertion` statement into fallthrough. A distinct cause alone
does not erase eager predicate arguments.

### F. Compiler-recognized diagnostic sites

A special assertion operation could combine lazy predicate capture, build-wide
selection, and failure attribution. It would not require a general macro system,
but the compiler would own its syntax or canonical core identity and semantics.
Its effective artifact contract would expose enabled diagnostic failures.

This directly serves the ergonomic customer, at the cost of a dedicated source
or intrinsic form. It can reuse ordinary predicate computation and crash lowering;
it need not entail general instrumentation or runtime proof objects.

## Cross-cutting constraints

### Diagnostics versus validation

If a check may be disabled, the ordinary program must verify without facts
established solely by its successful execution. A shared no-op/checking
requirement cannot promise `ensures condition`. A check needed to validate an
external result for subsequent safe use is ordinary required validation, not
optional diagnostics.

A facility that omits predicates should restrict their computation to valid,
terminating, application-side-effect-free observations, or explicitly specify
which behavior disabling removes. Delayed borrowed inputs still need valid
lifetimes. Evaluating a predicate cannot be justified solely by the disputed
claim, such as dereferencing a pointer to determine whether it was safe to read.

### Trust and optimization

Ordinary optimization, including use of admitted guarantees, is the baseline
considered here. An assertion of an already assumed postcondition may disappear.
An all-or-nothing build switch means common enablement policy, not mandatory
retention of every redundant test.

Protecting only an assertion function body is insufficient: its Boolean argument
or a predecessor branch may already have been simplified using the claim.
Guaranteeing detection despite those assumptions would require rules across
predicate inputs, control flow, proof facts, and their derived dependencies.
Such optimization-resistant checking is a separate, more expensive alternative,
not approved or required by this RFC. Authors can instead expose a weaker
boundary contract and validate the result without assuming the disputed fact.

### Compilation closure and effective contracts

A root-wide selection could cover source dependencies compiled in that artifact
without alternate API names or dependency-authored debug entry conventions.
Selection would precede the analyses whose facts and outcome contracts it changes.
Each variant would retain its exact policy, checks, costs, and effective contracts
in artifact/cache/evidence identity. Optimization selection remains independent.

Precompiled dependencies cannot acquire missing assertion sites from a switch.
They need compatible retained source/representation or an appropriate rebuilt
variant; coverage must not silently claim otherwise. Separate products have their
own roots. A dependency cannot silently override its consumer's root policy.

A root cannot change an external host's fixed contract. Enabled diagnostic
failures must be exposed to component admission and publication; an artifact
cannot simultaneously gain reachable failure sites and retain an unconditional
no-crash execution guarantee. A generic permission toggle is not a substitute
for this compatibility check.

### Cleanup, proofs, and outcomes

Failure has no implicit unwind, flush, cancellation, or recovery. A diagnostic
variant that can interrupt cleanup cannot retain the same unconditional
cleanup-completion guarantee. Surviving external state needs its existing
containment or recovery contract; a diagnostic label discharges no obligations.

Proof and hermetic semantic evaluation cannot acquire an escape from their
formation and safety obligations through a runtime diagnostic switch. The
unmodified program must still establish its contracts without optional checks.
Any instrumented artifact needs verification under its actual outcome contract,
not reuse of a no-crash certificate with the new failure sites omitted.

## Design lab: discriminating cases

These are proposed acceptance scenarios for a future design, not executed tests
or authorization to implement one. They separate syntax convenience, erasure,
ownership, trust, and interface compatibility.

| Case | Question or required distinction |
| --- | --- |
| Disabled assertion of a scalar comparison | Can existing optimization remove the work without special semantics? Do not assume a measured result. |
| Disabled delayed predicate with borrowed inputs | The predicate is not invoked; argument preparation and borrow formation remain valid. |
| Disabled call with an owned linear input | No implementation may silently forget the input's disposition obligation. |
| No-op implementation declaring `ensures condition` | A false input exposes that the shared guarantee is invalid. |
| Assertion repeats an admitted postcondition | Normal optimization may remove it; this is not admission verification. |
| No-crash library API with a failing diagnostic | Ordinary calls require a crash ceiling; build-policy candidates must expose changed effective contracts without renamed APIs. |
| Ordinary undeclared `Trap` under an Assertion-only policy | It must still reject; the distinct category must not become blanket crash permission. |
| Assertion-only policy with arbitrary `crash Assertion` | Compare source-authorable designation against a sealed failure primitive; decide attribution guarantees explicitly. |
| Assertions disabled with a remaining diagnostic crash | Reject or require a different valid selected body; never silently invent fallthrough. |
| Root enables checks in rebuilt and precompiled dependencies | Report actual coverage and reject incompatible artifacts rather than claiming complete insertion. |
| Assertion interrupts a cleanup hook or callback | Validate the changed termination/failure contract and external caller expectations. |
| Disabled assertion was the only bounds justification | Ordinary program checking must reject; diagnostic omission cannot leave an assumed fact behind. |

## Decisions before implementation

Choose whether ordinary library assertions are sufficient or whether absence of
per-API diagnostic clauses justifies a new effective-contract rule. If a build
policy is chosen, specify its closure, inheritance, default, artifact identity,
and compatibility with fixed external interfaces. Choose ordinary `Trap`, a
distinct cause, or special diagnostic sites; specify source access and attribution.
Separately choose eager versus delayed evaluation and disabled-predicate behavior.

No choice follows automatically from closing the admission question. Existing
explicit validation and provider tests remain the baseline. Do not add an
implementation task for automatic instrumentation or a selected assertion path
until the owner adopts that path.
