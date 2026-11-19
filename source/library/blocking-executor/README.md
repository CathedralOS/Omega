# Bounded workers and persistent connection state

This package declares bounded submission queues, moved-custody admission and
linear completion claims. Executable queue bodies, worker loops and provider
integration are still open under `COMPONENT-SUBSTRATE` in
[TASKS.md](../../../TASKS.md). The declarations alone show no working
executor or server.

The intended server keeps connection state and buffers in an arena and handles
events with a bounded set of worker activations. Workers record progress and
return to the queue while I/O is pending. Queue capacity bounds waiting
submissions; worker count separately bounds simultaneous execution stacks. A
suspended worker keeps its stack. See the
[concurrency guide](../../../wiki/language_guide/chapter_18_concurrency.md#many-waiting-connections-bounded-workers)
for the authoring tradeoff and storage obligations.

Finish with executable queue and worker bodies, a completion-based I/O
provider and one working server. The tests must cover:

- A full queue rejecting a submission and returning all its custody.
- A worker-count bound enforced separately, with connection state kept
  between worker calls instead of a parked task per connection.
- A borrowed-buffer read raced against a timeout, including cancellation and
  late completion. Reusing a buffer requires retiring every
  outstanding external access; a cancellation request or unrelated worker
  settlement is not enough.

The provider must support retained storage through the ordinary
[foreign-storage contract](../../../wiki/spec/build/foreign_storage.md), and
workers must meet the [task stack guarantee](../../../wiki/spec/build/task_runtime.md#activation-description-and-stack).
This needs no new task representation or source keyword.
