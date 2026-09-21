# Embedding Omega: bindings, execution, and lifetime

Design lab, not ratified surface or a shipped SDK. The owner requested a concrete
embedding design and reconsideration of the name `Service<T>`. No source keyword,
core rename, provider rule, or implementation feature is adopted by this draft.
API spellings below are sketches. The experiment section distinguishes executed
Rust probes from proposed Omega/library examples.

## Recommendation

Make embedding an ordinary library consuming Psi. Keep source compilation optional.
Use normal boundary requirements for imports and normal selected entry contracts
for exports. Give the application explicit ownership of a runtime, its instances,
provider state, and unfinished invocations. Do not create a process-global VM,
automatic worker thread, dynamic library loader, or scripting-only trait system.

Prefer `Binding<T>` to `Service<T>`: a value binds this exact interface to an
authorized implementation. This applies equally to a filesystem, game API, test
double, firmware interface, and interpreted component; it implies neither a
network service nor asynchronous execution.

`Service` is currently a compiler-known core type name, not a lexer keyword:
[its declaration](../../source/library/core/service.omg) is ordinary `boundary data`;
[its establishment semantics](../spec/build/component_publication.md#service-bindings-and-era-entry)
are compiler-owned. Renaming the type would not add a keyword or change those
semantics. `Trusted<T>` incorrectly suggests that wrapping an implementation proves
it trustworthy. `Boundary<T>` describes a crossing but not a bound implementation.
`Bound<T>` is a reasonable shorter alternative; `Binding<T>` is more direct.

There is already a target/toolchain `Binding<...>` foreign-locator type in
[foreign bindings](../spec/build/foreign_bindings.md). A rename must resolve that
naming collision deliberately (qualification, or `ForeignBinding` for the locator),
not silently merge the two meanings. This lab uses the proposed `Binding<T>` only
inside explicitly marked design examples; the current spec still says `Service<T>`.

## The application and packages

```text
game_api/       build.omg + game.omg: the shared API contract
game/           build.omg + application sources + native game implementation
scripts/        build.omg + reward.omg: imports game_api, emits reward.psi
```

The native application depends on the interpreter library and `game_api`. The
script depends on the same exact API package. The application may additionally
depend on the source compiler if users supply `.omg` text instead of `.psi` bytes.
These are ordinary package dependencies, not a new plugin manifest.

In an Omega host, an intended library import is `use omega::interpreter;` with
ordinary package resolution; its exact public package path is not settled here.
Rust and C embeddings expose the same lifecycle through generated host bindings.
The CLI is a client, not the embedding API. The reference interpreter crate is
not by itself a promise of a stable external SDK.

Loading Psi neither executes `build.omg` nor fetches dependencies. Compiling
untrusted source with a build file is a separate admitted compilation request,
with ordinary lock agreement and attenuated build authority. A script's build
cannot confer its filesystem grants on its eventual runtime instance.

## One script, both directions

Proposed source example (`Binding` is the candidate rename):

```omega
// game_api/game.omg
pub boundary trait Game {
    machine set_health(player: u64, health: u32)
        reaches Game;
}

pub trait ScriptCalls {
    machine reward(player: u64)
        reaches Game;
}

// scripts/reward.omg; ordinary imports omitted
data RewardScript {
    game: Binding<Game>;
}

machine RewardScript::reward(&mut self, player: u64)
satisfies ScriptCalls::reward
{
    self.game.set_health(player, 100);
}
```

The script build selects the public `ScriptCalls::reward` entry and its attached
receiver under the ordinary entry-root contract. `satisfies` alone does not expose
private code. Its import description names the exact `game` field/slot and `Game`
schema; its export description binds the selected machine to the exact entry
requirement. The host supplies the field at authorized instance establishment.
There is no implicit script `main` or privileged `run` method name.

Compile-time binding generation produces typed host import/export descriptors
from these checked descriptions. The generator can be untrusted: admission still
rejoins exact package/declaration identities, complete contracts, representations,
and selected adapters. Names are diagnostics, not dispatch authority.

For a Rust host, the generated bridge can look like this:

```rust
impl GameProvider for GameHost {
    fn set_health(&mut self, player: u64, health: u32) {
        self.world.set_health(player, health);
    }
}
```

For an Omega host, the implementation remains ordinary Omega:

```omega
data GameHost {
    world: World;
}

machine GameHost::set_health(&mut self, player: u64, health: u32)
satisfies Game::set_health
{
    self.world.set_health(player, health);
}
```

Select that exact satisfier for this binding; there is no second
authoring interface for a script provider. The adapter translates between the
guest's checked values and that selected host machine's calling convention.
Generating the adapter is real work, not something this example proves existing
reflection already implements.

The host entry is also checked/validated in the other direction. An arbitrary host
cannot call a guest export with unchecked preconditions or fabricate qualified
inputs merely because its scalar types line up. Static host proofs, admissible
checked evidence, or a declared checked entry adapter must establish the contract.
General propositions are not automatically runtime-checkable predicates.

## Library API and ownership

Use three ownership roles, not a collection of forwarding facades:

| Role | Owns |
| --- | --- |
| `Program` | Immutable admitted Psi, public schemas, and reconstructed execution metadata; shareable across instances. |
| `Runtime<Host>` | Host context/provider state, installed bindings, guest instances and their storage, resource custody, and unfinished calls. |
| `Invocation` | Exclusive access to one active call and its continuation; a scoped control handle into the runtime. |

An instance identifier is runtime-scoped, not another global owner. Separate
instances have separate guest state. Sharing host resources is explicit through
their supplied bindings. Closed-instance identifiers cannot become valid for a
new instance reusing storage. An export handle likewise names its instance and
exact program entry, not merely an ordinal or a function address.

The ordinary synchronous path, shown as host-library pseudocode:

```rust
let program = Program::load(psi_bytes, admission_policy)?;
let mut runtime = Runtime::new(GameHost::new(world), storage_limits)?;

let game = runtime.bind::<Game, GameHost>()?;
let script = runtime.instantiate(&program, RewardImports { game })?;

// Convenience over start/resume; this example handles a normal return.
let outcome = runtime.call::<ScriptCallsReward>(script, (42,), work_budget)?;
match outcome {
    Returned(()) => { /* this invocation has settled its obligations */ }
    Paused(call) => { /* retain call; replenish/resume or validly cancel it */ }
    Waiting(call, request) => { /* retain call and service the exact request */ }
    Faulted(failure) => { /* contain; effects already performed remain */ }
}

// Allowed only when no active call/resource loan excludes this access.
runtime.host_mut()?.world.advance_physics();
runtime.close(script)?;
let host = runtime.shutdown()?;
```

The code illustrates the result cases, not a runnable implementation of the
handlers: ignoring a Paused/Waiting/Faulted arm does not permit the following
access/close. Those operations reject and preserve custody until settled.

`Runtime::bind` is a proposed embedding operation, not the existing argument-free
`Build::select_provider` under a new name. It admits an exact adapter and establishes
a runtime-owned binding against actual host state. An instance consumes its supplied
import bindings. A second instance needs separately authorized supply, not copying
an affine carrier. If an interface can be supplied twice, the runtime may establish
two bindings to the same provider under its ordinary sharing/loan rules.

Mapping by interface type alone is insufficient: a guest can have two `Game` fields
bound to different worlds. Generated `RewardImports { game }` represents exact
import slots; a generic low-level API must retain that distinction too. Instantiation
checks the complete demanded roster before running guest code. Missing, duplicate,
wrong-runtime, wrong-schema, or unauthorized slots reject without fallback.

The host context need not be the entire application. It can own the world, or only
the application's intentionally exposed command/resource interface. A scoped host
context may borrow application data; then the runtime cannot outlive that borrow.
That is an explicit scoped convenience, not the default requirement that every
long-lived script permanently borrow `&mut world`. An owning runtime gives the
application mediated access to its host state between completed invocations.

Many guest instances may share one runtime. Separate runtimes provide independent
execution/storage accounting, but not OS isolation from their native providers.
`Program::load` does not require an always-live engine singleton. Instances retain
their immutable program owner, so dropping the caller's `Program` handle is safe.

## Interpreting, pausing, and resuming

The underlying operation should be explicit and resumable:

```text
call = runtime.start(entry, arguments)
call.resume(work_budget)
    -> Returned(value)
     | Paused(budget_exhaustion)
     | Waiting(exact_pending_host_operation)
     | Faulted(diagnostic_and_custody)
```

`call`/`resume` execute on the caller's thread unless the application deliberately
uses a compatible executor. No implicit async runtime, background worker, DLL, or
global registry. A synchronous wrapper is just the same operation driven to its
next outcome, not a second execution engine.

- Fuel pause occurs at an interpreter-safe point. It is not source `suspends`,
  cancellation, a return, or permission to release live loans. Replenishment
  continues after already completed effects, rather than repeating them.
- Semantic waiting requires the operation's declared operational contract. The
  pending request owns its arguments, completion identity and custody. Resume
  accepts completion of that exact request once; it does not call the host again.
- A native synchronous host call may block inside its declared contract. Interpreter
  fuel cannot preempt it or bound its wall time. Finite deadlines need an actual
  cancellable/isolated host mechanism, not a fuel setting.
- The selected bindings remain fixed throughout an invocation. Passing a new
  handler object on each resume is not a public rebinding operation. Replacement,
  if desired later, follows the existing installation/replacement contract.
- Host state or guest memory cannot be lent to another operation while a retained
  loan excludes it. A conservative first wrapper keeps the runtime exclusively
  borrowed for an unfinished call; finer independent scheduling must preserve the
  actual loan/claim structure, not hide it with a mutex or `RefCell`.
- Do not expose an unrestricted `&mut Runtime` in a host callback. Callback context
  supplies only its admitted arguments/resources. Guest re-entry needs the existing
  declared callback contract and nonconflicting custody; otherwise reject it.

An `Invocation` handle going out of scope cannot discard the runtime's unfinished
activation. The runtime remains busy until the call completes or obtains a valid
disposition. A Rust wrapper cannot enforce Omega's linear obligations using an
ordinary destructor alone; its containment/custodian contract must account for
abandoned handles and foreign-host misuse explicitly.

## Values, shutdown, and failures

| Crossing | Rule |
| --- | --- |
| Scalars | Exact typed values, not unchecked casts or host-width integers. |
| Records, sums, bytes | Schema-directed conversion; ordinary owned transfer or a scoped checked view, not a cast of VM memory to a Rust struct. |
| Borrowed views | The backing stays live and respects access mode for the entire loan, including any permitted suspension. |
| Host objects | Established typed resources with exact owner/generation and lifecycle, not a script-forgeable integer pretending to be authority. |
| Guest objects retained by host | An owning/pinning handle prevents instance reclamation until released or transferred. |

Avoid mandatory serialization for every call. Copy scalar values; reuse validated
views or transfer owned storage when their representations and lifetimes permit.
Safe direct borrowing is an explicit adapter capability, not an assumed universal
zero-copy property.

`close(instance)` closes new entry and checks active calls, exported handles,
callbacks, child work, resource claims and required cleanup. Success reclaims that
instance's storage. Busy/failing close retains ownership; it does not free and then
report a failure. A forever-live runtime must be able to reclaim closed instances,
not retain every historical script until the entire application exits.

`shutdown()` similarly returns host ownership only after its dependent obligations
are settled. Cleanup may itself need host calls and a work budget. It is not an
implicit language-level unwind. Cancellation is offered only with the actual
cancellation/disposition contract; plain fuel exhaustion supplies none.

For the simple scalar-only game API, there are no retained host loans or linear
resource obligations: discarding a contained faulted guest can reclaim its private
memory, but cannot undo a health update. A provider exporting a lock, transaction,
DMA loan, or registered callback has a stronger contract. If the runtime cannot
safely contain abandonment, it must reject that embedding mode before granting the
resource, or require a wider custodian/containment scope. Leaking memory is not a
general solution to a lock or protocol debt.

Provider result mismatch or an internal error after possible host effects faults
the invocation; the public API must not automatically retry it. A provider cannot
claim `ensures` by returning well-shaped bytes: arbitrary native code is a named
trusted premise unless independently checked. Checked Omega host providers can
supply checked evidence. Physical native isolation remains a separate guarantee.

## Authority and evidence

The embedding application is the receiving authority. Its instance supplies only
the bindings it admits: no default filesystem, process, network, or dynamic loader
merely because the application itself can use them. Descendants can attenuate,
not replace virtual backing with host backing. Host adapter code must implement
that scope; the language cannot constrain malicious native code by type spelling.

The runtime always validates artifact structure, schemas, entry/input validity,
authority, and supported execution. Optional PCC is separate: a receiving policy
may require the adjacent `.proof` and independently check it. An accepted
producer-trust route without PCC must not be reported as independent verification.
Loading is bounded too: decoding, evidence checking and instance allocation are
not covered merely by the later instruction budget.

Test groups can use this same embedding lifecycle with fresh instances and selected
mock providers. Tests add discovery, scheduling and verdict policy above it. They
do not justify a second interpreter, special source annotation, or native fallback.
The semantic constant evaluator remains a different admission regime; general
embedded runtime execution may legitimately reach host APIs.

## What the lab actually executed

The reference interpreter exposes a low-level
[effect handler](../../omega-rust/psi/semantics/terminal-interpreter/src/terminal_interpreter/effects.rs),
an owning `TerminalExecution`, fuel and typed outcomes. New
[embedding probes](../../omega-rust/psi/semantics/terminal-interpreter/tests/unit/embedding_lifetimes.rs)
construct, encode, independently verify and interpret a two-call scalar Psi
artifact with a native Rust host handler. They deliberately have no source frontend,
Service establishment, borrowed host resources, or full embedding facade.

Windows command (also valid with `cargo`/`mbx` on macOS):

```text
cargo nextest run -p terminal-interpreter --test unit -E 'test(embedding_lifetimes::)' --no-tests fail --no-fail-fast
```

`mbx` was unavailable on this host; Cargo was used under the repository fallback.
Seven probes passed on Windows at the `a9286683d0` base plus the lab changes:

1. Execution survives destruction of the input code/evidence buffers.
2. One-unit fuel replenishment resumes without repeating completed host effects;
   the scalar-only handler borrow ends after each resume.
3. Two executions can use independent host state.
4. The raw effect hook permits switching host sinks mid-execution. This proves it
   is **not** an admitted stable provider binding; the public runtime must own that
   association. This test describes the oracle API, not a new source permission.
5. Rejection after a host mutation preserves that mutation; failed bookkeeping is
   not rollback and must not trigger an automatic retry.
6. A host result with the wrong schema is not accepted as successful completion.
7. A native host computes a scalar result; the guest receives it and returns it
   through the interpreter to the embedding application.

These are not a source-to-SDK demonstration or a macOS runtime pass. Current startup
decodes/verifies on every `start_artifact`; the immutable shared `Program` admission
entrance is still work. Current outcomes expose completion, sponsor exhaustion and
crash, not a complete pending-native-operation protocol. There is no generated
Game adapter or ready public embedding package in this experiment.

## Remaining implementation, in customer order

1. One source-authored API, native host implementation, script and selected export
   through compile -> load -> bind -> call -> typed return -> close. Missing/wrong
   bindings reject before the first host effect. Includes a host-call result, not
   only a print sink. No universal resource bridge prerequisite.
2. Share admitted immutable program state; keep two persistent instances with
   independent guest state and independently supplied providers. Include two slots
   of the same trait bound differently, and wrong-runtime/stale-handle rejection.
3. Pin bindings across budget pause, preserve once-only effects, report faults
   without retry, and reclaim an instance after clean close while another runs.
4. Exercise a borrowed byte view and one owned host resource end to end: retained
   result prevents close, ordinary release permits it, fault/abandonment has an
   explicitly supported disposition. Unsupported adapters fail before exposure.
5. Add pending host completion and an explicit callback customer under the existing
   suspension/callback contracts; test duplicate/late completion and live custody.

The long-term boundary includes all five; do not advertise a full embedding SDK
after only step one. Nothing here requires a JIT, hot reload, a DLL ABI, or a plugin
deployment manager. A C ABI is an adapter over the same ownership model when there
is a C host customer, not a separate runtime design.

## Comparison and decisions still needing owner approval

Wasmtime separates immutable modules, host functions and store-owned host state;
its [hello-world embedding](https://docs.wasmtime.dev/examples-hello-world.html)
is a useful working reference. Its [Store contract](https://docs.wasmtime.dev/api/wasmtime/struct.Store.html)
also documents that instances live until store destruction. Borrow the explicit
ownership principle, not that reclamation restriction: this customer includes
long-lived applications repeatedly loading and retiring scripts. Its
[Linker](https://docs.wasmtime.dev/api/wasmtime/struct.Linker.html#method.func_wrap)
also separates reusable host function definitions from per-store state.

Recommended owner decisions are the core name (`Binding<T>`), the ordinary typed
embedding layer described here, and explicit lifetime/custody-aware shutdown.
Exact method/package spellings are library design, not new language keywords.
The next proof of ergonomics is the source-to-host customer in step one, not an
unbounded table of hypothetical FFI features. This draft intentionally does not
ratify a rename, alter specs, or create implementation commitments on TASKS.md.
