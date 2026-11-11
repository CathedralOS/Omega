# Chapter 18: Concurrency

A task runs a machine on its own stack. You start it, keep its handle, and
finish it to get the result.

```omega
let task: Task<WorkResult> =
    TaskRuntime::start<WorkResult, Job, Worker::run, ActivationStorage::Fixed>(move job);

do_other_work();

transition suspend block TaskLifecycle::finish(move task) {
    TaskOutcome::Returned { value } -> use_result(value)
    TaskOutcome::Cancelled -> handle_cancellation()
    TaskOutcome::Failed { receipt } -> handle_provider_failure(receipt)
}
```

`start` takes the result type, the argument type, the machine to run and how
its stack is stored. The build chooses the runtime behind `TaskRuntime`; code
that starts tasks never does.

## The handle must be finished

`Task<T>` is `[linear]`. Dropping it is a compile error: you finish it, store
it, or hand it to something that will.

```omega
data WorkerState {
    case Idle;
    case Running(task: Task<WorkResult>);
}

Supervisor::adopt(&mut supervisor, move task);
```

When starting can be refused at run time, `try_start` hands the arguments
back:

```omega
transition TaskRuntime::try_start<WorkResult, Job, Worker::run, ActivationStorage::Fixed>(move job) {
    StartOutcome::Started { task } -> keep(move task)
    StartOutcome::Rejected { arguments, reason } -> recover(move arguments, reason)
}
```

## Mark calls that may wait

There are two ways to wait. Suspending parks the task so its worker thread
can run something else. Blocking holds the thread. A machine declares which it
may do, and each call acknowledges it:

```omega
let count: u32 = Counter::read(&counter);
let event: Event = suspend Inbox::take(&mut inbox);
let guard: Guard = block Mutex::lock(&mut mutex);
let outcome: Outcome = suspend block Operation::finish(move operation);
```

A marker says the call may wait, not that it will. Extra markers are allowed.
A `suspend` call stands alone as a statement, a `let` initializer or a
transition subject; it cannot sit inside a larger expression. There is no
`async` keyword and no separate kind of function: a suspending machine is an
ordinary machine with `suspends` in its header.

## Borrow values across a wait

```omega
let count: u64 = suspend Storage::read(&mut storage, &mut buffer);
consume(buffer, count);
```

`buffer` stays borrowed while the read is parked. Some values cannot be held
across a wait at all, such as a lock guard that must be released on the thread
that took it. A type states that with `carry(...)`, and the compiler rejects a
`suspend` while such a value is live. See [carry](../spec/resources/carry.md).

## Cancel a task

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

`Task::request_cancel(&task)` asks. The worker finds out at its next wait, as
ordinary data, and runs its own cleanup path. Nothing is unwound from outside,
and the requester still has to `finish` the task.

## Wait for several events

Give every source a case and wait on one mailbox:

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

## Choose where a task's frames live

The last argument to `start` picks how a task's stack is stored:

```text
Fixed                                  Split

task A: [ whole stack ........ ]       task A: [ kept frames ]--+
task B: [ whole stack ........ ]       task B: [ kept frames ]--+--> worker stack
task C: [ whole stack ........ ]       task C: [ kept frames ]--+    (borrowed while running)
```

```omega
machine serve(request: Request) -> Reply suspends { ... }

let a = TaskRuntime::start<Reply, Request, serve, ActivationStorage::Fixed>(move first);
let b = TaskRuntime::start<Reply, Request, serve, ActivationStorage::Split>(move second);
```

`Fixed` gives each task a whole stack, sized by the compiler from its call
graph, which it keeps while parked. `Split` keeps only what must survive a
wait and borrows the rest from the worker while running, which costs far less
memory when many tasks are parked. `serve` is the same machine either way.

A library that starts tasks for its caller passes the choice through:

```omega
machine accept_loop<const Storage: ActivationStorage>(listener: &mut Listener) suspends {
    ...
    TaskRuntime::start<Reply, Request, serve, Storage>(move request);
}
```

## Many waiting connections, bounded workers

One task per activity is the simplest design and the right one whenever the
tasks fit in memory. When they do not, find what is actually large:

| What is large | Do this |
| --- | --- |
| Nothing | One task per activity. |
| A big buffer few activities use at once | Lease it from a bounded pool around the expensive phase. |
| Call depth needed only between waits | Start the tasks with `ActivationStorage::Split`. |
| The number of waiting activities itself | Keep each one as a record and let a few workers advance them. |

The last row looks like this:

```text
completion event -> queue -> available worker
                              1. update the connection record
                              2. submit its next I/O operation
                              3. return to the queue

waiting connection -> record + buffer + pending I/O claim in an arena
```

A hundred thousand idle connections then cost a hundred thousand records and
32 worker stacks. It is more work to write than a task per connection, so use
it only when you need it. The
[blocking-executor package](../../source/library/blocking-executor/README.md)
has the building blocks.

## Step an activation yourself

A generator is a machine you resume one step at a time. It pauses with the
same `suspend`; there is no `yield`:

```omega
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

The generator is `[linear]`, so a paused producer is never silently dropped.

## Atomics

```omega
data TicketLine {
    next_ticket: AtomicU32;
}

machine take_ticket(line: &mut TicketLine) -> u32 {
    AtomicU32::fetch_add(&line.next_ticket, 1, NoOrdering)
}
```

The last argument is the ordering: `NoOrdering`, `Receive`, `Publish`,
`ReceivePublish` or `GlobalOrder`. See
[atomic contracts](../spec/language/concurrency.md) before designing a
lock-free protocol.

## What the compiler does not prove for you

Memory safety covers parked tasks. It does not prove that something will wake
them. Check for the usual deadlocks: two tasks each waiting on the other, two
paths taking locks in opposite orders, a receive with no reachable sender.
[Protocol proofs](../spec/language/concurrency.md#protocol-proofs) covers how
to state and prove progress when you need it.

## Where next

[Chapter 19](chapter_19_capabilities_effects_boundaries.md) covers where
`TaskRuntime` and other services come from. The complete rules are in
[task runtime](../spec/build/task_runtime.md) and
[effects](../spec/language/effects.md#call-site-acknowledgements).
