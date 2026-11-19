# Chapter 17: Drops and cleanup

When an owned value goes out of scope, Omega cleans it up. Plain data needs
nothing. A resource names the machine that releases it.

## Select automatic cleanup

```omega
data MutexGuard<T> dropped by cleanup_mutex_guard<T> {
    mutex: &Mutex<T>;
}

machine cleanup_mutex_guard<T>(guard: MutexGuard<T>) {
    unlock_raw(guard.mutex);
    let MutexGuard { mutex as _ } = move guard;
}
```

`dropped by` picks the cleanup machine. It takes the value by value, because
cleanup is the end of it. The `let` pattern on the last line is what ends the
guard: it takes the wrapper apart into its fields, each of which is then an
ordinary value.

Owning the value lets cleanup hand a field to something that consumes it:

```omega
data Lease [linear] dropped by end_lease {
    pool: PoolHandle;
    slot: SlotClaim;
}

machine end_lease(value: Lease) {
    let Lease { pool, slot } = move value;
    PoolHandle::give_back(move pool, move slot);
}
```

There is no `Drop` trait. A cleanup machine is an ordinary machine, and
calling `end_lease(move lease)` yourself releases the lease early.

Automatic cleanup has to be something that cannot go wrong: it may not fail,
wait or crash. Closing a file that can report an error is an explicit call.

## Dispose of a value early

```omega
omega::core::drop(guard);                                  // run its cleanup now

let closed: CloseResult = block File::close(move file);    // a consumer with a result
let outcome: TaskOutcome<T> = suspend block Task::finish(move task);
```

After any of these the name can no longer be used.

## Take a value apart without cleaning it up

The package that declares a type can offer other ways to end a value, such as
returning its parts:

```omega
public machine into_parts(value: Lease) -> LeaseParts {
    let Lease { pool, slot } = move value;
    LeaseParts { pool: move pool, slot: move slot }
}
```

Only machines in the package that declares `Lease` may write that pattern.
Everyone else calls what the package makes public. Inside the package too, a
`Lease` that is moved, passed on or falls out of scope gets its normal cleanup;
only the explicit pattern ends it another way.

The parts keep their own cleanup. If a step must never be skipped, make it a
field, for example a `[linear]` registration that only `unregister` can
consume, so that every machine that takes the value apart has to deal with it.

## Values that must be finished

A `[linear]` value has no automatic fallback unless its type gives it one. It
must reach a consumer such as `commit`, `finish` or `abandon` on every path.
Use it when silently dropping the value would lose something real: a
transaction, a reservation, a running task.

## Cleanup order

Cleanup runs in reverse declaration order:

```text
record fields: file, socket        array slots:   0, 1, 2, 3
cleanup:       socket, file        cleanup:       3, 2, 1, 0
```

Locals are cleaned before the parameters declared before them. If a protocol
needs another order, write a consumer that does it explicitly.

## Branches

Each branch cleans up what it does not pass on, so branches that own different
things still arrive at the same state:

```text
branch A owns {file, scratch}     branch B owns {file}
             |                               |
        clean scratch                        |
             +--------------+----------------+
                            |
                     successor owns {file}
```

## Moving one field out

```text
before moving resources.file:   file live,  socket live
after moving resources.file:    file moved, socket live
remaining cleanup:             socket
```

A record with no `dropped by` can have a field moved out, and the rest is
cleaned up as usual. A type with `dropped by` cannot; its package provides
something like `into_parts`.

## Crashes skip cleanup

A crash or a process exit abandons values without cleaning them up. For a
graceful shutdown, finish tasks, release resources and return from the entry
machine. See [Chapter 16](chapter_16_errors_traps_failure.md).

## Where next

[Chapter 18](chapter_18_concurrency.md) covers tasks. The complete rules are
in [consumers and cleanup](../spec/language/ownership.md#consumers-and-cleanup).
