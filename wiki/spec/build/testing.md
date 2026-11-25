# Requirement-based test groups

Tests are ordinary checked machines satisfying an explicitly registered machine
requirement. Build configuration selects test roots and their execution
environment; it does not introduce a machine species, trusted boundary, or
special calling permission. The runner executes each enabled test across its
configured target/engine plan before reporting the requested build successful.

Interpreted runner instances use the same [embedding lifecycle](embedding.md) as
other Psi consumers, with selected group providers and fresh guest state. Native
runs use separate test products with their own admitted execution resources.
Test discovery and verdict policy do not justify a second interpreter or weaker loan/custody
rules. A fresh guest instance does not roll back effects in a genuine host provider.

## Registration and discovery

Register an exact ordinary trait machine requirement, not a trait name or a
method-name convention. Ambiguous overload paths reject rather than selecting
by visible satisfiers:

```omega
let memory_tests: TestGroup = builder.tests.group::<Tests::memory>();
```

`group` is a compiler-owned Build operation on the `tests` facet. Its
requirement operand uses the existing
[non-executing product-reference rules](scoped_execution.md#selecting-product-declarations-without-executing-them).
It resolves under the author's product scope to an exact declaration application.
A same-named user machine gains no product-selection privilege. Registration is
evaluated configuration in the same Build activation, not a new dependency edge.
Its product references remain inert until the final dependency join.

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

A runnable entry has no observed result and receives only arguments established
by its explicit installed runner contract. A parameterless test constructs its
fixture in ordinary local or static storage. A parameterized root is an installed
ordinary call whose contract supplies each argument and its custody; a mutable
parameter alone requests no allocation. Reuse
[entry storage rules](entry_roots.md#entry-shape-and-arrival-bridge), including
ordinary initialization and WCSU. Boundary calls in the checked test
closure demand exact provider slots independently of that context; missing,
ambiguous, or unauthorized supply rejects before invocation. Provider selection
constructs no source routing value. Other fixture construction and parameterized helper
calls are ordinary test code.

A Build can install an ordinary adapter with
`builder.tests.group::<Checks::modify, registers_fixture>()`. The adapter takes
one static machine parameter. Build specializes it once for each concrete
satisfier of the selected requirement and checks the resulting entry against the
target's ordinary arrival contract. The adapter can initialize a local fixture,
lend it to the selected machine, and inspect it on handback. Every normally
returning path through the adapter must invoke that exact satisfier, directly or
through another checked machine. Group registration and reported results retain
the identity of the individual satisfier.

For an explicitly fixture-bearing installed requirement, the registered exact
trait application may name the fixture type as an ordinary trait argument, for
example `Tests<ScanFixture>::memory`. The runner's installation must separately
provide its actual argument. Every satisfier matches that declared parameter.
Register different concrete applications separately; the runner neither infers
a data subject nor guesses generic arguments or synthesizes a whole-trait map.

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

Std's optional `testing\Test::run` is an ordinary convenience requirement:
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
it does not make dependency requests target-dependent or grant ambient environment
access. This contract introduces no debug/release mode or special CLI profile.

<a id="test-runner-filesystem-slot"></a>

The test harness uses [ordinary provider selection](provider_selection.md#declaration-and-choice)
for the test root's exact boundary requirements. Each selection belongs to
the registered group and target's test product. For a test that calls
`FilesystemService`, the harness selects a conforming provider and supplies
the filesystem backing within its actual grant. Selection checks the provider's
contract. The runner checks the resources delegated to the test separately.
The application keeps its own provider selections.
`builder.tests.service<Group, Slot>()` is retired. A matching trait name,
package name or schema cannot cause the runner to supply a filesystem.

An authored Build sets `group.inputs.private_filesystem = true` to supply an
empty private filesystem for that group. The field defaults to false. This
configures backing independently of the provider selected for a boundary.
Without an authored stdin the Linux executor supplies an empty input pipe.
It captures stdout and stderr separately. It exposes only the private namespace to filesystem operations.

When the harness selects a private filesystem, the runner arranges for its
provider to start with an empty filesystem for **each test invocation**, on
both native and Psi. Setup supplies fresh backing before the test body runs.
Fixtures and callees share writes within that invocation. Other invocations,
including parallel tests and the other engine, get independent state. The
provider implements the selected filesystem contract over this state, including
directory operations, metadata and errors. Benign private backing and captured
output require no separate approval. A group that uses no filesystem needs no
filesystem setup. Disabled groups still undergo ordinary source checking,
receive no execution grant and are reported as disabled.

[Build attenuation](scoped_execution.md#inputs-and-default-filesystem) applies
throughout the test's provider and dependency closure. A child can pass on its
virtual backing or restrict it further. Its ordinary filesystem calls operate
on that delegated view. Admission rejects a raw host provider that needs an
ungranted resource before execution. An unsupported operation on an admitted
virtual view reports the contract's error without falling back to the host.
The same rule applies to console, network, process and credential services.

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
an admitted execution protocol with enforceable scope. Selecting native execution
grants no additional authority: unsupported providers cannot escape their
configured backing through native code or ambient host APIs.

Tests containing [checked assembly](../language/assembly.md#interpreted-execution)
use the same supported instruction semantics and explicitly supplied environment.
Private model state is fresh per test unless an admitted shared resource contract
says otherwise; it persists between that test's blocks and work-budget pauses. A model's
machine-owner authority is not host machine authority. Unknown operations or
missing environment support reject before execution, never pass as no-ops.
Simulation does not validate real hardware timing or device behavior.

## Execution plan

Target environment and execution engine are independent, as in
[embedding](embedding.md#target-environments-and-execution-engines). For each
enabled group and selected target, the default is to run every applicable,
permitted engine available on the executing host, including interpreted and
native. Cross-compilation support alone is not execution availability. Ordinary
Build group configuration can narrow engines or require particular target/engine
combinations. Unset engine configuration uses this default; it does not disable
tests. Exact API spelling is not a new language construct.

For example, a Build can require native execution for one target or exclude
interpretation for that target through the handle it received at registration:

```omega
let group: TestGroup = builder.tests.group::<Tests::memory>();
TestGroup::engine(group, linux_x86_64, TestEngine::Native, TestEnginePolicy::Required);
TestGroup::engine(group, linux_x86_64, TestEngine::Psi, TestEnginePolicy::Disabled);
```

Setting the same target and engine to `TestEnginePolicy::IfAvailable`
restores its default. Each setting applies to the named group, target and
engine. A comparison that requires an explicitly disabled engine is incomplete.

Before test effects, publish the execution plan with each test's exact group,
target, engine, selected bindings and status: scheduled, explicitly disabled,
or unavailable with a reason. Missing host adapter or engine support is not a
test pass. An unavailable required combination makes the run incomplete and
prevents successful build publication; an optional unavailable combination stays
visible without blocking another engine's run. Every enabled test must execute
on at least one engine, and every scheduled invocation must pass. A started
invocation that fails or exhausts its budget cannot be reclassified as optional
unavailability after the fact. Malformed bindings or missing authority reject;
availability probing must not conceal an admission failure.

Both engines run the same test contract and inputs against the configured
services, with fresh instance-local state. Engine-specific adapters must honor
those services and exact selected foreign bindings; a virtual filesystem cannot
become a real filesystem by choosing native execution. There is no silent
fallback or engine-specific weakening of the test oracle. Report each engine's
outcome separately; deterministic comparison observations must agree. Neither
engine executes merely because its compilation succeeded.

### Comparing native and Psi tests

`omega build --compare-tests <root.omg>` asks the Build test runner to compare
native and Psi execution for each selected, enabled test on its selected
target. The owner ratified `build-test-comparison` on 2026-11-22. The unit of
comparison is a registered test. The runner executes an application entry only
when the test explicitly exercises it. A project with no selected tests has
zero comparisons.

Run each test twice with the same arguments, stdin bytes, initial files and
configured service inputs. Give each execution its own copy of mutable fixture
state so a write in the first run cannot affect the second. The runner compares
three kinds of observation for the named test and target.

In an authored Build, `TestGroup::stdin(group, bytes)` snapshots a finite byte
view for each invocation. `TestGroup::expect(group, TestOutput::Stdout, bytes)`
and the corresponding `Stderr` selection set an independent expected byte
sequence. The last setting for a stream in that group wins. An explicit empty
expectation requires zero bytes. Omitting it leaves the stream available for
engine comparison without imposing a group-wide expected value.

With private filesystem backing, `TestGroup::seed_file(group, path, bytes)`
copies a regular file into every invocation before the test starts. Parent
directories are created in the private root. Paths are canonical relative byte
names. A repeated seed for the same path replaces its initial bytes.

`TestGroup::observe_file(group, path)` compares the engines' final observation
of that path. `TestGroup::expect_file(group, path, bytes)` also checks each
execution against the expected bytes. `TestGroup::expect_missing(group, path)`
requires absence. The last output declaration for a path applies. An empty
expected byte view requires a present, empty file. File expectations select
observations independently of the group's filesystem backing and provider choice.

- **Termination.** Compare normal return, exact process exit status, or crash
  as appropriate for the invocation. If the host loses information needed to
  decide, as with a truncated exit status, report the comparison as unavailable.
- **Standard streams.** Compare stdout with stdout and stderr with stderr,
  byte for byte. Preserve the two streams separately.
- **Declared outputs.** Compare each additional observation the test declares,
  such as the bytes of `disk.img`, a returned value, or the final value of an
  observed field. File presence is observable too. A missing file differs
  from an empty file.

Report each engine's result against the test's expectations and report the
comparison separately. If both exit zero but print different bytes, report
the first differing stream and byte offset. If the observations match but both
print `99` when the test expects `42`, the runs agree and the test fails. Agreement covers only
the inputs and observations compared.

An explicit comparison requires both executions. A mismatch fails validation.
If an engine cannot run on the selected target, or the runner cannot obtain
comparable inputs or a required observation, report the reason and leave
validation incomplete. Compiling an artifact earns no comparison credit.
Build can report a successful requested comparison only after every required
pair agrees and each invocation passes its own test.

## Scheduling, execution, and outcomes

Automatic build-time tests cannot execute
[assumed binding contracts](assumed_binding_contracts.md#runtime-only) or use
their assumptions, including through fixtures, helpers, provider selections and
erased evidence. Runtime acceptance and live-host grants do not override this
restriction. A separately executed runtime harness is a runtime product under
its own acceptance, not permission to launch the prohibited relationship inside
the automatic build-time runner.

The lifecycle is:

1. Execute admitted build configuration and freeze its group selections.
2. Finish source checking and discovery; construct separate test roots and
   provider closures for each selected package/target occurrence and expose
   its target/engine execution plan.
3. Reconstruct test-time authority requests, check lock agreement and actual
   grants, and verify Psi and entry provisioning before execution.
4. Run every scheduled test/engine invocation with fresh activation, context,
   captured-output and mock-provider state. Settle its ordinary owned resources
   on normal completion.
5. Require the execution plan's pass and coverage conditions before publishing
   the new successful build result set. Preserve diagnostics on failure; live
   effects have no rollback.

This is execution of ordinary target-semantic code by the selected engines,
not [hermetic semantic-evaluation admission](../language/evaluation.md).
Possible `Trap` and service reach are legitimate declared test behavior, not
grounds for pretending the invocation is a constant evaluation. Provider and
target semantics remain explicit; a host-native-only operation does not acquire
a Psi implementation by being used in a test. Its native invocation may run
when scheduled and admitted, while interpretation is reported unavailable;
whether that prevents publication follows the explicit execution plan.

Tests inherit sponsor-controlled work and storage limits; unset limits inherit
policy rather than mean unlimited. Exhaustion is not catchable test behavior.
No termination proof is inferred from a timeout, and no termination guarantee
is waived when one is declared. Parallel execution must preserve per-invocation
state and actual authority isolation. Report results in test/target/engine roster
order, not host completion order; registration supplies no cross-test shared fixture state.
Normal return is a pass only after required child activations and custody have
settled or transferred under the ordinary contract.

| Observation | Result |
| --- | --- |
| Normal completion and settled obligations | Pass for this invocation only. |
| Contract-permitted `Trap` or `Abort` | Failed test; prevent successful build publication. |
| Process exit instead of returning to the runner | Not a pass; report the terminal outcome. |
| Work/storage exhaustion during a scheduled invocation | Incomplete execution, distinct from an assertion failure; no successful build. |
| Required engine unavailable, or enabled test has no runnable engine | Incomplete execution; no successful build. |
| Optional engine unavailable before execution | Report not run and the reason; never count as a pass. |
| Malformed contract, missing authority, or invalid evidence | Reject before affected execution. |
| Interpreter/checker invariant failure | Internal failure, never an ordinary test failure. |
| Disabled group or explicitly excluded engine | Not run; never reported as passed. |

Trap handling belongs to the runner's containment boundary, not language-level
unwinding or cleanup. It cannot reclaim externally held resources merely by
discarding interpreter memory or assuming a native trap is recoverable. Continue
other tests only when the failed activation can be safely contained; otherwise
stop without successful publication.
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
Interpreted tests execute Psi; native tests lower separate runner roots
to a test executable. Their exclusive entries, fixtures and providers are not
roots of the published application Psi or native image. Do not rely on optional
dead-code elimination or remove still-called code
after testing. If application code calls a test machine, that ordinary reachable
code belongs to the application and must satisfy its contracts and exclusions.

Reuse source preparation and compatible checked/lowered representations. A
different target, provider closure, root-sensitive fact or optimization premise
requires the affected work again; equal source bytes alone do not permit reuse.
Produce a native test image only for a scheduled native run; never append it
to the application image. Interpreted success alone is not a claim that native
lowering, ABI, hardware, or foreign-provider behavior was exercised. Native
success with virtual services likewise does not validate the real device.

A custom native harness is an ordinary separately selected executable with an
ordinary entry binding. It may call visible test machines or shared helpers under their
usual contracts. It does not mark its main as a test, gain private visibility, or
implicitly collect the groups. This differs from the build runner's generated
test product, which uses the registered roster and owning-package test scope.
Neither creates a special source-level test main. Native execution needs its own
real containment and authority; an in-process trap does not become recoverable
by convention.

## Compiler tests

The repository's own compiler behavior is tested by Rust tests on the code
that implements it. A test that needs an Omega program reads an ordinary
package kept under `tests/fixtures/<tier>/<group>/<name>/`: a `pass` case must
check, a `fail` case must reject with its expected diagnostic fragment, and a
runnable case (the `run` tier and `*_exit` pass cases) has an ordinary program
entry whose exit status and output are its result. Where a runnable case
executes natively and on the interpreter, the repository harness compares
[the same observations](#comparing-native-and-psi-tests) and checks the case's
expected result. This also allows one-off program comparisons in development
tooling without treating every application entry as a registered test.
Native execution on the host proves nothing about another target, so a
target-specific result belongs to a case built for that target.

The fixture corpus that was this suite until 2026-11-13 (`tests/omega`) is
deleted; `tests/README.md` records how to read it from history.

## Implementation acceptance

Use one small package with two satisfiers of one registered requirement, then
vary it through the same path: a failing check, a context with a virtual
filesystem, and a second group. Require exact discovery, fresh state, distinct
failure/incomplete diagnostics, and application outputs free of test-only roots.
Keep negative controls for inherited-identity aliases, unresolved generic tests,
dependency test non-discovery, private access, missing service establishment,
child attempts to widen virtual authority, and unaccepted live-host requests.
Run the no-std variant and an ordinary native harness calling a shared test
machine. Helper tests alone do not deliver this build-to-execution contract.
Run a pure group and a virtual-filesystem group on both engines, reporting
separate results and matching deterministic observations. Have two tests each
create the same file exclusively, write a different value and read back their
own bytes. Both must pass in either order and the host file at that path must
retain its original contents. Repeat through a child with the delegated view.
Cover a native-only binding with optional and required interpretation, explicit
engine exclusion, no runnable engine, and a native attempt to escape virtual backing. Inspect
both application products for absent test-only roots; a scheduled native test
failure or budget exhaustion must not turn into a successful optional skip.
