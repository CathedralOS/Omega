# Chapter 4: Nested machines and transitions

A machine can declare helper machines inside itself, and a `transition` picks
which one runs next. Together they are how Omega writes loops and state
machines.

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

`step` calls itself until `remaining` is zero. In the `_` arm the compiler
knows `remaining` is not zero, so the subtraction is safe.

## What nesting does

Nesting only limits where a name is visible. `step` can be used anywhere in
`countdown`; `done` only inside `step`. A machine's statements come first and
its nested machines after.

A nested machine is not a closure. It sees none of the enclosing machine's
parameters or locals, so pass what it needs:

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

Enclosing generic parameters are the exception: they are compile-time context
and stay in scope.

## A call returns; a transition arm finishes

An ordinary call comes back and the caller continues:

```omega
machine answer() -> i32 {
    let value: i32 = base();
    value + 1

    machine base() -> i32
    ensures result == 41
    {
        41
    }
}
```

A transition arm is the last thing a machine does. The arm's value, or the
result of the machine it calls, becomes the result of the whole machine:

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

Because nothing is left to do after the arm, a machine that repeats through
transition arms uses constant stack however many times it goes round.

## Choosing an arm

A transition with a subject tests its arms in order and runs the first that
matches:

```omega
transition header {
    Header { ok: 0, version } -> accept(version)
    Header { ok as _, version as _ } -> reject()
}
```

`ok: 0` tests the field, `version` binds it, and `field as _` ignores one.
Every field is mentioned unless the pattern ends in `..`, so adding a field
later makes you revisit the pattern. The arms must cover every possible value;
there is no silent fall-through. [Chapter 6](chapter_6_pattern_matching_dispatch.md)
covers patterns in full.

## Facts go with the arguments

Each call must establish the callee's `requires` from the arguments it
passes. A nested machine is checked against its own contract, not against
whatever happened to be true where it was called:

```omega
transition remaining {
    0 -> done()
    _ -> step(remaining - 1)     // here: remaining != 0
}
```

## Final calls, including suspension

An arm's call can be one that waits. The marker stays on the call:

```omega
machine finish_read(storage: &mut Storage) -> u32
suspends
{
    transition {
        _ -> suspend Storage::read(storage)
    }
}
```

`finish_read` finishes when the read does. Nothing is started in the
background. See [Chapter 18](chapter_18_concurrency.md).

## Where next

[Chapter 5](chapter_5_expressions_evaluation.md) covers expressions. The
complete rules are in
[nested machines](../spec/language/machines.md#nested-machines-and-identity)
and [contracts](../spec/language/state_contracts.md).
