# Chapter 3: Machines

A machine is Omega's unit of code: it takes inputs, does work and usually
returns a value.

```omega
machine double(value: u32) -> u32
requires value <= 1000
{
    value * 2
}

let answer: u32 = double(21); // 42
```

The last expression is the result. `requires value <= 1000` is a
precondition: every caller must show it holds, and in exchange the body may
rely on it, which is how the multiplication is known not to overflow.

## Data is always a parameter

```omega
data Player {
    health: i32;
}

machine take_damage(player: @mutable Player, amount: i32)
requires amount >= 0
requires player.health >= amount
{
    player.health = player.health - amount;
}

take_damage(@mutable player, 4);
```

A field call such as `ops.fill(buffer)` requires an actual callable field.
It passes only the arguments inside the parentheses. For the machine above,
pass the player explicitly. Group related machines in a
[file](chapter_15_modules_imports_visibility.md) and call its exports through
the import alias.

## The header

Clauses follow the signature with no semicolons between them. The body, or
the next clause, ends a clause:

```omega
machine identity(value: u64) -> u64
requires value <= 100
ensures result == value
terminates
{
    value
}
```

| Clause | Says |
| --- | --- |
| `requires` | What the caller must establish. |
| `ensures` | What the caller may rely on afterwards; `result` names the return value. |
| `terminates` | The machine always finishes. |
| `reaches`, `crashes`, `suspends`, `blocks` | What it may touch and how it may fail or wait. See [Chapter 16](chapter_16_errors_traps_failure.md), [18](chapter_18_concurrency.md) and [19](chapter_19_capabilities_effects_boundaries.md). |

A machine is private unless marked `public`. A private machine's omitted
operational clauses are worked out from its body. On a public machine, leaving
off `suspends`, `blocks` or `crashes` promises it does none of them.

A machine always has a brace body; there is no `machine f(...) = expr;` form.
A declaration with no body ends in a semicolon and is a requirement that
something else implements:

```omega
public boundary trait InterruptCompletion {
    machine complete(acknowledgement: InterruptAcknowledgement)
    reaches PortIo
    requires acknowledgement in InterruptAcknowledgement::Pending
    crashes none;
}
```

[Chapter 14](chapter_14_traits.md) covers traits and
[Chapter 19](chapter_19_capabilities_effects_boundaries.md) covers boundary
traits like this one.

## The program entry

If `hello.omg` defines only `machine main`, `omega run hello.omg` uses
core's default Build. To run a project with a custom build, name the file
containing `machine build`, for example `omega run release.omg`. That build
picks the product entry, here `start`. The entry creates the program's state
and passes it on. [Build-first entry selection](../spec/build/command_line.md#build-first-entry-selection)

```omega
data Application {
    total: i32;
}

machine run(application: @mutable Application) {
    let increment: i32 = add_i32(3, 4);
    application.total = increment;
}

machine start() {
    let mutable application: Application;
    run(@mutable application);
}
```

`let mutable application: Application;` starts from zero. A type whose invariant
does not hold at zero needs a constructor machine instead. There is no global
program state: anything long-lived is created here and handed down. See
[program entries](../spec/build/entry_roots.md).

## Calling

```omega
let command: Command = resolve_command(@parser, @line);
let guard: Guard = block Mutex::lock(@mutable mutex);
let event: Event = suspend Inbox::take(@mutable inbox);
```

A call that may wait is marked at the call site: `block` when it may block the
thread, `suspend` when it may park the task.
[Chapter 18](chapter_18_concurrency.md) explains the difference.

## Promising a result

```omega
machine double(value: u32) -> u32
requires value <= 100
ensures result <= 200
{
    value * 2
}
```

The body proves the `ensures`; callers get to use it. Exact calls checked
together can also use routine facts from the body. Independently checked clients rely on the
published promise, so write the result guarantees they need.

A by-value parameter's name in a contract means the original input, even when
the body changes a `mutable` parameter. A promise `result == position + step`
still uses the supplied position and step. For a mutable reference, ordinary
postcondition reads describe the final contents; `old(...)` describes entry
contents. These are proof observations, not extra copies or ownership. See
[contract inputs](../spec/language/state_contracts.md#contract-inputs-and-working-values).

A private helper can leave off `requires` and let the compiler infer simple
input conditions, checked at every call. Once you write `requires`, it must
cover everything the body needs; the compiler cannot quietly add more conditions.
Public machines must handle all inputs their published contract admits. See
[local calls and contracts](../spec/language/state_contracts.md#local-call-summaries).

[Chapter 7](chapter_7_types_constraints_invariants.md) shows how these facts
flow through branches and calls.

## Loops are recursion

Omega has no loop statement. A machine repeats by calling itself as its last
action, and a machine that promises to finish says what gets smaller:

```omega
machine countdown(n: u64)
terminates by n -> |nat|::Nat::Descending
{
    transition n {
        0 -> {},
        _ -> countdown(n - 1)
    }
}
```

Two rules apply:

- **A runtime recursive call must be the last thing the arm does.** It then
  compiles to a jump and the stack does not grow. `1 + f(n - 1)` is rejected at
  run time; keep pending work in explicit storage instead.
- **`terminates by` is only needed when you promise termination.** An event
  loop that runs forever owes no ranking.

Proofs and compile-time evaluation may recurse in any position.
[Chapter 4](chapter_4_nested_machines_transitions.md) introduces `transition`,
and [termination](../spec/language/termination.md) lists the other ways to
measure progress, such as a cursor moving up to a bound or a tree getting
smaller.

## One declaration, several uses

The same machine can be called at run time, evaluated at compile time, cited
as a proof, started as a task or supplied where a trait asks for one. Its
contract decides which of those are allowed. An operator is a machine too:

```omega
machine + add(left: Vector, right: Vector) -> Vector { ... }
```

See [operators](chapter_5_expressions_evaluation.md#operators).

## Where next

[Chapter 4](chapter_4_nested_machines_transitions.md) splits a machine into
steps. The complete rules are in [machines](../spec/language/machines.md) and
[contracts](../spec/language/state_contracts.md).
