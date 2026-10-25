# Bounded atomic residents

Owning outcome: **ATOMIC-MEMORY-MODEL** in TASKS.md; the full item remains open.

Fact: **A shared atomic store preserves its resident bound and ordering, and a load carries that bound into Exact subtraction.**
Every place copies this sentence, preserves the bound 100 and stored value 41,
and consumes the observation in `100 - observed` before checking the runtime
oracle 59. Call and field declarations must prove their receiving restrictions;
no caller guard re-establishes the bound. Each reject twin changes only the
store helper's `requires value <= 100` to `requires value <= 101`.

The validated starting candidate `df88b7f60a952df5754c3e95992a8349a3893348`
rejects the local store despite its sufficient precondition, and cannot prove
the consuming subtraction. The preserved optimized producer at `666e562d988`
has identical Rust/library/manifests/toolchain inputs to that candidate; this
is not a claim that it represents combined main integration `688d4541f376`.

Acceptance is pending until all nine places and their twins run through the
completed producer and are pinned under tests/omega. Linux x86-64 is the
available execution host. Serial agreement proves no concurrent theorem.

The representation keeps dedicated atomic identity and resident validity
separate. An atomic replacement carries its own checked resident establishment;
ordinary scalar store/range evidence supplies no atomic authority. A fresh load
may use the declaration bound, never a cached current-resident value. Fetch
requires universal transition preservation; replacement evidence cannot stand
in for that proof. Single-attempt exchange retains failure and Uncommitted custody.
