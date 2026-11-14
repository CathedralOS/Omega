# Owned activations and typed replies

Ratified 2026-11-09 as documentation. Source may hold unique execution custody
over an activation; generators, schedulers and tasks are libraries over it. The
rule lives in
[task activation](../../spec/build/task_runtime.md#owned-activations-and-typed-replies).
No compiler support or fixture outcome is claimed here. Delete this record when
`OWNED-ACTIVATIONS` on [TASKS.md](../../../TASKS.md) lands and the spec section
suffices.

## Context

A parked activation could be resumed only by its runtime. The holder of
`Task<T>` could request cancellation or finish, nothing else. Pull-style
consumption of a producer with nested control flow therefore needed a
hand-written step machine or a task plus a mailbox, with a scheduler in between
and no deterministic stepping. Core iteration was callback-only. The spec's three
owners, execution custody, storage and the task claim, were already distinct;
only the first was unavailable to source.

## Decisions

1. [Storage plans](../../spec/build/task_runtime.md#storage-plans), `suspends`,
   `blocks` and their markers keep their meanings. No `yield` keyword,
   yielding clause or generator-colored machine.
2. An activation is prepared from an exact entry machine and arguments, its
   checked storage plan, which carries the `ActivationStorage` selection, and
   explicitly supplied compatible retained backing. Preparation runs nothing
   and returns what it did not take on failure.
3. Execution custody is a linear claim with one owner. The running state and a
   usable claim never coexist.
4. Stepping runs until return or an actual park and yields the result with
   backing settled, or the suspended activation with its pending protocol. No
   compiler-known yielded outcome.
5. Stepping does not park the caller. Reach includes the child operations that
   may run; blocking and crashes are preserved; storage and loans stay accounted;
   termination needs evidence that the resumption reaches a stop. The lowest
   park always hands control to the execution owner. This is a compiler-
   recognized boundary.
6. A park point retains its occurrence, the live activation, the published
   request and the reply contract. Resuming needs the suspended activation and
   the matching admissible reply; the pairing is checked and a mismatched or
   stale reply returns both.
7. A reply means what its park contract says. A notification to recheck is not a
   completed outcome. An early reply cannot start concurrent execution, and a
   wait protocol cannot lose readiness between check and subscription.
8. Payloads travel through typed endpoints the protocol owns. Receiving yields
   the item and a one-shot capability for that publication. No type is recovered
   from an opaque pause. Schedulers handle pending packages through checked
   callable fields under `some`.
9. A generator is a rendezvous library. `emit` is an ordinary suspending
   operation; the consumer answers continue or stop; stop runs the producer's
   normal cleanup. No suspended activation is discarded; closing may suspend.
10. `Task<T>` stays the observation and settlement claim and grants no execution
    custody. A task facility is activation control, a scheduling policy and a
    completion protocol.
11. Compiler-backed activation declarations and the basic rendezvous and
    generator library live in core. Executors, pools and host wait adapters are
    packages.

## Stages

| Stage | Establishes |
| --- | --- |
| Foundation | Execution custody, stepping, typed endpoints and replies, checked pairing, existing loans and carry preserved, continue-or-stop settlement. Owned or copied items. |
| Lending | A suspension lease handing a reference into paused local storage to the consumer, exposing only the published payload and blocking acknowledgment while reborrows live. |
| Contract precision | A pause-protocol contract naming protocol and scoped endpoint, published and inferable because parking is bounded by held endpoints; per-step progress. |

The last two are independent. Until contract precision lands, a manual generator
interface keeps an other-pending-work outcome and a general `next` may suspend.

## Sketch

Names illustrate the planned core library.

```omega
data Announcement { first: u32; second: u32; }

machine announce(out: &mut Emitter<u32>, job: Announcement) suspends {
    suspend Emitter::emit(out, job.first);
    suspend Emitter::emit(out, job.second);
}

machine drain(generator: Generator<u32>) suspends {
    transition suspend Generator::next(move generator) {
        Next::Item { value, generator } -> handle(value, move generator)
        Next::Finished -> done()
    }
    machine handle(value: u32, generator: Generator<u32>) suspends {
        use_value(value);
        drain(move generator)
    }
    machine done() {}
}
```

## Acceptance

- A copying generator and a round-robin scheduler run as ordinary libraries over
  stepping under `omega run --both`.
- A stale or mismatched reply returns the activation and the reply unchanged.
- A recheck notification cannot stand in for a completed read or release its
  buffer.
- An early reply never produces two executions of one activation.
- Stopping after ten items runs the producer's cleanup and settles its backing;
  dropping a suspended activation rejects.
- A borrowed input slice and a thread-restricted resource held by the producer
  stay enforced across every park.
- `Task<T>` cannot step or resume its activation.
- Tasks rebuilt on the primitive keep their existing documented behavior.
