# Chapter 4: States And Transitions

A state is an internal control label. A transition arm targeting a local state
transfers values there without adding a call frame. An arm can alternatively
complete the machine with a value or an ordinary final call's result.

```omega
machine countdown(remaining: u32) -> u32 {
    transition {
        _ -> step(remaining)
    }

    state step(remaining: u32) {
        transition remaining {
            0 -> done()
            _ -> step(remaining - 1)
        }
    }

    state done() {
        0
    }
}
```

The nonzero arm establishes that unsigned subtraction is valid. This example
shows control transfer; an exported termination promise additionally uses the
ranking contract explained in [Chapter 9](chapter_9_proof_obligations.md).
The [state specification](../spec/language/state_contracts.md) owns arrival,
mutation, and return obligations.

## Working Rules

Calling a machine enters its top-level body. When internal states follow, setup
ends in an explicit tail `transition`; there is no separately authored entry
member. Ordinary calls target machines; internal jumps target states in the
current machine. A machine call written as the final arm operation remains a
call, not a state jump.

A jump has no return address and does not resume its source state later.
Completion ends the invocation and must satisfy its result and postconditions.
An implicit Unit completion cannot satisfy a value-returning contract.

## State Parameters

A state's parameters are its explicit input frontier:

```omega
machine forward(value: Inventory) -> Inventory {
    transition {
        _ -> done(move value)
    }

    state done(value: Inventory) {
        value
    }
}
```

Machine parameters do not become ambient names inside every state. Locals and
state inputs identify what the body may observe, mutate, move, or lend.
Every incoming edge establishes that state's requirements.

A declared state receiver, such as `state advance(&mut self, index: u64)`,
retains the machine attachment. The jump writes `advance(next_index)`;
`self` is not repeated among its ordinary arguments. This does not authorize
rebinding the receiver to another object.

Explicit transfer need not copy bytes. Storage planning may reuse the same
slot while proof and debug artifacts retain the exact mapping. Owned values
still transfer once, and dying values need their legal disposition.

## Data Patterns

A transition can destructure its saved subject:

```omega
transition header {
    Header { ok: 0, version } -> accept(version)
    Header { ok as _, version as _ } -> reject()
}
```

`version` binds the field; `ok: 0` contributes projected equality;
`field as name` renames; `field as _` waives. Without `..`, every field
must be mentioned. Adding a field then requires the author to revisit the pattern.
`..` deliberately opts out of that check in arm position.

Case payloads use the same vocabulary. The subject evaluates once; extraction
uses that value. [Pattern matching](chapter_6_pattern_matching_dispatch.md)
explains ordered selection and coverage.

## No Silent Fall-Through

A dispatch must cover its admitted subject. An uncovered runtime possibility
is a compile error, not an implicit trap. Complete finite case/Boolean coverage
or a wildcard establishes coverage; arbitrary guard ladders need a fallback.

An explicit `_ -> {}` completes with Unit. A value-producing context instead
needs its declared result. No fallback silently discharges cleanup, authority,
or return obligations.

## Terminal Completion

A final expression supplies the machine result:

```omega
machine answer() -> i32 {
    transition {
        _ -> done()
    }

    state done() {
        42
    }
}
```

The jump is internal transfer; `42` is a returned value. Return postconditions
are checked against the live facts on that exact edge, not assumed from the
machine's entry contract after mutation.

### Final calls, including suspension

Assume `Storage::read(storage)` returns `u32`, permits suspension but not blocking,
and the borrowed storage satisfies its loan/carry requirements:

```omega
machine finish_read(storage: &mut Storage) -> u32
suspends;
{
    transition {
        _ -> suspend Storage::read(storage)
    }
}
```

This is an ordinary call followed by completion with its result. If it parks,
`finish_read` has not returned; when it completes normally, the returned value
must satisfy `finish_read`'s result contract and ordinary cleanup still applies.
Unit-returning calls work the same way. No task is launched or detached, and
syntactic tail position cannot release storage still borrowed by the callee.

Use `block` or `suspend block` when the selected call's contract requires them.
A local state jump takes neither marker: any suspending call in the destination
state is acknowledged where it is invoked. See
[terminal arm calls](../spec/language/state_contracts.md#terminal-calls-in-transition-arms)
and [borrows across suspension](chapter_18_concurrency.md#borrows-across-suspension).

## Lowered Graphs

One source state may lower to several basic blocks. Tools retain correspondence
to the source state and its edges. That permits optimization without changing
arrival requirements, ownership transfers, effects, or terminal outcomes.
