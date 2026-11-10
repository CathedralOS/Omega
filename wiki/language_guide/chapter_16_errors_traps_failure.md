# Chapter 16: Errors, traps, and failure

Return a sum when the caller can handle failure. Its cases describe what
happened and carry the information needed to continue.

## Handle a failed operation

This parser accepts one ASCII digit:

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

The true arm proves subtraction is valid. The caller covers both outcomes,
and `value` is available only in the `Parsed` arm. Larger parsers can add
cases such as `Empty` or carry the failing offset.

Use `_ = call();` when intentionally discarding a result that may legally be
disposed of. A linear result still needs its required consumer; discarding its
name cannot dispose of an unfinished task or transaction.

## Return an error

A wrapper can translate a failure into its own result type. Here an invalid
digit becomes an invalid setting:

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

The transition makes the choice explicit. Each outgoing path must transfer or
clean up its owned values and restore borrowed storage. A library
`Result<T, E>` follows these same sum and ownership rules.

## Use facts from a result

A successful payload can carry a useful bound:

```omega
domain u32::TableIndex requires self < 16;

data Slot {
    case Found(index: u32 in TableIndex);
    case Full;
}
```

Constructing `Found` takes an index cast with `as +TableIndex`, and that cast
proves the range. In a `Found { index }` arm, the caller
can use that bound when indexing a sixteen-entry collection. Ordinary borrow
and indexing requirements still apply.

Across a call, `requires` states what the caller proves and `ensures` states
what completion establishes. An error outcome must fulfill the guarantees
attached to that outcome too. See [contracts](chapter_9_proof_obligations.md).

## Distinguish proof errors from runtime failure

An unproved Exact division or array index is a compile error. To accept
untrusted input, branch first and return an error for the invalid case:

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

The nonzero arm proves the division safe. No handler or runtime check is
inserted merely because the checker could not prove an obligation.

## Declare possible crashes

A crash ends the activation without a normal successor or cleanup.
`crashes Trap` permits specified operation or platform faults;
`crashes Abort` permits deliberate termination of the execution domain.
On a public machine, leaving the clause off promises there are no crash
routes, so a helper edit that introduces a trap fails to compile at that
boundary until the public declaration says `crashes Trap` with its guard. A
private helper with no clause has its routes derived from its body. On a
callback requirement, leaving the clause off accepts whatever the selected
callback publishes, and `crashes none` accepts only crash-free callbacks.
Declare Abort when a machine can deliberately end execution:

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

This declaration permits Abort for the machine. A guarded crash clause can
narrow that permission to particular input conditions; a caller that rules out
those conditions can rule out the crash. Omitting a cause forbids it.

A clause describes permitted behavior. To get trapping arithmetic, write a
trapping operator such as `+!` or operate on a `Trapping` value; adding a crash
clause to Exact arithmetic does not change its meaning. See [guarded crashes](../spec/language/effects.md#guarded-crashes).

## Plan recovery separately

Returning an error takes an ordinary cleanup path. Crashing abandons the live
state: a device may be half-programmed or a peer may have observed half a
protocol. A supervisor needs isolation, an owner-death protocol, or a reset
guarantee before continuing with affected resources.

Restarting creates a fresh activation. It does not resume the abandoned
computation or establish that its buffers are safe to reuse. See
[recovery contracts](../spec/language/effects.md#recovery-and-execution-domains).

Likewise, process exit abandons outstanding obligations. For graceful shutdown,
finish tasks, release resources and return from the entry operation. Automatic
cleanup must itself return normally.

## Provision stack storage before execution

Omega accounts for stack demand, including compiler spills, before execution.
The activation needs sufficient aligned, accessible backing for its lifetime.
A provisioning failure belongs to startup or admission; source code does not
gain a stack-overflow branch because register allocation used more space.

The [stack reference](../spec/resources/storage.md#compiler-owned-stack-accesses)
covers call chains, tail cycles and physical backing.

## Handle host errors and cancellation

A host API can use the same sum pattern:

```omega
data ReadOutcome {
    case Read(bytes: u64);
    case Closed;
    case Failed(error: IOError);
}
```

Its contract also states which buffers and handles remain valid after each
outcome. Whether the read blocks or suspends is a separate declaration.

Cancellation is similarly delivered through task protocol data at a declared
wait or safe point. Requesting it leaves the task's completion obligation live.
[Concurrency](chapter_18_concurrency.md#cancel-a-task) shows the full lifecycle.
