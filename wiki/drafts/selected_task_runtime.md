# Selected task-runtime design

Design record for **TASK-RUNTIME-NATIVE-SUPPORT**: what selecting a
`TaskRuntime` provider means today, what the selection retains, and which
joins remain before a selected runtime executes activations natively.
Contract: [task activation and lifecycle](../spec/build/task_runtime.md);
source surface: `source/library/core/task.omg`; implementation note:
[task-plans](../../omega-rust/omega/representations/task-plans/README.md).

## Selection model

`TaskRuntime` is an ordinary `boundary trait` (`core/task.omg`) whose
`start`/`try_start` take a compile-time machine-symbol argument
(`machine Target(arguments: Arguments) -> T suspends; blocks`) — never a
runtime function value or capture inference. A root profile may select a
default provider and an owner may override the slot, but selection grants
no ambient authority: `start` still requires a held runtime capability,
and static binding retains the exact selected plan plus the authored
requirement. Missing or duplicate selection, requirement drift, and
narrowing of the published machine contract all reject.

Selection and custody are separate questions. The contract fixes three
owners — runtime custody (park/wake/cancel under the provider contract),
physical storage (accountable backing and lifetime), and the linear
`Task<T>` claim — so arena pools, hosted providers, remote executors, and
inline execution satisfy the same contract without sharing a storage
representation.

## What a selected runtime carries today

`omega-rust/omega/representations/task-plans` is the post-check state a
selection produces:

- **Activation plan.** Per monomorphized machine argument: exact contract
  and entry, argument/outcome layouts (`TaskArgumentLayout`), target
  calling plan, whole-call-graph `StackPlan<M>` demand, canonical
  suspension crossings with per-place `LiveCarryDemand` frontiers, the
  four-axis CPU/thread preservation demands, and cancellation behavior
  required by the selected start operation. `05_task_activations.json`
  and `05_carry_manifest.json` report these; emission is evidence, not
  custody.
- **Static binding.** The domain-separated SHA-256 specialization
  commitment binds operation, exact checked requirement, package-qualified
  ownership, and target/entry signature with parameter modes; compact
  coordinates are report fingerprints only.
- **Instance ledger.** One admitted runtime instance owns its ledger plus
  fixed stack provisioning (`provider_admission.rs`): fresh lease eras,
  slot spend and claim minted inside one admission, conservation of moved
  arguments on every rejection. Settlement burns the era; reuse requires
  a fresh one — old evidence never replays.
- **Claim routing.** `accept_invocation` mints the exact
  `provider`/`activation` field pair a source `Task<T>` carries; the pair
  resolves back to exactly one live claim, and foreign-instance,
  fabricated, or settled pairs fail closed (`claim_route.rs`). A provider
  serving `request_cancel`/`finish`/`settle` needs no shadow map.
- **Lifecycle states.** A claim parks only at a canonical crossing of its
  plan, resumes the same invocation under unchanged bindings, and cannot
  settle while parked. `observe_cancellation` records where a recorded
  cancel request was observed, so `Cancelled` can never be fabricated for
  a never-suspending or inline-completed activation.

## What selection does not yet execute

The ledger models transitions a provider would drive; nothing performs
them on a real stack. Missing joins, in dependency order:

1. **Physical invocation.** The marshalled `MovedTaskArguments` image is
   provider-domain custody; a real runtime must write it into the
   activation's argument area and enter the activation on its fixed
   nonmoving stack. No `run`-leg crosses into a selected `TaskRuntime`
   provider today.
2. **Park/resume realization.** Canonical crossings carry live frontiers
   and preservation demands, but native park/resume — saving the frame at
   a checked crossing and re-entering the same invocation — needs its
   backend realization. The bounded scalar suspension carrier does not
   license receiver/structural/claim frontiers without their exact joins
   (see the Terminal producer's structural-results note).
3. **Safe-point observation.** `observe_cancellation` records where a
   request was observed; an executing runtime must actually reach those
   crossings — a task that never suspends stays finishable but not
   cancellable, per the contract.
4. **WCSU completeness.** `stack_composition` takes the max aligned live
   chain and fails closed on unresolved calls, but the final-physical-frame
   WCSU producer theorem — final demand plus actual reservation feeding
   `establish_stack_lease` — is still an open join.
5. **Executable custody route.** A selected runtime also needs the
   executable-installation/UEFI execution legs (the freestanding image,
   provider realization on target, and the `omega run` leg) before an
   activation exists physically; those belong to their own rows
   (UEFI-PHYSICAL-SEMANTIC-ENTRY, UEFI-OS-HANDOFF, EXECUTABLE-* family).

## Design constraints to preserve

- `Task<T>` stays linear: no copy/overwrite/drop discharge, no implicit
  fire-and-forget, no second task type for borrowed storage.
- Provider provenance is permission state, never a nominal result
  parameter; a task cannot outlive its storage.
- Cancellation is cooperative at canonical safe points only — no semantic
  preemption, no optimizer-inserted polls in non-suspending kernels.
- An ordinary hosted blocking executor is a package
  (`source/library/blocking-executor`), not a task-runtime mode; the
  shared `WaitSubstrate` boundary (word wait + wake-one/wake-many) is the
  approved substrate, not a merged host mechanism.

## Validation scope for this record

Documentation-only draft; no code changed. Read against
`origin/main` tip at authoring: `core/task.omg` boundary trait and claim
vocabulary, `wiki/spec/build/task_runtime.md`, and the `task-plans`
README/lib surface.
