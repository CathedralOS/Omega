# Build-level behavior exclusions

An authoritative build may forbid behavior in its exact selected executable
composition, independently of conservative callable declarations. This is a
product-admission requirement, not an effect mask or an assertion-specific mode.
The contract is normative; implementation and protocol coverage remain tracked
in [the execution board](../../../TASKS.md#build-level-behavior-exclusions).

The first customers are one source library built with checking or no-op assertion
implementations, and optional logging without editing public contracts between
configurations. No new assertion syntax, crash cause, implicit debug/release mode,
or arbitrary provider replacement mechanism is introduced.

## Two independent checks

Both obligations hold:

1. Every implementation satisfies its authored callable/requirement contract.
2. The selected composition satisfies the build's additional exclusions.

Published crash ceilings and service summaries retain their ordinary meaning and
identity under [service reach and operational ceilings](../language/effects.md).
A build without a Trap exclusion cannot authorize a Trap forbidden by an
intermediate interface. A declaration permitting Trap cannot override the build's
exclusion. Private inference, public crash coverage, direct boundary declarations,
static callback dependencies, and fixed external requirements remain unchanged.

The product check may establish a stronger fact from verified selected
implementations than their broad published allowances. That fact belongs to this
composition; it does not rewrite a callable's signature, make it satisfy a narrower
requirement, or supply facts to ordinary callers that only possess its contract.
No general crash-effect polymorphism or automatic API refinement follows.

## Restriction vocabulary

The root supplies finite sets of exact exclusions. Their meanings are distinct:

| Exclusion kind | Required absence |
| --- | --- |
| Crash cause | No possible semantic outcome of that cause, initially `Trap` or `Abort`. |
| Abstract service | No possible invocation of the exact boundary service, using the existing nominal identity and boundary-parent closure. |
| Physical authority class | No selected mechanism exercises the named terminal class under the existing receiving-policy classification. |

An abstract Console invocation still counts when its selected Console provider
is silent. An ordinary no-op logger that never invokes Console can satisfy a
Console exclusion, even when its public contract conservatively permits Console.
Likewise, removing physical filesystem access does not erase a Readable service
invocation. Physical class exclusions use
[service permission and terminal authority](permissions.md); they do not redefine
the abstract services or invent a second mechanism-classification system.

Service names must resolve to exact authorized declarations, not string labels.
Unrecognized crash causes, unknown classifications, and ambiguous identities
reject. Arbitrary authored crash categories, user-defined predicates, Boolean
effect-row syntax, and a general policy-plugin language are not part of this
contract. Exact Build API signatures and versioned encoding fields must be
specified by the existing build/protocol owners before claiming source or wire
compatibility; this document fixes the required semantics rather than inventing
an illustrative spelling that appears shipped.

## Selection and scope

Exclusions are evaluated Build selections, like other product configuration, not
dependency-discovery declarations. They may be assembled by authorized helpers
within the root's admitted build. Repeated selections combine by set union;
duplicates are idempotent, order cannot change the result, and later selections
cannot remove earlier restrictions. No added exclusion is implied by omission.
Existing target, source, authority, and receiving-policy restrictions still apply.

There is no intrinsic debug/release switch. An application may select a checking
implementation in one configuration and a no-op implementation plus a Trap
exclusion in another. Selection uses existing checked static applications and
provider mechanisms; it does not make every ordinary function a replaceable
Build provider slot. Selection itself grants neither authority nor permission to
change a requirement's meaning.

The checked scope is the exact target child and selected executable root or
component closure, not every package acquired by the build and not every process
on the host. It includes every possible admitted entry and all code, state and
selected dependencies reachable from those entries: startup, generated adapters,
argument evaluation, callbacks, timers, workers, cleanup, and retained providers.
An unused unselected implementation in the source inventory need not satisfy a
product exclusion, but remains subject to ordinary source checks.

Consumer exclusions cover dependency contributions throughout that product's
execution closure. A dependency cannot opt out or weaken the consumer's ban.
Independently published components retain their own restrictions and must also
satisfy the receiving composition's requirements. Exclusions do not automatically
apply to resolver activity, build-helper execution, or unrelated target children;
those retain their own admission contracts. An artifact-only output of arbitrary
bytes cannot be advertised as a checked executable satisfying exclusions.

## Verification and publication

Check exclusions after generated source and the relevant root/provider selections
are fixed, and before admitting or publishing the executable product. This is a
compiler admission step after authored build execution, not a build callback
that inspects its own unfinished product. It follows the existing
[staged execution](execution.md) and [product-selection boundary](scoped_execution.md#target-inspection-and-reflection).

Reconstruct a sound conservative account of possible behavior from the selected
semantic operations, closed calls and verified realization evidence. Declarations
remain the contract-checking baseline; absence checking may use stronger facts
only when independently justified for the exact selected composition. A direct
abstract boundary invocation retains its service identity even when native
lowering erases the call. Native instruction scanning, a producer-written empty
row, and a test run cannot establish semantic absence.

Sound guard proofs may exclude impossible paths under the admitted root premises.
The checker need not decide arbitrary semantic reachability: unsupported or
incomplete evidence is failure to establish the exclusion. Optional optimization
selection must not determine admissibility. Selection and ordinary semantic
checking must suffice for the no-op assertion customer; the check cannot depend
on a particular inlining or dead-code-elimination pass having run. Later native
realization must preserve the semantic evidence and satisfy applicable physical
class exclusions through its accounted mechanisms.

For a static selected call, use its verified implementation and exact application.
For dynamic or installation-bound calls, cover every admitted target or retain
the fixed contract conservatively. A provider currently selected behind a mutable
slot is insufficient unless replacement is constrained to preserve the exclusion.
An opaque or precompiled dependency with only a broad may-Trap or may-Console
contract cannot establish absence of that behavior. Obtain stronger independently
verifiable evidence, choose another artifact, or reject. No private source need
be exposed to author lookup, and no consumer may merely strip a dependency's
declared allowance.

Retained evidence binds the exclusion sets, execution scope and entry roster,
source/generated and semantic subjects, selected applications/providers,
target/realization identities, checker schema, and checked or explicitly accepted
assumptions. Consumers independently verify those relationships. Changed inputs
require rechecking; cached success and equal native bytes alone are insufficient.
Assumptions remain visible and governed by ordinary admission rules; a build ban
is not evidence that an unverified host contract is true.

Report a satisfied exclusion, a prohibited possible behavior with attributable
call/entry/provider context, or inability to establish absence with the missing
evidence. The latter two both fail required product admission but are distinct
diagnostics. Do not label a conservative possible path as a witnessed runtime
execution. No partial successful product is published when an exclusion fails.

## Installation and replacement

The stronger property is part of exact product admission and, when applicable,
the owner-authorized component installation/replacement envelope. A receiving
composition with stricter bans must establish them for the joined closure; an
old receipt for a weaker policy is not enough. Runtime rebinding, new entries or
replacement must preserve the restrictions through checked admission or require
a new owner-authorized composition. Candidate code cannot widen its own envelope.

Crash exclusions cover their named semantic outcomes, not hardware failures,
termination, cleanup completion, availability, confidentiality or recovery in
general. A Trap exclusion does not exclude Abort or ProcessExit. Existing
[component custody](component_publication.md) and external survivor obligations
remain required regardless of which exclusions were requested.

## Assertions and optional logging

An assertion is ordinary checked code. A shared requirement for checking and no-op
implementations may permit guarded Trap without requiring it. Public wrappers
retain the same explicit allowances across configurations. The build can then
reject the checking composition and accept the verified no-op composition under
a Trap exclusion, without altering those source declarations. It still rejects
any unrelated possible Trap in the selected closure.

A shared checking/no-op requirement cannot promise that normal return establishes
the asserted condition. Validation needed to justify subsequent safe operations
is required program behavior, not optional diagnostics. Ordinary arguments are
eager: selecting a no-op does not erase their effects, crashes, divergence, borrow
validity, or ownership obligations. Optimizations may discard safely removable
computation under their usual rules, not because the consumer is called assert.
Explicit delayed computation may use ordinary named machine arguments; work
outside that delayed call still occurs. No zero-overhead guarantee is implied.

Assertions are not generated from arbitrary preconditions, postconditions or
admissions. An assertion repeating an admitted fact may disappear under ordinary
optimization; preserving a check against assumptions requires a separate design.
Failure introduces no implicit unwind, flush, recovery, or authority. The settled
[admission and diagnostics rules](../proofs/contracts.md#admission-and-runtime-diagnostics)
remain unchanged: admissions require no runtime checking.

## Implementation acceptance

| Case | Required result |
| --- | --- |
| Same source and public Trap ceilings; checking implementation selected | A Trap-excluding build rejects a possible failing assertion. |
| Same source and public Trap ceilings; no-op selected | Pass if the entire selected closure excludes Trap, without optional optimizations. |
| No-op selected but predicate evaluation or another helper can Trap | Reject; no assertion-specific omission. |
| Trap is not excluded by the build but violates an intermediate API | Ordinary contract checking rejects. |
| Ordinary silent logger with a broad Console allowance | Pass a Console exclusion when complete verified implementation evidence establishes no Console invocation. |
| Silent provider selected for an actual Console boundary invocation | Reject a Console exclusion; physical silence is a different property. |
| No process-output mechanisms, but a Console invocation remains | May satisfy the physical class exclusion while failing the service exclusion. |
| Unknown foreign behavior or precompiled broad allowance only | Refuse absence certification; report the evidence gap. |
| Callback, startup, cleanup or generated entry introduces a forbidden behavior | Reject with provenance to that contribution. |
| Reordered or duplicate bans; another target child added | Same canonical restriction set and independent child outcome. |
| Optimization families enabled or disabled | Same exclusion admissibility for the same semantic selection. |
| Stale scope, provider, entry, policy or evidence substituted | Independent consumer rejects. |
| Replacement introduces a previously excluded behavior | Reject under the existing installation envelope. |
| Only Trap is excluded, but another terminal cause remains | No false all-crash or recovery guarantee. |

Exercise real source selection, dependency compilation, generated source,
independent evidence consumption, and physical realization where required; a
hand-built empty summary does not close the customer. Exact API/codec work and
implementation limits belong to the owning tasks. Any unresolved semantic or
trust amendment must be raised in [owner questions](../../../OWNER_QUESTIONS.md)
before relying on it, not silently chosen by an implementation.