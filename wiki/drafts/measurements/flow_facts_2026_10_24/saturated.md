# Product-check flow pass, saturated, 2026-10-24

The product-check flow wall is the checking work. The per-group baseline copy
is off that wall. This closes the two cuts named in
[the visit record](visit.md).

Same host and workloads as the
[whole-compile profile](../resolution_boundary_2026_10_24/README.md): Windows 11,
AMD64 Family 25 Model 97 Stepping 2, 32 logical CPUs, dev profile `opt-level = 1`.
Source base `8a3aa29fa568f93c15e9f47f36a616b65dee9ca3`. One run each. Temporary
timers, since removed, summed worker time with atomics and kept the slowest
group's `clone_from` and sweep on their own clocks. Native publication
succeeded and each program exited 0. Summed thread times overlap the wall;
only `max group` is the critical path.

The second check is the product check (~103 machines, 339–340 state visits,
~1,075 statements, ~5,200 expression nodes, 468–469 calls). The first check
is the Build check and stays small: its slowest group is 12–14 ms.

| Workload | Slowest group | That group's copy | That group's sweep | Copy, all groups | Sweep, all groups |
| --- | ---: | ---: | ---: | ---: | ---: |
| length | 242 ms | 1 ms | 240 ms | 423 ms | 579 ms |
| dynamic dispatch | 222 ms | 1 ms | 220 ms | 440 ms | 532 ms |
| nested record | 213 ms | 0 ms | 212 ms | 376 ms | 514 ms |

About 51 groups copy the baseline. Together those copies are ~400 ms of thread
time, and they do not sit on the slowest group. Removing the copy would not
move this ~220 ms wall. The earlier pass `clone_from`, measured in the visit
record, remains two calls that round to 0 ms.

The sweep accounts as follows. Thread milliseconds, product check. Transition
and expression rows sit inside the call/transition chunk. Storage writes and
call facts sit inside expression evaluation, so they are already inside
`eval exclusive`.

| Workload | Entry | Expression eval, exclusive | of which storage writes | of which call facts | Result facts | Statement rest | Exit |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| length | 4 | 308 | 153 | 23 | 10 | 88 | 8 |
| dynamic dispatch | 5 | 285 | 137 | 21 | 11 | 79 | 7 |
| nested record | 4 | 270 | 133 | 19 | 12 | 79 | 9 |

Entry context and exit facts are not the wall. `build_call_flow_fact` is about
20 ms of thread time across ~468 calls. The expression walk's own node work,
after subtracting the storage-write and call-fact time it contains, is about
120–130 ms of thread time across ~5,200 nodes. Statement rest (loans, the
write, transfer, and invalidation after the calls) is about 80 ms.

`call_storage_writes` is the largest single helper inside the walk, about
130–150 ms. It is already memoized per call occurrence on the flow build: a
later sweep clones the retained `Rc` instead of rebuilding the alias closure.
That cost is the first closure, not a repeated semantic copy.

## Decision

Rejected further cuts on this flow pass. The ~220 ms product-check wall is one
group sweeping expression evaluation, memoized call storage footprints, and
the statement's loan and transfer work. The baseline copy that looked large in
summed thread time is 0–1 ms on that group. No separate compile-time win was
implemented. The rest of Stage 05, including std/core machine checking, is
outside this pass.
