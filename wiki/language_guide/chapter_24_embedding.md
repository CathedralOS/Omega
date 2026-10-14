# Chapter 24: Embedding And Scripting

An application can use Omega as a scripting language by depending on its
interpreter library. Scripts are ordinary Omega programs compiled to Psi. They
use the same contracts, ownership, and boundary interfaces as native programs.
This chapter describes the accepted design, not a completed public SDK. The
[embedding specification](../spec/build/embedding.md) defines its contract and
[execution board](../../TASKS.md#embedding-and-interpreted-components) tracks delivery.

## One interface, two implementations

Suppose a game exposes a boundary-trait requirement:

```omega
pub boundary trait Game {
    machine set_health(player: u64, health: u32)
        reaches Game;
}

pub trait ScriptCalls<Context> {
    machine reward(context: &mut Context, player: u64)
        reaches Game;
}

data RewardScript { }

machine RewardScript::reward(context: &mut RewardScript, player: u64)
satisfies ScriptCalls<RewardScript>::reward
reaches Game
{
    Game::set_health(player, 100);
}
```

The host and script depend on the same interface package. The build selects an
exported entry for the script and supplies every demanded static import slot;
the example does not implicitly export `reward`. There is no special script
`main` name. Missing, ambiguous, or incompatible supply rejects before guest execution.
`ScriptCalls` declares the context parameter rather than relying on an implicit
receiver. The installed entry adapter supplies that parameter from the admitted
instance; an ordinary in-language call supplies it explicitly.

The exact boundary-trait application names the import slot. Native execution may
use a direct selected provider; interpretation uses a checked adapter to the host
implementation. Both must meet the same contract. Wrapping a native function
does not prove it trustworthy.

The bridge is ordinary dispatch:

```text
guest call -> installed typed adapter -> native host call
           <- validated result       <- host result
```

The reverse direction invokes a selected guest export and checks its complete
entry obligations. Generated bindings remove repetitive marshalling; they cannot
invent domain evidence or resource authority. Two static game roles use distinct
applications such as `Game<PrimaryWorld>` and `Game<PreviewWorld>`; arbitrary
runtime worlds use an ordinary world/session handle declared by the protocol.

## Loading and running

The target environment and execution engine are separate choices. Windows,
macOS, Linux and Cathedral describe environments; interpreted and native describe
how code executes. Most libraries are ordinary Omega code and work in both
engines without another implementation. A platform library supplies target
implementations; external mechanisms need host adapters for interpretation.

```text
Filesystem::read(...)       -> selected portable filesystem slot
LinuxRawSyscalls::read(...) -> selected Linux contract, not a similar host API
```

These names illustrate library contracts, not compiler-blessed services. Psi
executes the program and dispatches its boundary calls. Target-aware host
adapters live outside Psi and reuse Omega's ABI/mechanism libraries; the host
installs them. An ordinary Omega provider body can itself run interpreted.
An adapter is not required for every library or machine.

Missing provider slots in the selected executable closure prevent execution and
name the missing requirement. There is no silent service substitution, native fallback
or extra authority. A likely-unused branch does not excuse a missing binding.
The [test runner](chapter_23_testing.md#enablement-and-results) can schedule
both engines explicitly and reports unavailable combinations separately.

The host owns a runtime, its host context, and script instances. An immutable
admitted program can be shared by instances, while each instance owns its state.
Loading bytes does not run build files or grant filesystem access.

The library flow, shown as pseudocode rather than fixed API spelling, is:

```text
program  = load(psi_bytes, receiving_policy)
instance = runtime.instantiate(program, supplied_bindings)
call     = runtime.start(instance, selected_entry, arguments)
outcome  = call.resume(work_budget)
```

`outcome` distinguishes return, budget pause, pending host operation, and fault.
A synchronous call is a convenience over the same mechanism. No background thread
or async executor is silently created. Fuel limits interpreter work, not the
duration of a synchronous native host call.

A paused call retains its locals, loans, and unfinished obligations. Returning
control to the host does not authorize editing borrowed state. Dropping the call
handle does not cancel it. Closing an instance can report Busy while preserving
its resources; successful closure reclaims that instance without closing all the
others. Host effects already performed are not rolled back by a later failure.

## Calls, messages, and editors

The host can call `update` with input data and receive commands. A script can
call a supplied host query directly, submit a bounded queued command, or receive
an event through another exported entry. Events are ordinary calls and data, not
a separate language mechanism. Queue acceptance is not action completion.

Reflection describes the retained public API and selected type schemas. An editor
can discover available operations and invoke them; it cannot overwrite arbitrary
live fields or paused locals. Read and change live objects through exposed checked
operations, with ordinary access constraints. A detached document may be freely
edited and submitted to an operation that validates and applies it.

This matters even when an edit would preserve a type invariant: paused code can
rely on stronger facts about the current value. A fuel stop can also occur inside
an invariant window. Neither the schema nor a checked setter grants permission
to interfere with that activation. Diagnostic snapshots are observations, not
established values that can be written back to the interpreter.

## Replacing part of a running program

A source module is not automatically a replaceable component. Build composition
must establish an independent boundary. Then interpreted and native components
follow the same [publication and lifetime rules](../spec/build/component_publication.md):

```text
compile replacement -> submit to authorized updater -> stage/check
                    -> handle state -> publish new entries
                    -> retire old code after its obligations settle
```

New calls use the published implementation. Existing activations, callbacks, and
returned objects retain the implementation they still need. State can remain
outside the component, be reset, or undergo an explicit migration. A paused
continuation does not automatically resume inside edited code.

An interpreted program can own the updater itself, including a long-running OS.
The host supplies component-execution mechanics, not the OS's update policy.
Alternatively, an explicitly appointed host supervisor may own updates. Loading
an artifact alone never authorizes replacing the running program.

Artifact bytes may come from disk, a virtual filesystem, a network, or memory.
There is no special scripting import resolver. Optional source compilation uses
the ordinary root and nested build files with their lock and attenuated authority;
compiling one independent component need not rebuild every product. Fused callers
or shared concrete layouts can require a larger rebuild.

## Native and interpreted Cathedral

The goal is actual Cathedral startup, event handling, and self-managed component
replacement under an interpreter, using different selected bindings rather than
execution-mode conditionals throughout its shared code. A host routine that
implements Cathedral's boot or update logic would not demonstrate this goal.
Native timing and real-device correctness remain separate from a host simulation.

Inline assembly runs by interpreting supported instruction semantics against
ordinary values/storage and a supplied target environment. Scratch registers
are block-local; memory, device and contracted processor-state effects persist.
Missing instruction/environment coverage rejects before execution, not as a
no-op or native fallback. This preserves assembly-bearing OS code without
requiring an alternate body per block or promising whole-host emulation. See the
[assembly chapter](chapter_22_inline_assembly.md#running-assembly-under-interpretation).
