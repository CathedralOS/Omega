# Chapter 24: Embedding And Scripting

An application can use Omega as a scripting language by depending on its
interpreter library. Scripts are ordinary Omega programs compiled to Psi. They
use the same contracts, ownership, and boundary interfaces as native programs.
This chapter describes the accepted design, not a completed public SDK. The
[embedding specification](../spec/build/embedding.md) defines its contract and
[execution board](../../TASKS.md#embedding-and-interpreted-components) tracks delivery.

## One interface, two implementations

Suppose a game exposes a boundary requirement:

```omega
pub boundary trait Game {
    machine set_health(player: u64, health: u32)
        reaches Game;
}

pub trait ScriptCalls {
    machine reward(player: u64)
        reaches Game;
}

data RewardScript {
    game: Binding<Game>;
}

machine RewardScript::reward(&mut self, player: u64)
satisfies ScriptCalls::reward
reaches Game
{
    self.game.set_health(player, 100);
}
```

The host and script depend on the same interface package. The build selects an
exported entry for the script and establishes its receiver from supplied bindings;
the example does not implicitly export `reward`. There is no special script
`main` name. Missing or incompatible bindings reject before guest execution.

`Binding<T>` names an established implementation of a boundary requirement. It
is a core type name, not a keyword or network-service facility. Native execution
may use a direct selected provider; interpretation uses a checked adapter to the
host implementation. Both must meet the same contract. Wrapping a native function
does not prove it trustworthy.

The bridge is ordinary dispatch:

```text
guest call -> installed typed adapter -> native host call
           <- validated result       <- host result
```

The reverse direction invokes a selected guest export and checks its complete
entry obligations. Generated bindings remove repetitive marshalling; they cannot
invent domain evidence or resource authority. Two fields with type `Binding<Game>`
may refer to different game worlds, so bindings identify slots, not just types.

## Loading and running

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

Inline assembly remains supported language surface. How target-specific assembly
runs under interpretation is an [open owner question](../../OWNER_QUESTIONS.md)
named `interpreted-inline-assembly`; this chapter does not require removing it
from OS code or promise an instruction emulator. See the
[assembly chapter](chapter_22_inline_assembly.md).
