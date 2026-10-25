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

The optimized producer built from `a47fa862be36` passed root, all nine places,
and composition through both native and Terminal execution on Linux x86-64.
All nine twins reject for the resident obligation. The row and composition are
pinned under `tests/omega/pass/facts/atomic_bounded_resident`, with twins and
prior-value soundness controls under the corresponding fail tree. A previous
producer admitted the synthetic result placeholder as zero; the repaired
checker retains a runtime observation and refuses self-use before the event.
Serial agreement proves no concurrent theorem. Matching non-Linux hosts remain
unavailable; those executions are not claimed.

This row establishes declaration-owned field-where bounds for store, swap and
decisive exchange and preserves them on a fresh load. Declaration-only atomic
ranges, bounded single-attempt exchange and universal fetch preservation remain
internal implementation work, alongside non-observing exchange, native root
operations, relation/refinement checks and TR3–TR8 concurrent runtime witnesses.
The full TASKS.md outcome remains open.

The representation keeps dedicated atomic identity and resident validity
separate. An atomic replacement carries its own checked resident establishment;
ordinary scalar store/range evidence supplies no atomic authority. A fresh load
may use the declaration bound, never a cached current-resident value. Fetch
requires universal transition preservation; replacement evidence cannot stand
in for that proof. Single-attempt exchange retains failure and Uncommitted custody.
