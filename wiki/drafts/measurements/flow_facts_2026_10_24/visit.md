# Product-check state visits, 2026-10-24

The per-pass baseline copy is not the cost. Building one state's flow fact is
the larger share of the recorded thread time, and each parallel group also
clones the whole baseline plan once.

Same three workloads and the same two-check split as
[the repetition record](README.md). One run each. Scopes were temporary and
have been removed. Times below are the product check. `build_flow_facts` wall
stays 237–256 ms, in line with the 232–240 ms measured before these inner
scopes. The inner rows are summed across worker threads, so they overlap one
another and the parent wall. They are not a second clock to add to the compile.

| Workload | Flow wall | State facts | Visits | Group baseline clones | Group `clone_from` | Pass `clone_from` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| length | 256 ms | 546 ms / 339 | 339 | 374 ms / 51 | 126 ms / 51 | 0 ms / 2 |
| dynamic dispatch | 240 ms | 531 ms / 339 | 339 | 311 ms / 52 | 103 ms / 52 | 0 ms / 2 |
| nested record | 237 ms | 541 ms / 340 | 340 | 378 ms / 51 | 144 ms / 51 | 0 ms / 2 |

The owner-side baseline clone rounds to 0 ms. The copy inside the fixed-point
loop also rounds to 0 ms; it runs twice on the product check. The copies that
show up are the per-group ones: about 51 groups each clone a baseline and then
`clone_from` it before sweeping. On dynamic dispatch that is 414 ms of summed
thread time beside 531 ms inside `build_state_flow_fact`. A state visit records
about 1.6 ms of thread time. The Build check stays the small one: 89 visits,
27–28 ms of state facts, and 15–17 ms of flow wall.

## Next question

Which part of `build_state_flow_fact` spends that 1.6 ms: entry-context
rebasing, the statement walk, or exit facts. The per-group baseline clone is
the other candidate, and only if a group can sweep without its own full copy.
