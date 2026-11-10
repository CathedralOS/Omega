# Chapter 17: Drops and cleanup

When an owned value reaches the end of its lifetime, Omega checks how it is
disposed of. Moving it transfers that responsibility. Plain values need no
cleanup; a resource may need a cleanup operation or an explicit consumer.

## Select automatic cleanup

The type owner chooses a cleanup machine with `dropped by`. This guard releases
its mutex when consumed; the sketch assumes the mutex's checked release API:

```omega
data MutexGuard<T> dropped by cleanup_mutex_guard<T> {
    mutex: &Mutex<T>;
}

machine cleanup_mutex_guard<T>(guard: MutexGuard<T>) {
    unlock_raw(guard.mutex);
    let MutexGuard { mutex as _ } = move guard;
}
```

The cleanup machine takes the guard by value: cleanup ends the value, so the
machine doing it owns it. The `let` pattern on the last line is what ends the
guard. It takes the wrapper apart, and each component is then an ordinary
owned value with its own cleanup; here the only component is a reference,
which needs none.

Until that pattern, the guard is an ordinary `MutexGuard<T>`. If a cleanup
machine let it reach the end of the body whole, that would be cleanup calling
itself, and the termination check rejects it.

Owning the value is what lets cleanup hand a component to an operation that
consumes it:

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

A cleanup machine is an ordinary machine, so calling `end_lease(move lease)`
yourself disposes of the lease early.

### Take a value apart without cleaning it up

The same pattern lets the type's own package offer operations that end the
value another way, such as handing its parts to the caller:

```omega
public machine into_parts(value: Lease) -> LeaseParts {
    let Lease { pool, slot } = move value;
    LeaseParts { pool: move pool, slot: move slot }
}
```

Only machines declared in the package that declares `Lease`, and its selected
cleanup machine, may write that pattern. Everyone else calls what the package
makes public. Nothing is skipped by accident: inside the package too, a `Lease`
that is moved, passed along or left to fall out of scope gets its normal
cleanup, and only the explicit pattern ends it differently.

The pattern ends the wrapper only. The parts keep their own cleanup and their
own obligations, and the checker accounts for each. What it cannot check is
work that exists only as a line in your cleanup machine. If a step must never
be skipped, hold it as an owned component, for example a linear `Registration`
that only `unregister` can consume; then every machine that takes the value
apart has to deal with it.

Automatic cleanup must return normally, without failure, suspension, blocking,
trapping, aborting or process exit. It can use only authority and facts available
at that point. Closing a resource that may wait or fail therefore needs an
explicit operation.

A default domain or named domain may select cleanup. Combining domains must
resolve to at most one distinct cleanup machine application; field cleanup
then follows structurally. Removing a qualification does not remove cleanup
that the value still owes. See [domain-selected cleanup](../spec/language/domains.md#domain-selected-automatic-cleanup).

## Dispose of a value early

Call the ordinary consuming core machine:

```omega
omega::core::drop(guard);
// `guard` has moved and cannot be used again.
```

This uses the same checked cleanup plan as an ordinary lifetime end. No `Drop`
trait is needed. Generic containers compose their elements' cleanup plans;
an application rejects if those elements cannot legally be disposed of there.

For protocols with a result or waiting behavior, use their named consumer:

```omega
let closed: CloseResult = block File::close(move file);
let outcome: TaskOutcome<T> = suspend block Task::finish(move task);
```

Handle the result according to its contract. `finish` settles the task;
`suspend block` only acknowledges how the call may wait.

## Keep linear obligations accounted for

An affine resource may have an automatic fallback. A linear resource must reach
exactly one authorized disposition. The type owner can authorize automatic
cleanup when it meets the restrictions above; otherwise use an explicit
consumer such as `commit`, `unregister`, `finish` or `abandon`.

This matters when abandonment loses external capacity. A sound memory model
does not make silently losing that capacity appropriate. Such a protocol can
require an explicit `release` or `abandon` choice, with deployment policy
restricting abandonment further.

Putting reclamation work into a queue transfers the obligation to the queue.
Its contract must account for capacity and eventual servicing. The original
resource has not been released merely because it was enqueued.

## Cleanup on branches and returns

Omega first evaluates outgoing arguments or a return value, then transfers
their ownership and cleans up values that remain behind. Different branches
can therefore have different cleanup:

```text
branch A owns {file, scratch}     branch B owns {file}
             |                               |
        clean scratch                        |
             +--------------+----------------+
                            |
                     successor owns {file}
```

The successor receives the same required state in either case. Nested machines
spell their parameters and transfers explicitly; a transfer can reuse storage
without copying bytes when the storage plan permits it.

If runtime alternatives require different cleanup, represent the difference
with a sum or another explicit state value. For example, a resource held by
two possible providers needs enough information to select the right release
protocol. See [ownership joins](../spec/language/ownership.md).

## Cleanup order

Cleanup follows reverse declaration order. A selected cleanup machine runs
first; components its pattern did not bind are cleaned in reverse declaration
order. Arrays clean
remaining elements from highest index to lowest; sums clean their active
payload. Locals precede by-value parameters declared before them.

```text
record fields: file, socket
cleanup:       socket, file

array slots:   0, 1, 2, 3
cleanup:       3, 2, 1, 0
```

Borrow dependencies must fit that order. An owner cannot die before a cleanup
action that still borrows it. If a protocol needs another sequence, express
it in an explicit consumer.

A partially evaluated literal differs from a completed record: only the fields
already established exist. If construction leaves through an ordinary cleanup
path, those fields are cleaned in reverse evaluation order. A crash performs
no cleanup.

## Move part of a value

An aggregate with structural cleanup can keep track of its remaining fields:

```text
before moving resources.file:   file live,  socket live
after moving resources.file:    file moved, socket live
remaining cleanup:             socket
```

The moved field's recipient owns its cleanup. A second move of that same field
rejects. Arrays follow the same rule for identified elements, cleaning the
remaining indices in descending order.

A type with a selected cleanup machine cannot have a single field moved out
this way. Its package provides a consuming operation that takes the whole
value apart, as `into_parts` does above, if users need its fields. A dynamic index also needs enough evidence to
identify the unique owned element being removed.

## Prove cleanup is legal where it runs

A cleanup machine can have preconditions. They must hold at the point the
value dies. For example, a release protocol may require that a transaction
has transferred its outstanding work to a stable custodian.

If the proof fails, keep the value live, establish the missing fact, or call
an explicit consumer that handles the current state. A diagnostic names the
cleanup operation, its missing premise and the outgoing edge.

Body analysis cannot silently add a new caller precondition to make cleanup
legal. State required facts in the machine's contract. Generic code retains
the corresponding obligations until its types are substituted.

## Include cleanup in resource planning

Cleanup contributes service reach, work and stack demand to the machine that
performs it. Cleanup on a loop backedge runs on every iteration, so include
it in the iteration cost. Termination alone supplies no numeric work bound.

Crashes and canonical process exit abandon obligations instead of cleaning
them. See [failure and recovery](chapter_16_errors_traps_failure.md).
The [ownership reference](../spec/language/ownership.md#consumers-and-cleanup)
and [cleanup representation](../spec/terminal-psi/ownership.md) describe the
compiler's edge plans, conservation evidence and physical realization.
