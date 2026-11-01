# Std-importing check time, 2026-11-01

A cold `omega --check` of a program that imports `omega_language_std` took
285 s at main `35959703a70` on the Windows x86-64 development host (the
corpus golden says 12 s) and timed out at the pilot's 120 s cap on hosted
runners. Two repairs by other sessions removed most of that while this was
being measured: #1170 and #1161. Main `0f75ac52152` checks the pilot's
target fixture, `tests/omega/pass/expressions/declared_operator_match_result`,
in 44 s locally. The first change brings it to 34 s and records how to measure
the regime that hid the defect. The [second round](#second-round-integer-bound-rows-by-root)
brings it to 29 s.

Local host: Windows x86-64, optimized dev producers (opt-level 1), 32
logical CPUs shared with other sessions' builds.

## What hid it

A warm compile cache replays std's check: the same 12-line program takes 6.6 s
once the compiled-review cache holds std, so repeated local runs never showed
the cost. A hosted runner and every freshly rebuilt compiler start cold. Cold
observations use a fresh `LOCALAPPDATA` (Linux: `XDG_CACHE_HOME`) per run and
no phase profile. The cold check writes one 527 MB `source-graphs` record.

Phase-profile rows are inclusive wall time on the recording thread, and rows
inside the parallel checking workers add across threads and include time a
worker spends running stolen tasks. The `05.num.*` rows (60 to 90 s each) and
`05.d.7.3b.1.cap.2d.*` rows (up to 224 s) are such sums, not wall time. Wall
time lives in the serial rows: `prepare_typed_trees` (145 to 218 s before the
repairs), `lower_typed_trees` and terminal selection. An earlier attribution of
the cost to numeric observation read those sums as wall time.

## Cause

`resolve_named_result_overloads` runs once per specialization round and once
more while settling destinations: 13 calls on the std console fixture. Its
`predicate_type_contexts` pass observes the state prefix of every expression of
a state whose types carry a predicate (std's `ByteRead` sum has a `requires`)
through `observe_runtime_state_prefix`. On a bare `TypedTrees` that call has no
memos, so each of the 128 observations rebuilt the program-fixed tables
(private-arrival index, call-frame tables, proof-only classification), and each
sub-query inside one walk rebuilt them again. A 20-line std-free program with
the same shape paid 1.4 s per site; std's nested `byte` machine paid 12 s per
site. The 128 observations took 188 to 210 s of the 213 s preparation stage and,
through the checking phases that shared the cold tables, most of the thread-
summed numeric rows.

Main now carries both repairs: #1170 runs the two type-context batches through
one `CheckingContext` over the immutable program, and #1161 rules out every
non-machine binder in `machine_parameter_signature` before walking machine
scopes.

## This change

Sampling the repaired compiler cold (a debug-symbol build, a stack sampler on
the compile thread, 10 ms ticks) showed no single cost left, but several
queries that walk a whole table or roster once per ask. Each is replaced by a
structure that answers the same question.

| Query | Walk | Replacement | Share of compile-thread samples |
| --- | --- | --- | --- |
| each machine's type-parameter references | every type-reference row, per machine (2,400 machines) | one pass indexing the references of every type binder | 13 type-context calls 2.17 s to 0.33 s; case pass 677 ms to 3.9 ms |
| first-occurrence rosters past a few dozen entries | `push_unique`, a `contains` scan per append | `UniqueRoster`, comparing only entries of equal hash | about 7% |
| the machine storing a state | every machine's state list, when the retained parent misses | the state-position index that `machine_states_named` keeps | about 4% |
| shared and mutable slice views of an element | every type-reference row, twice per lowered collection type | `SliceViewIndex`, extended by appended rows and rebuilt when the arena's append epoch changes | 5.1% |
| the lowering catalogs of a callable | four `to_vec` clones per callable | borrowed for the catalogs' lifetime | 4.7% |

Phase scopes now split the specialization loop, the settle pass and every step
of the overload resolver, so a profile names these costs without new code.

## Results

### Local, cold, unprofiled

Fixture `tests/omega/pass/expressions/declared_operator_match_result`. Each
round alternates the producers; wall and CPU seconds.

| Round | Producer | Wall runs | Mean wall | Mean CPU |
| --- | --- | --- | ---: | ---: |
| 1 | main `0f75ac52152` | 45.9, 44.0, 42.2 | 44.0 | 71.4 |
| 1 | first commit alone | 40.5, 40.3, 37.2 | 39.3 | 68.4 |
| 1 | both commits without the slice-view index | 36.1, 36.0, 33.3 | 35.1 | 63.0 |
| 2 | main | 43.9, 41.9, 43.3 | 43.0 | 71.3 |
| 2 | both commits without the slice-view index | 35.6, 34.9, 33.8 | 34.8 | 62.7 |
| 2 | both commits | 38.1, 33.5, 31.7 | 34.4 | 60.2 |
| 3 | main | 46.6, 44.0, 45.8 | 45.5 | 70.2 |
| 3 | both commits | 34.0, 33.4, 34.2 | 33.9 | 61.6 |

Over the six runs of rounds 2 and 3, main averaged 44.3 s wall and 70.8 s CPU
and both commits 34.2 s and 60.9 s: 23% less wall and 14% less CPU. Run to
run spread on this host is 3 to 6 s because other sessions build alongside.

### Phase rows

Cold, `OMEGA_PHASE_PROFILE=1`, milliseconds. "Sum" rows add across threads.

Regression and repair, `tests/omega/pass/arithmetic/call_surviving_return_exit`:

| Row | main `35959703a70` | first commit on main `0f75ac52152` |
| --- | ---: | ---: |
| `05.c.prepare_typed_trees` | 218,700 | 2,360 |
| `05.c.3.b.settle_value_destinations` | 74,800 | 154 |
| `05.d.lower_typed_trees` | 36,400 | 26,700 |
| `05.d.7.3b.1.cap.2d.local_init` (sum) | 224,300 | 3,000 |
| `05.num.2a.3_collect_assoc` (sum) | 87,900 | 75 |
| `05.num.assoc.2_build_preds` (sum) | 79,500 | 52 |
| `05.num.2_observe_prefixes` (sum) | 92,700 | 60,900 |
| `05.overloads.4_predicate_type_contexts` | 144,500 | 330 |

This change, on `declared_operator_match_result`:

| Row | main `0f75ac52152` | both commits |
| --- | ---: | ---: |
| `05.c.prepare_typed_trees` | 5,528 | 2,070 |
| `05.c.2.specialization_loop` | 3,953 | 1,413 |
| `05.c.3.b.settle_value_destinations` | 1,058 | 167 |
| `05.d.lower_typed_trees` | 27,915 | 21,273 |
| `05.d.3.build_check_facts` (sum) | 15,375 | 12,405 |
| `05.d.7.check_checked_facts` (sum) | 9,175 | 5,789 |
| `05.lower_terminal_selection` | 5,897 | 3,124 |
| `pre_resolution.constant_initializers` | 2,209 | 804 |
| `psi.source.prepare` (sum) | 6,161 | 4,135 |
| `05.num.2_observe_prefixes` (sum) | 86,169 | 73,549 |
| `05.num.2a.3_collect_assoc` (sum) | 84 | 75 |
| `05.num.assoc.2_build_preds` (sum) | 62 | 56 |

### Hosted, same job

Each row is one dispatch of the pilot on a throwaway ref holding the final
code plus one workflow step (`work/probe-fin-*`). The step downloads main
`0f75ac52152`'s retained producer from an earlier pilot run and alternates it
with the head's producer, three cold direct `omega --check` runs each on
`pass/expressions/declared_operator_match_result`, so both producers see one
machine. Wall seconds; the last column is the pilot's own check leg for the
head, which also realizes the fixture and runs a second production for the
host target.

| Run | Runner CPU | Main, direct | Head, direct | Head / main | Pilot check leg, head |
| --- | --- | --- | --- | ---: | ---: |
| Linux 41307996898 | EPYC 9V45 | 37.1, 37.5, 36.1 | 29.1, 28.1, 27.6 | 0.77 | 34.7 |
| Linux 41308004475 | EPYC 7763 | 61.3, 61.4, 61.5 | 47.8, 47.2, 46.8 | 0.77 | 58.8 |
| Linux 41308010855 | EPYC 7763 | 62.0, 62.9, 62.9 | 48.4, 47.9, 48.2 | 0.77 | 59.4 |
| Windows 41308000938 | EPYC 7763 | 67.0, 67.9, 68.2 | 54.0, 53.7, 54.2 | 0.80 | 62.6 |
| Windows 41308007912 | Xeon 8370C | 74.3, 74.0, 73.2 | 61.4, 60.8, 60.7 | 0.83 | 62.6 |
| Windows 41308014110 | EPYC 7763 | 68.0, 69.4, 68.6 | 55.9, 54.9, 53.9 | 0.80 | 60.2 |

On the same machine the head's direct check takes 77% (Linux) and 80 to 83%
(Windows) of main's wall time, and 82% of its user CPU on Linux (the Windows
runs report none). The pilot's check leg is below 60 s on 3 of 6 runs, with a
Linux median of 58.8 s and a Windows median of 62.6 s. The three Windows runs
are all above 59 s, on the slower runner CPUs (Xeon 8370C, EPYC 7763); the
head without its slice-view index took 47.0 s for the same leg on an EPYC
9V74. The 60 s target is met on Linux and not met on Windows runners with a
slow CPU.

### Output equivalence

`omega --check` output (exit code and every diagnostic) is byte-identical
between main `0f75ac52152` and the final producer on 191 type-context
witnesses (83 exit 0, 108 exit 1) and on 64 std-importing fixtures. Unit
tests of `typed-trees`, `typed-trees-to-checked-trees`,
`checked-trees-to-lowered-psi` and `validation`: 173 run, 169 pass. The four
failures, three `source_guards` tests and `alias_obligations_survive_copy_
actual_substitution_and_complete_applied_views`, fail the same way on main
`0f75ac52152`.

Hosted pilot, 54 std-importing fixtures (33 pass, 21 fail), main (run
41308286683) against head (run 41308283898), Linux: 53 of 54 have the same
status, and both runs report the same four structural moves from the stored
golden (`pass/generics/declared_range_endpoint_inference` rejected, three fail
fixtures with moved diagnostics). The one difference is
`pass/providers/checked_boundary_operator_physical_custody`, which took 106 s
on main against the pilot's 120 s fixture cap in the 54-fixture run and timed
out in the head's run, where four fixtures common to both runs took 17 to 37%
longer. Run alone, it timed out on main (run 41311732191) and was `checked` in
92 s on the head (run 41311729054).

## Not retained

Five experiments were measured and dropped.

- Reading `OMEGA_STRUCT_TRACE` and `OMEGA_COMPILER_CHECK_SITES` at 57 sites
  costs a lock and an environment scan per pass; the stack sample put 38 of
  2,579 samples (1.5%) in `getenv`, 30 under `CaseEvidence::has_value`.
  Caching the switches once per process measured 33.0 s wall and 63.7 s CPU
  against 33.9 s and 61.6 s (three interleaved runs), and 37.2 s against 35.4 s
  in a second, noisier round. It touches 40 files and shows no gain.
- Starting the largest flow group first. With four rayon threads, the width
  of a hosted runner, it measured 37.1 s against 35.2 s (three runs each).
- mimalloc purge and commit settings through the environment
  (`MIMALLOC_PURGE_DELAY=-1`, `MIMALLOC_ARENA_EAGER_COMMIT=1`): 31.2 to 37.3 s
  for all three settings, inside the spread.
- `merge_rosters` sorts its whole roster to add one entry (2.9% of the
  runner leg's samples). A cheaper insert needs the roster known to be
  normalized, and an in-place rewrite in `continuation_identities.rs` breaks
  that; it needs a flag on the roster type.
- A linear-time `arena::unique_handles_in_order` (one hash-set pass instead of
  two sorts) left the `windows_removal_walk` group at 5.5 s with and without it,
  so the sorts are not what that group spends.

## Remaining time

The check is mostly serial. With `RAYON_NUM_THREADS=1` a hosted check of the
head without its slice-view index takes 58.1 s (Linux, EPYC 7763) and 53.6 s
(Windows, EPYC 9V74) against 47.4 s and 44.3 s with four threads, so about 25%
of the CPU work runs in parallel. Four threads and 32 threads measured the same
wall time locally.

In the sample taken before the slice-view index, the compile thread's samples
split as: stage 04 checking 32% (check facts 14%, checked-fact checks 8%),
package source preparation 30% (pre-resolution constant evaluation 19%),
terminal production 19%, typed preparation 11%.

One flow group costs 7.5 s of a 28 s cold check. The group is std's
`windows_removal_walk` (16 machines, a 4096-element `frames` array whose
element type has a domain-qualified handle field), three passes of 2.3, 2.4
and 2.8 s: a complete pass, a partial pass over two dirty states that costs as
much as a complete one, and the confirming complete pass. Other groups of 14
machines take 0.15 s. Its 227 `retained` calls (126 distinct context and point
pairs) scan 5.2 million facts, about 23,000 per call, and 5,194,376 of them
are `DomainMembership` facts on `frames`: the per-element rows that
`semantic/facts/field_domains.rs` seeds so that each fixed-array element
spells its call-boundary obligation. `windows_removal_unwind` (1.1 s) is the
same, 835,637 of 836,831 facts. Every query over a context's facts therefore
walks the array's rows, whatever place it asks about.
`configure_windows_filesystem` (0.7 s) scans 2.2 million `AssignedScalarValue`
facts over 1,144 calls, and `UefiX86_64::plan` (0.9 s) does not scan large
contexts. The group is one task on one worker, so it bounds the fan-out's wall
time. A std-free
machine chain over `[Frame; N]` whose element has no domain-qualified field
did not reproduce the cost (N of 256, 1,024 and 4,096 checked in the same
time). Candidate levers are a per-root index over a context's facts, so that a
query about `depth` skips the array's rows, and a memo for the repeated
`retained` keys. Collapsing the per-element rows of a large array changes what
a write to one element retires, which makes it a design question. The first
round takes none of these; the second round below skips building the rows of
other roots, and leaves the scan and the memo open.

The corpus runner's check leg realizes the fixture for its check target
(`macos_arm64` here) and runs a second production for the host target, as its
source and a stack sample of its four compile threads show. The pilot leg runs
6.6 to 11.5 s above a direct check on Linux and 1.7 to 8.6 s on Windows. Each
std-importing fixture in the runner also checks std cold: the 54 fixtures took
1,219 s on main at two jobs.

## Second round: integer-bound rows by root

Fixed base: main `74462fcd188`, the first round merged. It checks the target
fixture cold in 31.3 s on the development host.

### Cause

`CurrentIntegerBounds::new` called `retained` eagerly. `retained` scans every
fact of the active contexts and builds a `BoundPredicate` (an expression clone
and a canonical place per binding) for each domain-membership fact, then the
object answered queries about a handful of scalars. A fixed array declares one
membership fact per element and context, so `windows_removal_walk` rebuilt
about 23,000 rows per call to answer queries about `depth`, `restart` and
`handle`. A domain predicate binds its subject and the places below it, so for
a place under another root its consequence is the carrier and changes no bound.
`payloads_at_place` converted the place of every `Assigned*` fact, an
allocation, before testing that its root equals the subject's.

### Change

- `retained_at_root` returns the rows `retained` returns, less the
  domain-membership rows whose subject has another root.
- `CurrentIntegerBounds` builds the rows for a root on the first query about
  that root and keeps them for the object's life. The query methods take
  `&mut self`; an object that is never asked builds none.
- `payloads_at_place` compares roots before converting the fact's place.

### Results

Local, cold, unprofiled, four alternating pairs; wall and CPU seconds.

| Producer | Wall runs | Mean wall | Mean CPU |
| --- | --- | ---: | ---: |
| main `74462fcd188` | 31.2, 31.6, 31.7, 30.6 | 31.3 | 68.2 |
| head | 28.7, 29.4, 29.7, 29.0 | 29.2 | 64.1 |

The same code with the cache in a `RefCell` measured 29.2 s against 31.0 s
(four pairs). Per flow group, from a build with a temporary timing guard around
each group (not part of the change), milliseconds, three runs each:

| Group | main | head |
| --- | --- | --- |
| `windows_removal_walk` | 7805, 7544, 7146 | 5644, 5456, 5475 |
| `windows_removal_unwind` | about 1,100 | about 750 |
| `configure_windows_filesystem` | about 620 | about 400 |

Hosted, same job, as in the first round, with main `74462fcd188`'s retained
producer (run 41318768464 Linux, 41318773113 Windows) as the baseline. These
heads keep the cache in a `RefCell`; the committed form differs only in that
container.

| Run | Runner CPU | Main, direct | Head, direct | Head / main | Pilot check leg, head |
| --- | --- | --- | --- | ---: | ---: |
| Linux 41323813697 | EPYC 7763 | 47.7, 47.7, 47.7 | 45.3, 44.1, 44.0 | 0.93 | 56.0 |
| Linux 41323821572 | EPYC 7763 | 48.1, 47.1, 46.6 | 43.6, 43.6, 43.7 | 0.92 | 55.9 |
| Linux 41323828897 | EPYC 7763 | 48.2, 48.0, 47.4 | 45.2, 44.1, 44.3 | 0.93 | 56.1 |
| Windows 41323817564 | EPYC 7763 | 50.4, 50.0, 49.9 | 46.5, 46.6, 48.0 | 0.94 | 57.7 |
| Windows 41323825400 | EPYC 7763 | 50.0, 48.7, 49.3 | 45.8, 45.3, 45.2 | 0.92 | 57.4 |
| Windows 41323832889 | Xeon Platinum 8573C | 45.3, 43.7, 43.7 | 41.5, 40.1, 40.7 | 0.92 | 50.6 |

On the same machine the head's direct check takes 92 to 94% of main's wall
time. The pilot's check leg is 56.0, 55.9 and 56.1 s on Linux (median 56.0 s;
58.8 and 59.4 s on this CPU after the first round) and 57.7, 57.4 and 50.6 s on
Windows (median 57.4 s; 62.6 and 60.2 s on the EPYC 7763 after the first round).
The target is met on every measured runner: 2.3 s of margin on the Windows
EPYC 7763, 4.0 s on the Linux one. The Xeon 8370C that gave the slowest
Windows leg after the first round (62.6 s) did not appear in this round, so
its leg is not measured. The pilot's second leg (native and Terminal
execution) for the same fixture took 55.6, 55.3 and 56.0 s on Linux and 58.7,
56.5 and 50.1 s on Windows.

