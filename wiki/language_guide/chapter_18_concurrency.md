# Chapter 18: Concurrency

Start a task when work should run in another activation. Keep its handle, do
other work, then finish it to obtain the outcome:

```omega
let task: Task<WorkResult> = TaskRuntime::start<WorkResult, Job, Worker::run>(move job);

do_other_work();

transition suspend block TaskLifecycle::finish(move task) {
    TaskOutcome::Returned { value } -> use_result(value)
    TaskOutcome::Cancelled -> handle_cancellation()
    TaskOutcome::Failed { receipt } -> handle_provider_failure(receipt)
}
```

This protocol sketch assumes a runtime whose `start` returns immediately and
whose `finish` permits suspension and blocking. The application supplies the
worker, job and outcome handlers. A direct `Worker::run(job)` call instead runs
in the current activation. `TaskRuntime` and `TaskLifecycle` are boundary traits
from `omega::core`, called by path like `Console::write_line`; the build selects
the provider behind them, and a library that starts tasks never chooses it.

## Start a task

The selected runtime provides task creation. `start<M>` requires enough
resources to start `M`; `try_start<M>` can report a dynamic refusal:

```omega
transition TaskRuntime::try_start<WorkResult, Job, Worker::run>(move job) {
    StartOutcome::Started { task } -> keep(move task)
    StartOutcome::Rejected { arguments, reason } -> recover(move arguments, reason)
}
```

On rejection, the caller receives the moved arguments and reservations back,
or their contract accounts for transfer to another owner. A static mismatch,
such as an incompatible runtime contract, is a compile error.

## Finish or transfer the task handle

`Task<T>` is linear: its holder is responsible for finishing the task or
transferring that responsibility. You can store it while other work proceeds:

```omega
data WorkerState {
    case Idle;
    case Running(task: Task<WorkResult>);
}
```

Moving the handle into `Running` transfers the obligation. Overwriting or
discarding it would lose that obligation and rejects. `TaskLifecycle::finish(move task)`
consumes the handle and returns the lifecycle outcome shown above. A worker's
own recoverable errors belong in `WorkResult`.

A supervisor can take responsibility for background work:

```omega
Supervisor::adopt(&mut supervisor, move task);
```

The supervisor must eventually finish or transfer it in turn. Automatic cleanup
cannot wait for a task; use the explicit completion operation.

## Mark calls that may wait

Suspension parks the activation so its worker can do other work. Blocking can
occupy the worker itself. A contract declares these possibilities with
`suspends` and `blocks`; a caller acknowledges them with prefixes:

```omega
let count: u32 = Counter::read(&counter);
let event: Event = suspend Inbox::take(&mut inbox);
let guard: Guard = block Mutex::lock(&mut mutex);
let outcome: Outcome = suspend block Operation::finish(move operation);
```

These examples show the minimum markers for the four combinations. A permitted
wait may complete immediately. Extra markers are allowed, so replacing a
provider with a synchronous one need not change every caller. When both are
written, the order is `suspend block`.

Put a call marked `suspend` at a place where evaluation can pause: a complete
statement, simple `let` right-hand side, transition subject, or terminal
expression. Split a larger expression into steps:

```omega
// Rejected: the marked call is nested in an addition.
let total: u64 = prefix + suspend Source::next(&mut source);

// Use a separate binding, with the usual addition bounds established.
let next: u64 = suspend Source::next(&mut source);
let total: u64 = prefix + next;
```

