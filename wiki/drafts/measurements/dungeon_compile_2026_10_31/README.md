# Dungeon compilation, 2026-10-31

On pinned main `15e6cde9b172` (including #1019), std's valid Windows
`ByteRead::Byte` constructor failed checking when a completed u8 was widened
to i32. Preserve its intrinsic source bounds before that conversion erases the
carrier. The repaired baseline is `0dca947540c1`; the pristine producer remains a
separate failed correctness baseline, never a successful timing baseline.

After that prerequisite, clock sampling identified repeated value-ID walks
inside payload history. Shared premises now carry their exact immutable visitor
dependencies and closed/opaque classification. Retirement still checks the current
actual definitions; ambient and payload retirement remain distinct. Local owned
premises, replay, stores, joins, normalization and proof admission keep their
checks. Derived dependencies contribute neither semantic identity nor authority.

The unchanged 3,152-line dungeon compiled faster in every one of five warm pairs
and every one of three cold-source-graph pairs on this cloud machine:

| Median | Repaired baseline | Dependency preparation |
| --- | ---: | ---: |
| Warm wall seconds | 8.9403 | 8.2342 |
| Warm CPU seconds | 8.8813 | 8.1829 |
| Warm peak RSS KiB | 1,225,052 | 1,225,024 |
| Cold source-graph wall seconds | 44.3465 | 42.2431 |
| Cold source-graph CPU seconds | 56.3588 | 55.0670 |
| Cold source-graph peak RSS KiB | 2,050,220 | 2,060,976 |

The ratios of median wall times improve 7.9% warm and 4.7% cold. Taking the
median of each pair's percentage instead gives 12.1% warm and 6.7% cold; these
are different aggregates. Peak memory is essentially unchanged. All raw paired
points, source/producer hashes, controls and collection details are in
[results.json](https://github.com/CathedralOS/Omega/blob/6e5131c510b9e6227b4592652c9a5a39e350a970/wiki/drafts/measurements/dungeon_compile_2026_10_31/results.json). Whole compilation is still above one second.

The initial baseline matrix measured warm 10.263s and cold 31.793s at an earlier
point. Later isolated cold 40.936s therefore required fresh alternating controls;
the unchanged contemporaneous baseline was also slower. Use the paired table
above for the change's effect, rather than comparing runs from different periods.
Historical #1019 EPYC timings are from another machine and are not this baseline.

## Collection

Linux x86_64, Intel Xeon Platinum 8573C, 4 CPU cgroup quota, 16 GiB memory.
Both producers use ordinary release opt-level 3, without per-crate overrides.
The candidate's source is the repaired baseline plus Rust patch SHA256
`266e3fffdfb6911e55f348eb81a9b3ef3175c386e3a11223a56d04f87c598315`.
Builds and package review/preparation are excluded from compilation timing.
The final release build passed in 8m54s; differing Rust cache states make build
wall times unsuitable for a compiler-performance comparison.

For each source version, build with `mbx build --locked --release -p omega`.
Prepare and review the same project through normal `omega update --project
samples/cli/games/dungeon_crawler_cli --target linux_x86_64` before timing.
Compile the original `main.omg` with `--target linux_x86_64 --build-dir <unique>`.
External `perf_counter`/`os.wait4` records wall, user+system CPU and peak RSS;
outputs are captured to files, stdin is EOF. Native compilation alone is timed.
No Rust build or other Omega job runs concurrently.

Warm pairs alternate AB/BA with one unmeasured warmup before every timed compile.
Each producer uses its own normally populated task cache: the 269,879,856-byte
active graphs would evict one another under a shared 512MiB graph budget.
Normal same-project package refresh stays outside the timer; locks and graph
records are never copied between versions. Cold controls preserve/move only the
exact own active graph files and retain resolver state. OS page cache is uncontrolled.

Phase profiling is separate: on the repaired baseline three runs per mode
measured 22.2% extra wall time relative to coarse timers. The observed -2.3% coarse
timer delta is ordinary variability. A successful `gprofng collect app -p on`
clock profile found payload history at 3.893s inclusive of 8.796s sampled CPU,
including repeated 0.43-0.45s value-ID walker callsites. It changed no host
permissions or kernel settings. Inclusive scopes overlap and are not summed.
A contiguous-row storage trial was neutral (median paired change -0.07%) and
was reverted; its points are retained in the JSON to preserve attribution.

## Behavior and scope

- The final optimized producer passes the 13 focused payload history/predicate
  tests, including mixed opaque/value dependencies, mutation, instance retirement,
  immutable snapshot preservation, joins and equal-observation normalization.
- [The completed-carrier fact row](../../../../samples/cli/facts/completed_integer_carrier_bounds/README.md)
  passes all nine native/Terminal positions at zero and all nine reject twins.
  Source and permanent corpus fixture bytes match. All positives bind four hosted
  targets; execution evidence here is Linux x86_64 only.
- EOF and help/exit each exit zero in native and Terminal. The complete four
  stdout files match the authored 463-byte and 536-byte oracles exactly.
- Constructor, signed/unsigned widening, generic/bare predicates and a saved
  snapshot surviving replacement also pass native/Terminal controls.
- The frozen pristine producer rejects the exact new parameter witness with
  the original default-domain proof failure. No existing golden is rewritten.

This is a scoped compiler iteration with preserved proof, custody and output
checks. It is not a whole-corpus, release or four-host execution qualification.
The one-second target remains open. Source-graph recovery and native realization
remain material costs in the captured profiles.
