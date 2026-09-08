# Process exit

[Service reach](effects.md), [termination](termination.md), and
[ownership](ownership.md) remain independent contracts. Process exit is a
non-crashing external terminal outcome, distinct from normal return and
divergence. Successful exit means the declared transfer completed, not that the
status was zero, cleanup ran, or an application protocol succeeded.

## Canonical boundary and authority

Core owns the canonical `ProcessExit` boundary trait and its exact
`ProcessExit::exit_process(return_code: i32)` requirement. It owns the declaration
and semantic identity, not process management or platform implementation.
Psi recognizes the exact admitted canonical requirement, including package and
contract identity; a matching path, method spelling, or ordinary user declaration
does not acquire its meaning.

Invocation requires actual authority bound to the caller's containing process
execution domain by the admitted entry/runtime environment. Importing core,
declaring `reaches ProcessExit`, or selecting a provider grants no authority.
Embedded components receive none by default; the host must explicitly delegate
it through the ordinary capability machinery.

The operation ends that bound domain, including its active and suspended
activations. It does not automatically end child processes or other domains.
A provider cannot reinterpret it as worker exit, remote-provider exit, killing
another process, CPU halt, or reboot. Targets without a matching process
abstraction do not supply this service.

Console I/O does not imply process-exit reach or authority. Any retained
console-exit convenience adapter must obtain legitimate exit authority and
publish its additional reach. Console must not inherit `ProcessExit` wholesale.

## Progress and conditional calls

Under its requirements and admitted provider progress premises, the canonical
operation eventually ends the bound domain. It produces no normal result or
continuation and permits no source crash. Abrupt exit means no implicit cleanup,
not instantaneous completion, bounded latency, or an atomic stop of all
activations. Preceding observable events and the exact exit argument survive.

`terminates` continues to promise eventual arrival at a permitted terminal
outcome under the invocation's premises. It neither authorizes that outcome nor
promises normal return. `terminates by ...` supplies a ranking witness where
needed; callee/provider progress remains independently required. An exit leaf
needs no further cycle decrease, but operand evaluation and calls before it
still need progress evidence.

`reaches ProcessExit` is an ordinary conservative service ceiling, propagated
through callees and provider contracts. It permits exit without asserting that
any invocation necessarily exits. The caller must establish the ceiling, actual
domain-bound authority, argument requirements, and applicable crossing
obligations; it need not prove the entire process resource-free.

A helper may exit on one branch and return on another. Its normal continuation,
result, and cleanup exist precisely when it returns; only the exit branch has no
successor. A public reach ceiling alone cannot prove which arguments select
return. Greater precision requires checked body knowledge or a supported pinned
contract refinement, not inference from a selected provider. No guarded reach
syntax is introduced. Termination must cover every admitted path; reachable
divergence defeats an unconditional promise.

Normal-return `ensures` clauses keep their partial-correctness meaning. Exit
does not establish them and does not necessarily violate them. An unconditional
eventual-release or protocol-completion guarantee must account for terminal
outcomes explicitly or exclude abrupt exit.

## Abandonment and survivors

Process exit is an explicit domain-ending abandonment outcome, not an ordinary
resource consumer. It may abandon outstanding in-domain obligations, including
linear obligations, without cleanup or discharge claims. It creates no release,
cancellation, consumption, or protocol-completion receipt. Resources already
transferred to surviving owners are not reclaimed by this operation.

Exact-once disposition remains required on ordinary continuing and returning
paths. An exit branch has no post-exit ownership frontier to merge into a
returning branch. Audit evidence may retain a statically reconstructed local
frontier lower bound, but must not call it the complete process-wide obligation
set or proof that unlisted state remains safe.

Exit authority cannot invalidate guarantees relied upon by surviving components.
Crossing contracts must account for domain death through isolation, retained
custody, owner-death recovery, or reset; incompatible crossings reject. Ending
the process proves neither shared-memory validity, DMA cessation, nor release
of external locks. These are obligations at the actual verified/admitted
boundary, not a compiler guarantee about every host or peer.

Deployment policy may prohibit abrupt abandonment or accept lost work and
external capacity under its contracts. It cannot silently waive a safety premise
another verified component relies on. Reuse the existing
[execution-domain recovery rules](effects.md#recovery-and-execution-domains);
no universal exit-safety trait or per-call whole-process resource census is added.

## Graceful completion and cleanup

Graceful shutdown is ordinary checked program logic: stop work, settle tasks,
flush or close resources as required, then return through the
[entry bridge](../build/entry_roots.md#entry-shape-and-arrival-bridge).
Entry return alone does not establish that other activations were settled.
The root/runtime contract must settle or legally transfer their custody before
physical process completion; the bridge cannot conceal abandonment.

Automatic cleanup must return normally, excluding both crashes and external
completion. Its transitive contract cannot permit process exit merely because
exit is terminating and non-crashing. There is no implicit unwind, exit-handler
registry, or compiler-inserted cleanup of caller frames.

## Verification and implementation boundary

The exact canonical requirement owns the public terminal-event identity, not
the selected provider or a separately named effect. Checking and Terminal Psi
retain an explicit closed external-completion kind and a terminal invocation
with no normal result. An ordinary Unit call is not that representation; the
special canonical contract does not change result omission for other machines.
No `completes` keyword or public `never` type is introduced. Epsilon's independent
bootstrap `never` rules are unchanged.

Provider conformance preserves the terminal outcome, exact arguments, progress,
and preceding trace. Returning mocks, divergence, or abort cannot substitute.
An interpreter ends its simulated domain without killing the test runner.
A defensive trap after an unexpectedly returning native exit contains a violated
provider premise; it is not another permitted source outcome.

[Terminal observations](../terminal-psi/observations.md#process-exit-observations)
compare the exact semantic `i32` status. A host exposing fewer status bits does
not authorize truncation of that trace; physical presentation belongs to the
target contract.

This is the required contract, not a claim of implemented support. The bundled
Unit `Console::exit_process` path and the zero terminal-external codec group
remain transitional. The source, capability, provider, and verifier migration
is tracked by [PROCESS-EXIT-CONTRACT](../../../TASKS.md#process-exit-contract).
General completion annotations, separately named terminal effects, thread exit,
reboot, and arbitrary process killing are outside this decision.