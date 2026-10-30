# Repeated integer premise preparation

2026-10-29 (America/Los_Angeles). User-assigned ITERATION-PERFORMANCE continuation,
fixed base `88a0a52d5ea0a7d86df92724e31299ce7e4ccf5f` in the retained
contract-query-publication worktree. Compiler inputs are unchanged from the
published operand-resolution producer, SHA256
`1c7645a93c087116c69bb5951cc940709cdadecfeae847d38f9de957d3ad4d60`.
The previous [body-unfolding iteration](../body_unfolding_2026_10_29/README.md)
identified scalar guard integer observation preparation as dominant, but rejected
query-local operand reuse below its predefined 10% threshold. That patch is absent.

## Question and acceptance before editing

Split integer query time into goal projection, premise projection, cloning and
admission. Count projected rows, operations and fixed-point operation visits.
These counts describe work, not unique immutable contexts or cache hit rates.
If a small safe mechanism is identified, use the same unchanged operand-order
first-update workload, fresh independent package caches and A-B-B-A ordering,
profiling disabled. Retain only a mean reduction of at least 10% with separated
ranges, identical native images, actual native/Terminal agreement and focused
formation/storage/refusal controls. Preserve all valid observations. Stop the
bounded iteration at a supported decision; do not collect unrelated small wins.

## Research

Changed question: can shared solver premises reduce repeated guard preparation
without allowing a goal to establish its own formation? Query:
`site.microsoft.com z3 incremental solving assumptions push pop reuse assertions performance`.
Read sections 4.1-4.3 of [Programming Z3](https://z3prover.github.io/papers/programmingz3).
It separates retained assertions from scoped or assumed assertions; the default
solver may change strategy when moving from one-shot to incremental queries.
This suggests investigating reusable premise preparation, not substituting Z3 or
assuming reuse is always faster. Omega's integer observations also record child
operation formation and captured-value identity, which must remain independent
of the requested goal. Its own prepared-premise path already has conservative
fallback conditions; a clone of that engine is not automatically equivalent to
the general goal-first route.

Source review: `integer_predicate_observations` projects the goal first, then
premises and carrier facts, admits operations, and retains only admitted EQ/NE
comparisons for complement reasoning. `prepare_integer_premises` instead requires
complete projection and independent admission before clearing its operation list.
The existing prepared cache has a limited key intended for its current consumers;
do not extend it to active unfolding contexts without exact validity evidence.
The diagnostic changes observation scopes/counters only and preserves the route.

Coordination status read and the existing ticket
`1a92afc0764b427fa2e71bf095de2b2b` was present. Its continuation-note update timed
out during push and may have published; no successful update is asserted and it
was not retried. This file records the current scope. Other contract-entailment work remains isolated
in its own worktree. Use this worktree's Cargo target and the shared build-slot
hold; queue wait is not compiler time.
