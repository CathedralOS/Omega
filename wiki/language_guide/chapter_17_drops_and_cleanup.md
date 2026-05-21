# Chapter 17: Drops And Cleanup

Owned values need a deterministic cleanup story.

The language should not pretend cleanup is a library convention. If owned values
can manage heap storage, locks, file handles, sockets, GPU buffers, or other
resources, the compiler must know when those values stop being live and what
cleanup obligations remain.

Working model:

- Plain values with no cleanup simply stop being live.
- Resource-owning values may have a cleanup machine.
- The compiler inserts cleanup on graph edges where an owned value dies.
- Transition arguments can move ownership to the target state.
- Cleanup is visible in lowered graphs and proof artifacts.
- Automatic cleanup must be infallible. Fallible release needs an explicit
  machine such as `close`, `flush`, or `commit`.

## Cleanup Machines

A type can define cleanup through a reserved machine shape.

```omega
data MutexGuard<T> {
    mutex: &Mutex<T>;
}

machine MutexGuard::drop(&mut self)
    effects nonblocking
    ensures self.mutex unlocked
{
    self.mutex.unlock_raw();
}
```

The exact syntax is provisional, but the semantics are not:

- `Mutex::lock` can return `MutexGuard<T>`.
- `MutexGuard<T>` owns the obligation to unlock.
- When the guard dies, the compiler inserts `MutexGuard::drop`.
- The drop guarantee contributes the fact that the mutex is unlocked.
- If a context forbids blocking cleanup, `drop` must satisfy that effect
  requirement.

## Edge Cleanup

Cleanup is graph-aware, not merely textual.

```omega
state locked(&mut self) {
    let guard: MutexGuard<Data> = self.data.lock();

    transition self.mode {
        Mode::Read -> read_with_guard(move guard)
        Mode::Write -> write_with_guard(move guard)
        Mode::Done -> done()
    }
}
```

The lowered edge behavior is:

```text
locked -> read_with_guard:
  move guard into target
  do not drop guard

locked -> write_with_guard:
  move guard into target
  do not drop guard

locked -> done:
  drop guard
  jump done
```

The rule:

```text
If an owned value is live before a transition edge and is not moved into that
edge's target arguments, the edge must clean it up before jumping.
```

That requires move/liveness analysis over the machine graph. It is the same
reason transition arguments and state parameters must be explicit.

## Transferring Cleanup Responsibility

Moving a resource into a target state transfers the cleanup obligation.

```omega
state read_with_guard(guard: MutexGuard<Data>) {
    self.read_data(&guard);

    transition {
        _ -> done()
    }
}
```

Here `read_with_guard` owns `guard`. Its outgoing edge to `done` must either
drop `guard` or move it again.

Working rules:

- A moved value is unavailable in the source after the edge.
- A target that receives a resource owns its cleanup obligation.
- A resource value cannot disappear from the graph without cleanup.
- A must-cleanup value cannot be copied unless its type explicitly supports
  shared cleanup semantics.

## Drop Order

Drop order must be deterministic.

Tentative rule:

- Locals drop in reverse creation order on each edge.
- Fields drop after the owning value's cleanup machine.
- Field drop order should be fixed by the language, not target lowering.
- Partially moved values drop only the fields that remain live.

The exact field order is less important than choosing one and making it visible
to tooling.

## Effects And Failure

Automatic cleanup should be boring.

Suggested restrictions:

- Automatic `drop` cannot return a recoverable error.
- Automatic `drop` should be nonblocking by default.
- Blocking cleanup must be declared and may be rejected in contexts that require
  progress or interrupt safety.
- Fallible operations should be explicit machines, such as `file.close()` or
  `transaction.commit()`.

This avoids hiding important control flow behind cleanup while still giving the
language a complete ownership story.

## Relationship To Proofs

Cleanup operations emit requirements and guarantees like any other operation.

For a mutex guard:

```text
drop guard
requires guard owns lock
ensures mutex unlocked
```

For a file handle:

```text
drop file
requires file owns handle
ensures handle released
trusts platform close contract
```

These facts matter for:

- borrow checking,
- invariant restoration,
- deadlock and waitable-resource proofs,
- hot-swap quiescence,
- host-boundary trust reports.

Drop is therefore part of the same proof system as moves, transitions, effects,
and host contracts.

## First Implementation Shape

The compiler can start smaller than the full model.

Initial scope:

- Track owned locals across machine graphs.
- Insert structural cleanup for values with known cleanup.
- Reject user-defined fallible drop.
- Reject resource values that reach an edge without cleanup or move.
- Show lowered cleanup edges in debug/proof artifacts.

This gives the language a complete answer without requiring every resource API
to be designed up front.
