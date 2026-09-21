# Requirement-based test groups

Tests are ordinary checked machines satisfying an explicitly registered machine
requirement. Build configuration selects test roots and their execution
environment; it does not introduce a machine species, trusted boundary, or
special calling permission. The runner executes verified Terminal Psi before
reporting the requested build successful. These are accepted contracts;
[implementation tasks](../../../TASKS.md#requirement-based-tests) track delivery.

Runner instances use the same [embedding lifecycle](embedding.md) as other Psi
consumers, with selected group providers and fresh guest state. Test discovery
and verdict policy do not justify a second interpreter or weaker loan/custody
rules. A fresh guest instance does not roll back effects in a genuine host provider.

## Registration and discovery

Register an exact ordinary trait machine requirement, not a trait name or a
method-name convention. Ambiguous overload paths reject rather than selecting
by visible satisfiers:

```omega
let memory_tests = builder.tests.group<Tests::memory>();
memory_tests.service<Filesystem, MemoryFilesystem>();
```

`group` is a compiler-owned Build operation. Its requirement operand, and the
service/provider operands of `service`, use the existing
[non-executing product-reference rules](scoped_execution.md#selecting-product-declarations-without-executing-them).
They resolve under the author's product scope to exact declaration applications,
not host imports, strings, spelling matches, or evaluator table indices. A
same-named user machine gains no product-selection privilege. Registration is
evaluated configuration after dependency discovery, not a new dependency edge.

Each registration creates one group for the current package occurrence and
selected target. Its configuration remains owned by that Build activation;
the normalized result retains selections, not an escaping mutable handle.
Duplicate registration of the same requirement application rejects. One trait
may contain several independent group requirements with different contracts.
There is no privileged `run`, `default`, `Test`, or test attribute.

After the package's final source roster is checked, collect every concrete,
executable, package-local machine with a checked exact satisfaction edge to the
registered requirement. Multiple satisfiers are separate tests, not ambiguous
provider candidates. An individual test need not supply a whole-trait
conformance. An inherited requirement retains its declaring identity; an alias
or wrapper trait does not manufacture a second group identity.

Discovery includes checked generated satisfiers of an already registered
authored requirement, but cannot retroactively change the admitted build or
register a requirement introduced by that build's generated source. Private
tests run in their owning package's test scope without becoming public or
granting other tests access to private declarations. Imported dependencies'
tests are not automatically collected. A workspace member is tested when
selected as a build root, not merely because it is a dependency or catalog entry.

Discovery retains exact machine, requirement, package, and target identities
and reports a deterministic roster. It does not guess generic arguments or
enumerate possible specializations: an open generic satisfier needs an ordinary
concrete wrapper before it is runnable. Report non-runnable candidates rather
than silently omitting them. A registered group with no tests reports zero
tests; it is not evidence that any test passed.

## Entry contracts and ordinary calls

A runnable entry has no ordinary invocation arguments and no observed result.
It is either a free machine or an attached machine with one `&mut self`
receiver. The requirement declares the corresponding shape. Reuse
[entry provisioning](entry_roots.md#entry-shape-and-arrival-bridge): initialize
ordinary receiver fields only when ZII-valid and establish each direct
`Binding<R>` field against the selected authorized binding before entry.
Provider selection alone cannot construct arbitrary service values. Missing
bindings or unestablishable receiver fields reject before invocation. Other
fixture construction and parameterized helper calls are ordinary test code.

For a receiver requirement, each satisfier identifies its concrete receiver
carrier through ordinary `Self` matching. That is not a guessed generic test
argument or a synthesized implementation of the whole containing trait.

The runner establishes all entry preconditions; registration grants none.
Requirements retain ordinary reach, crash, suspension, blocking, termination,
ownership, and resource contracts. A group may permit `Trap`; `satisfies`
inherits that contract, not permission to bypass an intermediate contract.
Services required by a test must fit both its requirement and the actual grant.
Inheritance cannot widen a parent's service-free requirement.

Trait satisfaction, entry checking and execution keep their existing owners.
Psi owns source checking and portable representation/verification/interpreter
semantics. Build/product orchestration owns registration, discovery scheduling,
provider closure and grant admission, result aggregation and publication.
Libraries own test bodies, group contracts and fixtures. Reuse these owners;
do not put build policy in the pure evaluator or add a parallel test compiler.

Any ordinary machine may call a test machine subject to normal visibility and
contract checking. A test entry is a root for the runner, not a bodyless
`boundary machine`, a new trusted assumption, or a privileged function.
No test-only main, call permission, or automatic crash allowance is introduced.

Std's optional `testing::Test::run` is an ordinary convenience requirement:
parameterless, resultless, service-free, permitting `Trap`. It is not core
vocabulary, a compiler-recognized identity, or a fixed taxonomy of categories.
Projects may register it, arbitrary project requirements, or both. Templates
may author its ordinary dependency and registration; the compiler invents
neither. Naming a requirement `default` does not register it.

## Group configuration and authority

Groups are enabled on registration. `group.disabled` and
`builder.tests.disabled` default to false, so ZII configuration does not silently
disable testing. Effective enablement requires neither disable flag to be set.
Configuration may use ordinary admitted build computation and explicit inputs;
it does not make dependency discovery conditional or grant ambient environment
access. This contract introduces no debug/release mode or special CLI profile.

`service<Requirement, Provider>()` requests a checked provider selection within
the group's scope. Ordinary provider compatibility, uniqueness, establishment,
and admission apply. It grants no capability. Benign captured output and a
private in-memory filesystem require no approval ceremony, but are explicitly
selected rather than inferred from names. Disabled groups create no execution
grant and are reported as disabled, not passed; their declarations remain
subject to ordinary source checking.

Test execution is governed by the same authority hierarchy as other build-time
work. The root selects backing services and delegates bounded capabilities;
descendant package builds and groups can only attenuate them. Selecting a host
provider in a child cannot replace a root-selected virtual filesystem. Operations
supported by the delegated virtual interface use that backing; real-only or
otherwise unavailable operations reject without a host fallback. This applies
equally to console, network, process, credentials, and other services.

Restricted requests from the enabled test closure, including its fixtures and
providers, enter [install/update review](../packages/acceptance.md#restricted-build-acceptance)
before execution. Retain their package, test purpose and group, logical scope,
limits, and target/execution context. New or widened requests require matching
lock acceptance and actual executor grants. A dependency's lock, runtime
permission, group declaration, or std implementation supplies neither. Missing
analysis is not an empty request. Disabled/unselected configurations make no
execution claim; later enabling them requires fresh applicable checking.

Fresh interpreter state does not isolate arbitrary foreign code, erase live
external effects, or roll back a real filesystem. Host-backed operations need
an admitted execution protocol with enforceable scope. Unsupported providers
cannot execute by escaping to native code or ambient host APIs.

## Scheduling, execution, and outcomes

The lifecycle is:

1. Execute admitted build configuration and freeze its group selections.
2. Finish source checking and discovery; construct separate test roots and
   provider closures for each selected package/target occurrence.
3. Reconstruct test-time authority requests, check lock agreement and actual
   grants, and verify Terminal Psi and entry provisioning before execution.
4. Run every enabled test with fresh activation, receiver, captured-output and
   mock-provider state. Settle its ordinary owned resources on normal completion.
5. Require all enabled tests to pass before publishing the new successful build
   result set. Preserve diagnostics on failure; live effects have no rollback.

This is execution of ordinary target-semantic code by the Terminal Psi runner,
not [hermetic semantic-evaluation admission](../language/evaluation.md).
Possible `Trap` and service reach are legitimate declared test behavior, not
grounds for pretending the invocation is a constant evaluation. Provider and
target semantics remain explicit; a host-native-only operation does not acquire
a Psi implementation by being used in a test. Unsupported execution is reported,
never a pass or a silent native fallback.

Tests inherit sponsor-controlled work and storage limits; unset limits inherit
policy rather than mean unlimited. Exhaustion is not catchable test behavior.
No termination proof is inferred from a timeout, and no termination guarantee
is waived when one is declared. Parallel execution must preserve per-invocation
state and actual authority isolation. Report results in roster order, not host
completion order; registration supplies no cross-test shared fixture state.
Normal return is a pass only after required child activations and custody have
settled or transferred under the ordinary contract.

| Observation | Result |
| --- | --- |
| Normal completion and settled obligations | Pass for this invocation only. |
| Contract-permitted `Trap` or `Abort` | Failed test; prevent successful build publication. |
| Process exit instead of returning to the runner | Not a pass; report the terminal outcome. |
| Work/storage exhaustion or unsupported execution | Incomplete execution, distinct from an assertion failure; no successful build. |
| Malformed contract, missing authority, or invalid evidence | Reject before affected execution. |
| Interpreter/checker invariant failure | Internal failure, never an ordinary test failure. |
| Disabled group | Not run; never reported as passed. |

Trap handling belongs to the runner's containment boundary, not language-level
unwinding or cleanup. It cannot reclaim externally held resources merely by
discarding interpreter memory. Continue other tests only when the failed
activation can be safely contained; otherwise stop without successful publication.
Passing does not prove universal correctness, justify an axiom, or grant authority.

## Assertions and trusted claims

A test verdict must use a mandatory checked comparison or validation machine,
not a checking/no-op diagnostic whose no-op selection would silently pass it.
The existing [assertion contracts](behavior_exclusions.md#assertions-and-optional-logging)
remain unchanged, including eager arguments, public crash allowances, and
independent product exclusions. Production may exclude `Trap` while the separate
test closure permits it; neither setting rewrites callable contracts.

Tests of admitted guarantees must not assume the guarantee being tested to
establish their verdict. Use the existing
[validation boundary](../proofs/contracts.md#admission-and-runtime-diagnostics)
without the disputed assumption. Registration adds no optimization barrier or
alternate proof semantics. Tests may exercise algorithms whose memory safety
is already proved; functional results remain a separate subject.

## Products, reuse, and native harnesses

Test roots and application roots are separate selections over checked source.
Automatic tests execute at the Terminal Psi boundary; their exclusive entries,
fixtures and providers are not roots of the published application Psi or native
image. Do not rely on optional dead-code elimination or remove still-called code
after testing. If application code calls a test machine, that ordinary reachable
code belongs to the application and must satisfy its contracts and exclusions.

Reuse source preparation and compatible checked/lowered representations. A
different target, provider closure, root-sensitive fact or optimization premise
requires the affected work again; equal source bytes alone do not permit reuse.
The automatic test path needs no native test image. Test success is not a claim
that native lowering, ABI, hardware, or foreign-provider behavior was exercised.

A native harness is an ordinary separately selected executable with an ordinary
entry binding. It may call visible test machines or shared helpers under their
usual contracts. It does not mark its main as a test, gain private visibility, or
implicitly collect the groups. Native execution needs its own real containment
and authority; an in-process trap does not become recoverable by convention.

## Implementation acceptance

Use one small package with two satisfiers of one registered requirement, then
vary it through the same path: a failing check, a receiver with a virtual
filesystem, and a second group. Require exact discovery, fresh state, distinct
failure/incomplete diagnostics, and application outputs free of test-only roots.
Keep negative controls for inherited-identity aliases, unresolved generic tests,
dependency test non-discovery, private access, missing service establishment,
child attempts to widen virtual authority, and unaccepted live-host requests.
Run the no-std variant and an ordinary native harness calling a shared test
machine. Helper tests alone do not deliver this build-to-execution contract.
