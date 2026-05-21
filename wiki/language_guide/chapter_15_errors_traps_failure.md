# Chapter 15: Errors, Traps, And Failure

Failure semantics must be explicit.

Omega should not have hidden exceptions as ordinary control flow. Failure should
be represented as data, a declared trap, or a trusted host boundary.

## Fallible Results

Recoverable failure should be data.

```omega
data ReadResult {
    ok: bool;
    bytes_read: usize;
    error: IOError;
}

machine File::read(
    &mut self,
    out: &mut Buffer,
    result: &mut ReadResult
) {
}
```

Library conventions may provide `Option<T>` and `Result<T, E>` data shapes, but
they do not need special exception semantics.

## Traps

A trap is unrecoverable for the current machine invocation.

Examples:

- proven-impossible state reached,
- failed exact arithmetic proof in a checked runtime mode,
- violated trusted boundary contract,
- target fault reported by a host or OS boundary.

Traps must be visible in effects and build artifacts.

## No Hidden Unwind

Cleanup still happens along known graph edges.

If the language later supports unwinding, it must be modeled as explicit graph
edges with cleanup and proof obligations. It should not be an invisible second
control-flow system.

## Host Failure

Host calls and syscalls must declare how they fail.

```omega
machine HostFile::read(
    handle: HostHandle,
    out: &mut Buffer,
    result: &mut IOError
)
trust host
{
}
```

The contract decides whether failure is data, blocking, trap, or trust-boundary
violation.

## Cleanup On Failure

Failure paths must preserve ownership and cleanup obligations.

Working rules:

- Fallible data returns are ordinary machine returns or output parameters.
- Trap edges must clean up owned locals before leaving the current graph.
- Host boundaries must document whether resources remain valid after failure.
- Drop machines may themselves have effects, but failure inside cleanup must be
  tightly restricted.
