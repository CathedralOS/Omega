# Chapter 24: Embedding and scripting

An application can use Omega as a scripting language by depending on its
interpreter library. Scripts are ordinary Omega programs compiled to Psi. They
use the same contracts, ownership, and boundary interfaces as native programs.
The [embedding specification](../spec/build/embedding.md) defines loading, calls, and instance lifetimes.

## One interface, two implementations

Suppose a game exposes a boundary-trait requirement:

```omega
public boundary trait Game {
    machine set_health(player: u64, health: u32)
        reaches Game
        crashes none;
}

public trait ScriptCalls<Context> {
    machine reward(context: &mut Context, player: u64)
        reaches Game;
}

data RewardScript { }

machine reward(context: &mut RewardScript, player: u64)
satisfies ScriptCalls<RewardScript>::reward
reaches Game
{
    Game::set_health(player, 100);
}
```

The host and script depend on the same interface package. The build selects
`reward` as an export and binds `Game` to the host implementation. The installed
export contract supplies the `RewardScript` context from instance storage.

Native and interpreted calls use the same requirement. Interpretation reaches
the host through a typed adapter:

```text
guest call -> installed typed adapter -> native host call
           <- validated result       <- host result
```

Calling a script export in the other direction checks its entry contract and
arguments too. Generated bindings handle marshalling; host operations remain
responsible for the guarantees they promise.

## Loading and running

The target environment and execution engine are separate choices. Windows,
macOS, Linux and Cathedral describe environments; interpreted and native describe
how code executes. Most libraries are ordinary Omega code and work in both
engines without another implementation. A platform library supplies target
implementations; external mechanisms need host adapters for interpretation.

The command-line choice is explicit:

```text
omega run --engine native main.omg
omega run --engine psi main.omg
omega run --both main.omg
```

Native is the default. `--engine psi` interprets the program; `--both` runs and
compares both routes. The target environment is selected separately, so an
interpreted Linux operation still needs a provider for its Linux contract.
Missing adapters prevent execution.

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
or async executor is silently created. Work limits interpreter work, not the
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
rely on stronger facts about the current value. A work stop can also occur inside
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

## Native and interpreted cathedral

An interpreted OS executes its own boot, driver, scheduler, and update code.
The host supplies the processor and device environment through selected
bindings. This can exercise OS behavior without granting control of the real
host's hardware.

Assembly runs through supported instruction semantics. Memory, device, and
processor-state effects persist; scratch registers stay local to their assembly
region. Missing environment support prevents execution. Native timing and real
hardware behavior require their own tests. See [interpreted assembly](chapter_22_inline_assembly.md#running-assembly-under-interpretation).