A redundant `suspend` still follows that placement rule. A blocking-only call
may nest. Borrowing and carry checks follow what the checked contract actually
permits. The optional [source rewrite](../spec/build/command_line.md#source-rewriting)
can remove redundant markers.

Published machines declare their suspension and blocking ceilings. Private
bodies can infer them. Static callback applications substitute the selected
callback's contract; [callback effects](chapter_19_capabilities_effects_boundaries.md#suspension-and-blocking-through-callbacks)
shows a synchronous specialization of a suspending traversal.

## Borrow values across a wait

A suspending read can borrow its destination buffer:

```omega
let count: u64 = suspend Storage::read(&mut storage, &mut buffer);
consume(buffer, count);
```

While `read` is parked, the exclusive loan remains live and `consume` has not
run. The buffer must stay valid and unavailable for conflicting access until
the read completes according to its contract.

Some retained resources impose further restrictions. A guard might forbid
suspension; a host handle might require the same thread on resumption. Omega
tracks four independent requirements:

| Requirement | What the runtime must preserve |
| --- | --- |
| Suspension | Whether the value may survive a park. |
| CPU | Whether execution must stay on the same CPU. |
| Host thread | Whether execution must stay on the same thread. |
| Address | Whether the value's location must stay stable. |

A record combines the requirements of its live fields. Moving the record or
putting it inside another value preserves them. The [carry reference](../spec/resources/carry.md)
explains declarations and permissions for resource authors.

## Keep a resource across a callback

One helper can retain a resource while calling synchronous or suspending code:

```omega
public machine around<T, machine F>(held: T) -> T
requires machine F() suspends;
suspends
{
    suspend F();
    held
}
```

Returning `held` keeps it live across F. The compiler records the condition:
if F can suspend, held must permit suspension. A synchronous selection therefore
accepts a suspension-forbidden guard. A suspending selection needs a resource
that can cross a park; otherwise the application rejects, naming the resource
and callback. The `suspend` marker stays valid in either case.

The helper still satisfies ordinary ownership, borrowing and invariant rules
at the call. Other carry requirements, such as staying on the same CPU or thread,
keep their usual checks. An independent suspending call needs suspension
permission regardless of F. See [conditional carry](../spec/language/effects.md#conditional-suspension-carry)
for the contract and forwarding rules.

## Cancel a task

`Task::request_cancel(&task)` asks for cancellation and leaves the task handle
live. Requesting cancellation does not prove that the task stopped or release
a borrowed buffer. Finish the task and honor any separate I/O settlement
contract before reusing its storage.

A worker handles cancellation through the outcome of a wait or explicit safe
point. For example, given a suspending `Ring::take`:

```omega
data Take {
    case Cancelled;
    case Got(frame: Frame);
}

machine run_worker(worker: &mut Worker, ring: &mut Ring) {
    transition suspend Ring::take(ring) {
        Take::Got { frame } -> work(worker, frame)
        Take::Cancelled -> cleanup(worker)
    }
}
```

The cancellation arm takes the worker's cleanup path. It does not unwind
arbitrary frames. A task that never reaches a cancellation point may never
observe the request, even if it eventually finishes normally.

## Wait for several events

Several producers can send different cases to one mailbox. The consumer waits
once, then handles the event:

```omega
data Event {
    case None;
    case Packet(frame: Frame);
    case Tick;
    case Shutdown;
}

machine run_server(server: &mut Server) {
    transition suspend Inbox::take(&mut server.inbox) {
        Event::Packet { frame } -> Server::handle(server, frame)
        Event::Tick -> Server::heartbeat(server)
        Event::Shutdown -> Server::drain(server)
        _ -> run_server(server)
    }
}
```

Mailboxes and supervisors are library data. Their closing protocols must
account for queued payloads, including linear values that still need a consumer.

## Choose a stack budget

Each task activation gets a fixed, nonmoving stack sized for its call graph.
It retains the stack while parked, preserving addresses used by live borrows
and external operations. Moving the task handle leaves that stack in place.

A stack pool gives predictable allocation and reuse, but parked tasks still
occupy slots. Size it for live activations, including parked ones. The runtime
must guarantee usable backing for each full stack plan throughout its lifetime;
the provider chooses reservation and physical commitment policy underneath
that guarantee. See [task storage](../spec/build/task_runtime.md#activation-description-and-stack).

Where the number comes from depends on what the call reaches. Checked Omega
code, your own or a library's, contributes a demand the compiler derives from
the final machine code, so changing that code changes the plan on the next
build and nothing has to stay stable. A separately compiled component publishes
its demand in its description, and changing it is a contract change. A foreign
leaf declares `stack N bytes` on its requirement, an admitted claim the
compiler cannot check. Whether the plan's pages are resident up front or arrive
on first touch is the provider's policy underneath the guarantee; the bound
does not move either way.

### Decide what scales with what

A task per activity is the simplest shape and the right one whenever the
population and its stack plans fit the budget. When they do not, ask what is
actually large, because the fix differs:

| What dominates | Structure |
| --- | --- |
| Nothing; the plans fit | One task per activity, ordinary suspending calls |
| A big temporary buffer used by few activities at once | Ordinary tasks that lease a workspace from a bounded pool around the expensive phase |
| One phase with deep call frames | Ordinary tasks that hand that phase to a separately provisioned worker and park while awaiting it |
| Too many incomplete activities to park each one | Persistent operation records advanced by bounded workers, as in the next section |

The first two fixes keep the connection as an ordinary suspending task and
move only the large thing out of its stack plan. With a pool of `K` workspaces
of size `E`, `N` connections of reduced plan `S` and persistent state `P` cost
about `N(S + P) + KE`. The bound is on outstanding leases, so release the
workspace before a long wait; a suspended operation that keeps one keeps its
lease. Acquiring may wait or fail, and the pool says which.

A library that must serve both the task-per-activity and the bounded-worker
shapes implements its protocol once as an incremental operation: a record that
owns the persistent state, buffers and pending I/O claims, plus an `advance`
that reports waiting or finished. A direct-style driver advances it and
suspends; a worker loop advances it and returns to the queue. The protocol
logic is written once and the storage profile is the driver's choice. The
boundary is real, though: a driver cannot turn an opaque suspending call into
an incremental operation, and an application callback that suspends on its own
still needs an activation or its own place in the operation record. Libraries
that never need the bounded-worker shape keep ordinary suspending interfaces.

### Many waiting connections, bounded workers

Every parked task retains its full stack-plan capacity guarantee, with physical
residency governed by the provider; bounded workers limit the number of these
plans while connection state waits in an arena.

For 100,000 mostly idle connections, consider keeping connection records and
buffers in an arena and using 32 worker tasks to process events. This gives
32 worker stack plans, plus the connection data, I/O state and queues.

```text
completion event -> queue -> available worker
                              1. update the connection record
                              2. submit its next I/O operation
                              3. return to the queue

waiting connection -> record + buffer + pending I/O claim in the arena
```

The worker returns after submitting I/O. Suspending inside a connection handler
would continue to occupy that worker and its stack. Queue capacity bounds
waiting submissions; worker count bounds simultaneous deep execution. Bound
the connection data and buffers separately.

Before returning to the queue, restore the record's default invariant and any
handoff guarantees. Keep its pending I/O claim until completion proves that
external access ended. A timeout or cancellation request alone cannot release
the buffer.

This explicit state management costs more authoring effort than one task per
connection. Use task-per-connection when its stack plans and population fit
your storage budget. The [blocking-executor package](../../source/library/blocking-executor/README.md)
describes the bounded-worker building blocks and provider work.

## Step an activation yourself

A task hands the right to resume its activation to a runtime. You can hold that
right instead and run a producer one piece at a time. The producer is an
ordinary machine that pauses with the same `suspend` marker; there is no `yield`
keyword. The names below illustrate the planned core library:

```omega
data Announcement { first: u32; second: u32; }

machine announce(out: &mut Emitter<u32>, job: Announcement) suspends {
    suspend Emitter::emit(out, job.first);
    suspend Emitter::emit(out, job.second);
}

machine drain(generator: Generator<u32>) suspends {
    transition suspend Generator::next(move generator) {
        Next::Item { value, generator } -> suspend handle(value, move generator)
        Next::Finished -> done()
    }
    machine handle(value: u32, generator: Generator<u32>) suspends {
        use_value(value);
        suspend drain(move generator)
    }
    machine done() {}
}
```

Each `next` runs `announce` on its own fixed stack until it emits or returns,
then control comes back to `drain` with the item and the generator to continue.
The generator is linear. Finish it, answer an item with stop so the producer runs
its cleanup, or move it to another owner; a paused activation is never dropped.
A producer that waits on something other than its own emits makes `next` wait
too, which is why it carries `suspend` here. Schedulers and tasks are libraries
over the same stepping operation. See
[owned activations](../spec/build/task_runtime.md#owned-activations-and-typed-replies).

## Safe points and response time

A scheduler can preempt and restore a task's register and stack state between
instructions. Structured cancellation, migration and replacement instead use
explicit semantic safe points:

```omega
process_simd_chunk(buffer);
suspend scheduler.poll();
```

Loops and allocations do not insert those polls automatically. Choose polling
points where retained values satisfy the carry and cleanup requirements.

A stack bound says how much storage is needed, not how soon work completes.
A blocked operation needs a reason it will unblock before you can promise a
response time. See [work and response bounds](../spec/resources/logical_work.md).

## Use atomics for shared updates

An atomic counter supports concurrent updates under its access contract:

```omega
data TicketLine {
    next_ticket: AtomicU32;
}

machine take_ticket(line: &mut TicketLine) -> u32 {
    AtomicU32::fetch_add(&line.next_ticket, 1, NoOrdering)
}
```

The operation returns the prior value from that same atomic update.
`NoOrdering` supplies atomic access and per-location order. Publishing other
data needs an appropriate ordering protocol:

| Ordering | Purpose |
| --- | --- |
| `Receive` | Observe work preceding a matching publication. |
| `Publish` | Publish preceding work to a matching receiver. |
| `ReceivePublish` | Combine those roles in a read-modify-write. |
| `GlobalOrder` | Participate in the shared global order. |

Loads cannot publish; stores cannot receive. Compare-exchange has separate
success and failure orderings, and a single-attempt operation may fail to
commit even when comparison matched. Its outcome also accounts for the
ownership of the proposed and displaced values.

Use the [atomic contracts](../spec/language/concurrency.md) when designing a
protocol. CPU ordering and device/DMA visibility involve different participants;
a device-status read alone does not prove that a device released its buffer.

## Check what can unblock a wait

Start with these common deadlocks:

```text
Completion cycle: A waits for B; B waits for A.
Lock inversion:   one path takes A then B; another takes B then A.
Missing producer: receive has no reachable send, close, timeout, or event.
```

Check barrier counts and external waits too. Memory safety protects the state
of a parked task; a progress guarantee additionally needs a scheduler and
protocol that can resume it. Deployment policies can require those proofs.
See [protocol proofs](../spec/language/concurrency.md#protocol-proofs) for
deadlock, starvation and response guarantees.
