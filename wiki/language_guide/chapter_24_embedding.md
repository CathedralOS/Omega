# Chapter 24: Embedding and scripting

An application can run Omega as its scripting language by linking the
interpreter library. A script is an ordinary Omega program compiled to Psi,
Omega's portable format. It has the same contracts and ownership rules as
native code, and it reaches the host only through boundary traits.

## One interface, two sides

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

Host and script depend on the same interface package. The host implements
`Game`; the script implements `ScriptCalls`. The script can do to the host
exactly what `Game` offers and nothing else.

```text
script call -> typed adapter -> native host call
            <- checked result <- host result
```

## Running a program either way

```text
omega run --engine native main.omg     (the default)
omega run --engine psi main.omg        (interpreted)
omega run --both main.omg              (run both and compare)
```

Ordinary Omega code runs under both engines unchanged.

## Loading and calling from a host

In outline:

```text
program  = load(psi_bytes, receiving_policy)
instance = runtime.instantiate(program, supplied_bindings)
call     = runtime.start(instance, selected_entry, arguments)
outcome  = call.resume(work_budget)
```

`resume` runs the script for a bounded amount of work and reports whether it
returned, paused on its budget, is waiting on the host, or faulted. Nothing
runs in the background: the script advances only when the host resumes it.
Loading a program does not run its build files or give it file access.

## Editors and live state

An editor can use [reflection](chapter_21_reflection.md) to list a script's
public operations and call them. It cannot write into a paused script's
variables: the paused code may be relying on facts about them. Edit a detached
copy and submit it through an operation that validates it.

## Replacing code while running

A component with a declared boundary can be replaced in a running program:

```text
compile replacement -> submit to the updater -> check
                    -> handle state -> publish new entries
                    -> retire old code once nothing still uses it
```

New calls go to the new code. Calls already in progress finish in the code
they started in. See
[component publication](../spec/build/component_publication.md).

## Where next

The complete rules are in [embedding](../spec/build/embedding.md).
[Using source packages](packages.md) covers dependencies.
