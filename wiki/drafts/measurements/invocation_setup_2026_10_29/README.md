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