### Output equivalence

`omega --check` output (exit code and every diagnostic) is byte-identical
between main `74462fcd188` and the head on 190 witnesses chosen for integer
bounds, indexing, domains and predicates (49 exit 0, 141 exit 1) and on 64
std-importing fixtures (all exit 1 on both sides). Unit tests of
`typed-trees-to-checked-trees`: 28 run, 28 pass.

Hosted pilot, 54 std-importing fixtures (33 pass, 21 fail), Linux, main
`74462fcd188` (run 41328253795) against the head (run 41328249104): 53 of 54
have the same status, and both runs report the same four structural moves from
the stored golden with identical diagnostics. The 54th,
`pass/providers/checked_boundary_operator_physical_custody`, took 100.3 s of
the pilot's 120 s fixture cap on main and timed out in the head's run. The
head's run was on a slower machine: the 39 fixtures that both runs report a
time for took 1.08 to 1.67 times as long (mean 1.44), which puts main's 100 s
over the cap. Cold on the development host, alternating, the fixture took
25.2, 26.2 and 25.9 s on the head against 27.3, 27.2 and 28.9 s on main. It
timed out in the first round's 54-fixture comparison on the head as well, and
in its single-fixture runs main timed out while the head was `checked` in 92 s.
The fixture sits at the cap on a loaded 4-vCPU runner whichever producer runs it.

### What remains

The 60 s target is met on the six measured runners. The `windows_removal_walk`
group still takes 5.5 s as one task and was not re-sampled after this change.
The levers named under "Remaining time" stay open; none is needed for the
target.

## Hosted timing depends on the runner CPU

One producer took 44.3, 44.4 and 45.1 s for a direct cold check on a Windows
runner (AMD EPYC 9V74, 4 vCPUs) and 47.6, 47.2 and 47.4 s on a Linux runner
(EPYC 7763) in the same hour, while the pilot's own check legs of main and of
the first commit alone measured 74.5 s and 69.6 s on Windows runners whose CPU
was not recorded. Every large phase row on the Windows runner was 0.69 to 0.89
of the Linux runner's, with no phase standing out. Compare producers inside one
job, as the step above does, not across runs.
