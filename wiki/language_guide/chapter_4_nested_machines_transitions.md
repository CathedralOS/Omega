# Chapter 4: Nested Machines And Transitions

A nested machine is an ordinary machine whose name is visible only inside its
enclosing machine's scope. Nesting organizes code; it does not capture data or
require another stack frame.

```omega
machine countdown(remaining: u32) -> u32 {
    transition {
        _ -> step(remaining)
    }

    machine step(remaining: u32) -> u32 {
        transition remaining {
            0 -> done()
            _ -> step(remaining - 1)
        }

        machine done() -> u32 {
            0
        }
    }
}
```

The nonzero arm proves subtraction is valid. `done` is visible inside `step`,
but not directly inside `countdown`. `step` is visible throughout
`countdown`, including its earlier executable portion and inside `step`
itself. Nesting has no fixed depth limit.

The example shows control transfer, not a published termination promise.
A cyclic machine promising termination supplies the ranking evidence described
in [Chapter 9](chapter_9_proof_obligations.md). Runtime recursive cycles use
constant-stack iteration; nesting depth does not decide stack consumption.

## Calls Resume; Terminal Calls Complete

```omega
machine answer() -> i32 {
    let value: i32 = base();
    value + 1

    machine base() -> i32
    ensures result == 41;
    {
        41
    }
}
```

The ordinary call to `base` returns to `answer`, which continues and produces
42. A terminal call instead forwards the result:

```omega
machine answer() -> i32 {
    transition {
        _ -> base()
    }

    machine base() -> i32 {
        42
    }
}
```

Here `answer` has no continuation after the call. In both examples `base`
returns from its own invocation. It never returns directly from an arbitrary
lexical ancestor. Every normal completion owes that invocation's result and
postconditions; terminal forwarding must also satisfy the caller's contract.

The executable portion comes first and declarations follow. A transition,
a result expression, or ordinary Unit completion can end that portion. There
is no requirement to manufacture a transition just to declare a helper.
Executable statements cannot appear after the first nested declaration.

## Explicit Data, Lexical Names

```omega
machine forward(value: Inventory) -> Inventory {
    transition {
        _ -> done(move value)
    }

    machine done(value: Inventory) -> Inventory {
        value
    }
}
```

There is no closure. Enclosing runtime parameters and locals are unavailable
inside `done` unless passed as its explicit arguments. A mutable context is
likewise an ordinary parameter: `machine advance(context: &mut Context, index:
u64)` is called with `advance(&mut context, next_index)`.

Enclosing generic/static declaration bindings remain lexical context, not a
hidden runtime argument or captured activation. Shared helpers needed outside
a nesting scope belong at the appropriate common declaration scope.

A sibling can call another visible sibling, but cannot enter its private
descendants. Calling a visible ancestor invokes its entry; it does not resume
an enclosing activation. Qualification cannot bypass lexical visibility.

Explicit arguments need not copy bytes. Storage planning may reuse homes while
preserving exact values, loans and custody. Owned values still transfer once;
dying values need their legal disposition. A final call cannot discard backing
that its arguments still borrow.

## Contracts Are Not Captured Either

Every call proves the callee's requirements using its exact actual arguments.
Live facts established by an enclosing entry may help prove them, but nesting
does not restore those facts after a write invalidates them. A helper's body
checks against its own contract, not whatever happened to be true at one caller.

The [invocation specification](../spec/language/state_contracts.md) owns
arrival substitution, mutation, return obligations and recursive proof use.
Ordinary invariant and borrowed-storage restoration rules still apply at calls,
terminal transfers and completion. Nesting introduces no scope for hiding
unfinished restoration.

## Data Patterns And Coverage

A transition can destructure its saved subject:

```omega
transition header {
    Header { ok: 0, version } -> accept(version)
    Header { ok as _, version as _ } -> reject()
}
```

Here `accept` and `reject` are visible machines with compatible result
contracts. `version` binds the field; `ok: 0` contributes projected equality;
`field as name` renames; `field as _` waives. Without `..`, every field must
be mentioned. Adding a field then requires revisiting the pattern.

The subject evaluates once; extraction uses that value.
[Pattern matching](chapter_6_pattern_matching_dispatch.md) explains ordered
selection and coverage. An uncovered runtime possibility rejects rather than
silently trapping. Complete finite case/Boolean coverage or a wildcard establishes
coverage; arbitrary guard ladders need a fallback. An explicit `_ -> {}`
completes with Unit and cannot satisfy a value-returning contract.

## Final Calls, Including Suspension

Assume `Storage::read(&mut storage)` returns `u32`, permits suspension but not
blocking, and the reference satisfies its loan/carry requirements:

```omega
machine finish_read(storage: &mut Storage) -> u32
suspends;
{
    transition {
        _ -> suspend Storage::read(&mut storage)
    }
}
```

This invokes the call and completes with its result. If it parks,
`finish_read` has not returned; no task is launched or detached. Completion
still owes the result contract and cleanup.

Nested helpers obey the same exact acknowledgement rules: if their contract
permits suspension, their callers acknowledge it too. A future jump optimization
does not remove source acknowledgements. Use `block` or `suspend block` when
the selected call envelope requires them. See
[terminal arm calls](../spec/language/state_contracts.md#terminal-calls-in-transition-arms)
and [borrows across suspension](chapter_18_concurrency.md#borrows-across-suspension).

## From Declarations To Execution

Resolution checks lexical visibility, then gives each machine a flat identity
and explicit arguments. Downstream code need not carry a runtime nesting tree.
Basic blocks, calls and terminal transfers describe execution; source ancestry
remains useful for diagnostics and debug correspondence.

A machine is not a promise to allocate a frame. The compiler may inline, fuse,
reuse frames or emit jumps while preserving contracts, resource bounds and live
storage. Constant-stack runtime cycles are required behavior, not an optional
optimization. Non-tail runtime recursion does not become legal through nesting.

The source `state` keyword is retired. These examples describe the ratified
language ahead of implementation. Existing sample paths such as `other_state/`
are migration coordinates, not a second declaration kind; the
`NESTED-MACHINE-UNIFICATION` task tracks compiler, library and sample migration.
