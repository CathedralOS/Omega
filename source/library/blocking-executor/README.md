# Bounded workers and persistent connection state

This package declares bounded submission queues, moved-custody admission and
linear completion claims. Executable queue bodies, worker loops and provider
integration remain implementation work under `COMPONENT-SUBSTRATE` in
[TASKS.md](../../../TASKS.md). The declarations alone do not demonstrate an
operating executor or server.

The intended server arrangement keeps connection state and buffers in an arena
and processes events through a bounded set of worker activations. Workers record
progress and return to the queue while I/O is pending. Queue capacity bounds
waiting submissions; worker count separately bounds simultaneous execution
stacks. A worker that suspends retains its stack. See the
[concurrency guide](../../../wiki/language_guide/chapter_18_concurrency.md#many-waiting-connections-bounded-workers)
for the authoring tradeoff and storage obligations.

Complete this path with executable queue/worker bodies, a completion-based I/O
provider and one working server. Acceptance must exercise:

- Queue-full rejection returning the submission and all its custody.
- A separately enforced worker-count bound, with connection state persisting
  between worker calls instead of retaining a parked task per connection.
- A borrowed-buffer read raced against a timeout, including cancellation and
  late completion. Buffer reuse requires retirement of every outstanding
  external access; a cancellation request or unrelated worker settlement is
  insufficient.

The provider must support retained storage through the ordinary
[foreign-storage contract](../../../wiki/spec/build/foreign_storage.md), and
workers must satisfy the [task stack guarantee](../../../wiki/spec/build/task_runtime.md#activation-description-and-stack).
This path requires no new task representation or source keyword.
