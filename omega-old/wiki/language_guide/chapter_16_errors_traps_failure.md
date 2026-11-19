# Chapter 16: Errors, traps, and failure

There are no exceptions. A failure the caller can handle is a return value. A
failure nobody can handle is a crash, and a machine has to say when it can
crash.

## Return a sum

```omega
data ParseResult {
    case Parsed(value: u32);
    case BadDigit(byte: u8);
}

machine parse_digit(byte: u8) -> ParseResult {
    transition byte >= 48 && byte <= 57 {
        true -> ParseResult::Parsed { value: (byte - 48) as u32 }
        false -> ParseResult::BadDigit { byte: byte }
    }
}

machine digit_or_zero(byte: u8) -> u32 {
    transition parse_digit(byte) {
        ParseResult::Parsed { value } -> value
        ParseResult::BadDigit { byte } -> 0
    }
}
```

The caller has to cover both cases, and `value` only exists in the `Parsed`
arm. A library `Result<T, E>` is the same thing with type parameters.

To ignore a result on purpose, write `_ = call();`. That does not work for a
`[linear]` result, which still has to be finished.

## Translating an error

```omega
data SettingResult {
    case Level(value: u32);
    case Invalid;
}

machine parse_level(byte: u8) -> SettingResult {
    transition parse_digit(byte) {
        ParseResult::Parsed { value } -> SettingResult::Level { value: value }
        ParseResult::BadDigit { byte } -> SettingResult::Invalid
    }
}
```

There is no `?` operator. Each layer matches and decides.

## A result can carry a fact

```omega
domain u32::TableIndex requires self < 16;

data Slot {
    case Found(index: u32 in TableIndex);
    case Full;
}
```

In a `Found { index }` arm the caller already knows `index < 16` and can index
a sixteen-entry table without checking again.

## Compile errors are not runtime failures

Dividing by something that might be zero is a compile error. To accept
untrusted input, branch and return an error for the bad case:

```omega
data Division {
    case Quotient(value: u32);
    case ZeroDivisor;
}

machine try_divide(numerator: u32, denominator: u32) -> Division {
    transition denominator {
        0 -> Division::ZeroDivisor
        _ -> Division::Quotient { value: numerator / denominator }
    }
}
```

## Crashes

A crash stops the current execution without running cleanup. There are two
kinds:

| Clause | Permits |
| --- | --- |
| `crashes Trap` | A fault from an operation, such as `+!` overflowing. |
| `crashes Abort` | Deliberately ending execution. |

```omega
machine require_ready(ready: bool)
crashes Abort
{
    transition ready {
        true -> {}
        false -> abort()
    }

    machine abort() crashes Abort {
        crash Abort;
    }
}
```

A public machine with no `crashes` clause promises it cannot crash. If you
edit a helper so that it can trap, every public machine that reaches it stops
compiling until it says `crashes Trap`. A private machine's clause is worked
out from its body.

The clause permits a crash; it does not cause one. To get trapping arithmetic
write `+!`.

## Machines that never return

A machine that can never come back to its caller says so with the core type
`Never` as its result. Callers in other packages rely on that signature, not
on the body:

```omega
public machine fail() -> Never crashes Abort {
    crash Abort;
}

machine port_or_fail(parsed: bool, port: u16) -> u16 crashes Abort {
    transition parsed {
        true -> port
        false -> fail()
    }
}
```

No value of `Never` is ever made, and a caller never unwraps one. Because
`fail()` cannot return, it fits where a `u16` is expected. Code written after
such a call is accepted as unreachable, with a warning.

A machine that exits or crashes on some paths and returns on others keeps its
ordinary result type. Its `ensures` describes the paths that return.
`ProcessExit::exit_process` returns `Never`, so a helper that exits only when
something is fatal still returns its normal value otherwise. See
[published no-return](../spec/language/state_contracts.md#published-no-return).

## After a crash

An error return runs normal cleanup. A crash abandons everything in flight: a
device may be half-programmed, a lock may be held. Restarting gives a fresh
start, not a resumed one, so code that survives another part's crash needs an
isolation boundary between them. See
[recovery](../spec/language/effects.md#recovery-and-execution-domains).

## Stack overflow is not a runtime failure

The compiler adds up the stack a program needs, including what it uses itself,
before the program runs. Recursion that could grow the stack is rejected
([Chapter 3](chapter_3_machines.md#loops-are-recursion)), so there is no
stack-overflow path to handle.

## Host errors

Operating-system failures use the same pattern:

```omega
data ReadOutcome {
    case Read(bytes: u64);
    case Closed;
    case Failed(error: IOError);
}
```

Cancellation of a task is delivered the same way, as data at a point where the
task waits. See [Chapter 18](chapter_18_concurrency.md#cancel-a-task).

## Where next

[Chapter 17](chapter_17_drops_and_cleanup.md) covers cleanup. The complete
rules are in [guarded crashes](../spec/language/effects.md#guarded-crashes).
