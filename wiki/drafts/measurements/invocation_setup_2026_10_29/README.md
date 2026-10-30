# Invocation setup attribution

After the [mixed ancestry screen](../windows_ancestry_postqueue_2026_10_29/README.md),
identify large remaining serial work before changing a backend subscan. The
fixed compiler remains the qualified post-queue O3 producer `808ee3ff1725`,
source `19554817237`; local compiler/library inputs match it after restoring
the ancestry candidate. This is attribution, not a new speed claim.

Six version-only processes retained the first visit separately. The five warm
observations had median 103.5 ms process wall, 78.1 ms kernel CPU and 15.6 ms
user CPU. Previous baseline complete compiles spent median 137.7–168.0 ms
outside their CLI interval. Startup therefore explains only part of that gap
and does not establish enough removable work for a repeatable 20% whole-compile
win. The PE has 122.7 MB text, 24.0 MB read-only data and 5.0 MB exception data;
size alone does not establish load cost or justify a new build configuration.

Research, 2026-10-29: exact query `site.github.com/rust-lang/rust library std
sys pal windows thread CreateThread STACK_SIZE_PARAM_IS_A_RESERVATION`.
Read the actual [Rust Windows thread implementation](https://github.com/rust-lang/rust/blob/main/library/std/src/sys/thread/windows.rs)
and [Microsoft CreateThread contract](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createthread).
Rust passes the reservation flag; this refutes inferring that 256 MiB requested
stack size is itself committed on every spawn. This current upstream source
is explanatory context, not an exact pinned-toolchain runtime observation.
The old `sys/pal/windows/thread.rs` raw path returned 404. No stack reduction,
host tuning, process termination or global profiler session was attempted.

The next bounded diagnostic splits existing application setup into resource
creation, retained compiler storage and source acquisition/resolution, and
splits graph-storage custody from selected-library identity checks. Use existing
phase profiling, one frozen optimized diagnostic producer, unchanged images and
native oracles. Archive and remove the diagnostic patch before any A/B candidate.
Nested timers are attribution children, not additional elapsed work. Stop a
direction if its removable complete-invocation cost cannot support the target.

[Startup evidence](startup.json.gz) includes exact producer/source inputs,
all observations and logs. No Rust change or speed improvement is established
by this startup diagnostic.

## Finer attribution

Frozen timer-only source `57c4d3c1489` passed six unchanged image/native checks
(three first visits, three warm profiles). Across the warm profiles, three
compiler worker bodies took 200–590 ms; their enclosing spawn/join scopes
added only 0.9–1.9 ms. Do not implement a worker-lifetime optimization from this
evidence. Session resource-grant creation was also only 0.8–1.3 ms.

Compiler storage preparation took 74.0–77.4 ms, graph-lane construction
15.8–18.5 ms, and acquisition's optional cache-coordinate setup 93.7–129.3 ms.
Library identity and graph custody are separate children, not extra time to
add to their parents. [Scope values](diagnostic-summary.json) and
[exact diagnostic patch, optimized producer receipt and raw observations](diagnostic.json.gz)
retain the attribution. The four temporary Rust timer edits were removed.

Source tracing identifies repeated resolver verification of four sibling roots:
the resolver root and Git, workspace and external-local lanes. Each currently
reopens the same ancestry independently. The next falsifiable experiment is a
Windows sibling-custody walk: retain and freshly verify the parent, classify and
open each direct child without following links, reconcile every classified/opened/
retained identity, and freshly reconcile the parent again. This shares only the
common prefix within one operation, never a validity cache between operations.
Preserve all four roots' refusal behavior, rather than dropping checks for unused
lanes. It changes a different repeated-work domain from the parked per-path
linear-ancestry experiment and must stand on its own A/B evidence.
