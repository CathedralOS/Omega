# Data-oriented-design baseline: where a Dungeon compile spends its time

2026-11-02, main `c2cfe5f3dec`, Windows x86-64, Ryzen 9 7950X (16 cores, 32
threads, 127 GB). Release compiler (`opt_level` 3), SHA-256
`30a39fdc2fd259f8728e6ff8e8799ce44a2cf953b11cf5d24a566ae94dec06c3`.
**Decision: retain the measurement; no compiler change landed.**

A developer who edits `samples/cli/games/dungeon_crawler_cli` today waits for a
cold compile on every compile: about a minute here (52-68 s wall, about 100
CPU-seconds). The warm path has stopped working for every program that depends
on std. The record that lets a second compile skip checking is 640 MB; its
limit is 512 MiB. The encoder refuses it, and the miss is reported only when
`OMEGA_PHASE_PROFILE` is set. So `hello` takes 45 s the second time, as it does
the first. On Oct 31 it took 3.5 s.

**What to fix first:**

1. Get the retained record back under its limit, so warm reuse works again.
   This is worth about 50 s on every Dungeon recompile. A local build that only
   raised the limit (never landed) brings Dungeon warm to 9.5 CPU-s and 10 s
   wall under this load, with the same native image bytes.
2. In a warm compile, Psi 06 lowering (`checked_trees_to_lowered_psi`) takes
   6 of the 10.4 CPU-seconds. It reruns on every compile. Its payload-history
   analysis (`terminal_semantics` `payload_history`) alone takes 2.4 s.
3. In a cold compile, Psi 05 checking spends about a third of its CPU in
   overhead that data-oriented design can remove: lock-guarded lazy indices
   and interners, arena handle resolution, allocation churn, cloning and
   hashing.

The Omega-side sites this campaign started from are near zero on Dungeon.
They are the optimizer's analysis cache, `PsiOptimizationFunction::block`,
`OptimizationNode` sweeps, and the structural shape caches in abstract-to-target
lowering. The whole Omega optimizer is 0.17 CPU-s. Abstract-to-target lowering,
including every shape cache, is 0.12 CPU-s. The "4.4 s lowering" in the
Oct 31 note is Psi 06 terminal production, not
`02_abstract-operations-to-target-operations`.

## What a compile costs today

The host was saturated by other sessions during every run. CPU load stayed at
99-100%, and an `empty` warm compile took 1.6 s against 0.54-0.69 s on Oct 31.
Wall times are therefore about 2-3x an idle host. Compare runs from one session
only, interleaved. Each row also records the compiler process's own CPU time
(user plus kernel), which load inflates less.

Main compiler. Cold is a fresh private `LOCALAPPDATA`; warm reuses it. Three
interleaved rounds: three cold and six warm runs per program, medians.

| Program | Cold wall | Cold CPU | Warm wall | Warm CPU |
| --- | ---: | ---: | ---: | ---: |
| empty | 7.6 s | 9.7 s | 3.0 s | 1.5 s |
| hello | 45.9 s | 69.1 s | 44.8 s | 66.5 s |
| dungeon | 66.7 s | 107.1 s | 59.9 s | 101.8 s |

Probe compiler: identical except `source_graph.rs` `MAX_BYTES` raised to
3 GiB, built locally and never landed. Separate private project copies with
their own acceptance. Two rounds: two cold and four warm runs per program.

| Program | Cold wall | Cold CPU | Warm wall | Warm CPU |
| --- | ---: | ---: | ---: | ---: |
| hello | 49.4 s | 72.8 s | 7.4 s | 6.8 s |
| dungeon | 62.5 s | 108.0 s | 10.0 s | 9.5 s |

Every Dungeon compile, main or probe, cold or warm, published the same native
image (SHA-256 prefix `67c42e6689f87c75`); hello gave `80a0534ee925c928` and
empty `7e618a3c6c54d174`. All 39 timed compiles exited 0. One earlier probe
round was discarded: its project copies had been accepted with the main
compiler, so they refused to compile.

Warm-reuse record (main, `OMEGA_PHASE_PROFILE=1`):
`retained source graph ignored: encode retained source graph: compiler record at byte 536870909: encoded byte limit exceeded`.
Dungeon: preparation 94.7 MB, sources 49.6 MB, retained checks about 495 MB
(the probe's stored record is 639,577,548 bytes). `empty` (core only) stores a
59 MB record and is warm.

### Where the time goes, by stage

True warm Dungeon (probe, phase scopes, wall under load), total 10.7 s:

| Scope | Time |
| --- | ---: |
| Psi 06 `lower_terminal_selection` (3 calls) | 6.37 s |
| of which `reconcile_and_residences.payload_memberships` | 3.18 s |
| of which `lower_selected_machine.emit_composed_body` (585 calls) | 2.07 s |
| of which `payload.history` (498 calls) | 1.86 s |
| `source.graph.recovery` (read 0.27, digest 0.25, decode 0.93) | 1.49 s |
| `omega.product.produce_native_product` (all Omega stages) | 1.02 s |

Cold Dungeon (main, phase scopes; scopes are per-thread inclusive and overlap
across checking threads):

| Scope | Time |
| --- | ---: |
| Psi 05 total (wall) | 35.7 s |
| `psi.05.numeric.observe_prefixes` (1,257 calls, summed over threads) | 21.5 s |
| `psi.05.build_check_facts.flow_fixed_point` | 18.0 s |
| `psi.05.check_checked_facts` | 8.0 s |
| `psi.05.permissions.walk` + `setup` | 3.8 s |
| `psi.05.prepare_typed_program.validate` | 1.9 s |
| `psi.05.finalize_execution_identities` | 1.5 s |
| settle, including Psi 06 | 6.7 s |

Allocation, cold Dungeon (allocation-observation build): 413.6 M allocations,
37.0 M reallocations, 105.6 GB requested in total, 2.86 GB peak live.

## Ranked sites

CPU samples from the Visual Studio standard collector at its base rate (about
1 kHz: 96,726 samples for a cold Dungeon whose process used 98-108 CPU-s;
10,404 for a warm one at 8.4-12.4 CPU-s), so samples read as CPU-milliseconds.
"Excl" sums the samples that landed in matching functions. "Incl" is the
largest inclusive count of any matching function.

| Rank | Site | Cold Dungeon | Warm Dungeon |
| ---: | --- | ---: | ---: |
| 1 | Warm reuse lost: record over 512 MiB, so warm = cold | about 50 s wall per compile | (restores 9.5 CPU-s warm) |
| 2 | Psi 06 lowering `lower_machine` (incl), payload history 2.4 s inside | 5.8 s | 6.0 s |
| 3 | `OwnedInterner<TypeIdentityTable>::with_mut` (incl); 7.1 s of it is `Mutex::lock_contended` | 8.1 s | 0 |
| 4 | Allocator, mimalloc `mi_*` (excl) | 6.7 s | 0.93 s |
| 5 | `PagedArenaSet` handle resolution, `page_for_generation`/`span` (excl) | 6.1 s | 0.05 s |
| 6 | `OwnedIndex::with`/`with_mut` lazy indices (excl); top: `DomainPositionIndex` 1.5 s, `MachinePositionIndex` 1.3 s | 4.2 s | 0.08 s |
| 7 | `Clone::clone` 2.9 s + drop glue 1.0 s (excl) | 3.9 s | 0.36 s |
| 8 | `HashMap` (hashbrown 1.6 s + SipHash `DefaultHasher` 1.2 s, excl) | 2.8 s | 0 |
| 9 | Retained record recovery (read, SHA-256 digest, decode) | n/a | 1.5 s |
| 10 | `BTreeMap`/`BTreeSet` out-of-line operations, any key (excl) | 1.26 s | 0.24 s |
| 11 | `Proposition` eq/cmp/clone (excl), mostly in Psi 06 payload history | 0.58 s | 0.64 s |
| 12 | Psi 05 `flow/context.rs` maps (excl); the flow fixed point around them is 33 s incl | 0.31 s | 0 |
| 13 | `ScalarTerm` eq/cmp/clone/drop (excl) | 0.09 s | 0.12 s |
| 14 | Omega optimizer `optimize_abstract_operations` (incl), which contains `AnalysisManager::commit_revision`, `PsiOptimizationFunction::block` and `OptimizationNode` sweeps | 0.17 s | 0.17 s |
| 15 | Abstract-to-target lowering, every structural shape cache included (incl) | 0.11 s | 0.12 s |
| 16 | `BTreeMap` operations keyed by `semantic_vocabulary::identity` IDs, out of line (excl) | 0.05 s | 0.03 s |
| 17 | `KeyedRows` (excl) | 0.02 s | 0 |

Rows 14 and 15 are upper bounds for every Omega-side site named in this
campaign. `commit_revision`, `block`, `OptimizationNode` and the shape-cache
functions are each below the reporting threshold of 20 samples.

ID-keyed `BTreeMap` lookups (row 16) cannot be separated well by sampling. A
small-key search is usually inlined into its caller, and its cost then appears
in that caller. Row 10 bounds the out-of-line share. The remainder sits inside
the callers ranked above.

### Verdicts on W6, W7 and W8

- **W6, optimizer graph structure-of-arrays: not justified.** The whole Omega
  optimizer is 0.17 CPU-s of a 10.4 CPU-s warm Dungeon and of a 97 CPU-s cold
  one. Even deleting it outright would not move either target. W1 (delete the
  analysis cache) and the shape-table half of W2 sit inside the same 0.3 s;
  land them only as simplification, without expecting a time change.
- **W7, `ScalarTerm` arena: not justified as scoped.** `ScalarTerm` clones,
  drops and comparisons are 0.09 s cold and 0.12 s warm. The deep structural
  work one level up is `Proposition` eq/cmp/clone at 0.64 s warm (6%), mostly
  in Psi 06 payload history. Interning propositions there could be worth that
  much, but only as part of the Psi 06 repair (rank 2), not as an arena
  migration.
- **W8, kind bits in the handle generation: justified only as a bounded cold
  experiment.** Handle resolution (`PagedArenaSet::page_for_generation` and
  `span`) is 6.1 s exclusive in a cold compile (6%) and 0.05 s warm. Some of
  those samples are first-touch loads of arena data. Those loads stay with
  the caller, so the saving is at most 6 s and probably less. Measure one
  arena family before migrating the rest.

The profile points the remaining lanes at different work. W9's lock-guarded
lazy indices and interners (ranks 3 and 6) are 12 s of cold CPU, and 7 s of
that is threads waiting on one interner's mutex. W5 (flow precompute) works
inside an 18 s flow fixed point. Neither changes a warm compile, which is
rank 1 and rank 2.

## Proposed acceptance

The targets are CPU-seconds of the `omega` process: the median over the
interleaved runs from the script below. Wall time on this shared host moves
2-3x with other sessions' load. A candidate also passes only if every
published native image (empty, hello, Dungeon) is byte-identical to base.

| Target | Today | Proposal | Reachable because |
| --- | ---: | ---: | --- |
| Warm Dungeon | 101.8 CPU-s (reuse lost); 9.5 with reuse | **≤ 3.5 CPU-s** (about 3 s wall idle) | reuse restored (-92 s), Psi 06 not recomputed for unchanged inputs (-6 s), record recovery halved (-0.7 s) |
| Cold Dungeon | 107.1 CPU-s, 66.7 s wall | **≤ 55 CPU-s**, with wall ≤ 50% of base in the same session | ranks 3-8 total about 32 s of cold CPU (with some overlap), plus 3.9 s of unsymbolized `memcpy`, plus parallelism regained once checking threads stop serializing on the type-identity interner |

The proposed warm ≤ 3 s and cold ≤ 10 s are not both reachable with
data-oriented changes alone:

- **Warm ≤ 3 s is reachable**, but only by the first two fixes above. The
  W-lanes as scoped target sites worth under 0.3 s of a warm compile.
- **Cold ≤ 10 s is not reachable by data-oriented design alone.** `hello`
  already costs 69 CPU-s cold. That is checking core and std, identical work
  for every program on one compiler and library revision. Removing every DoD
  site ranked here leaves roughly 70 CPU-s. A 10 s cold Dungeon also requires
  reusing checked std across projects. That is a cache design decision outside
  this campaign, and it conflicts with W9's plan to delete the cross-run
  checked-library store.

## Measurement notes

- WPR refused to start without elevation ("Failed to enable the policy to
  profile system performance"). The Visual Studio 2022 standard collector
  (`VSDiagnostics.exe`, CPU agent) samples without elevation through its
  service. It records only the launched process, so the script launches
  `omega` directly. The launched process inherits the caller's environment,
  including `LOCALAPPDATA`.
- The release build links with identical-code folding, so a folded frame can
  carry another function's name. Example: the warm profile shows
  `CheckedScalarSelector::eq` with 2.3 s inclusive whose callees are unrelated
  Psi 06 functions. Rows above use exclusive leaf sums or inclusive counts
  checked against their callers; that frame was excluded.
- `memcpy`/`memmove` land in `vcruntime140.dll` without symbols: 3.9 s cold
  and 0.27 s warm. They are not attributed to callers and not ranked, but they
  are the same copying cost as rank 7.
- Psi 06 runs `lower_terminal_selection` three times for a Windows-only build
  (2.1 s each warm). Whether all three are needed was not investigated.
- The probe and the main compiler differ only in `MAX_BYTES` and the record
  element limit. Each project copy was accepted through
  `tools/benchmark/benchmark.py prepare` with its own compiler, because an
  accepted lock binds the compiler identity.

## Repeat the measurement

From a worktree root with `build/dod/{empty,hello,dungeon}` prepared as above.
Copy `build/pscale/{empty,hello}` from the
[Oct 31 probes](../compile_scaling_2026_10_31/README.md) and
`samples/cli/games/dungeon_crawler_cli` with its std path shortened by one
`../`, then run `benchmark.py prepare` per project with the measured compiler.
Keep the scripts in `build/dod/scripts/`. Run timing and profiles one after
another, never concurrently.

`run1.ps1`: one compile; prints exit code, wall time, and the process's own CPU time.

```powershell
param([string]$Omega, [string]$Root, [string]$Cache, [string]$Out)
$env:LOCALAPPDATA = $Cache
$info = New-Object System.Diagnostics.ProcessStartInfo $Omega
$info.Arguments = "--timings `"$Root`""; $info.UseShellExecute = $false
$info.RedirectStandardOutput = $true; $info.RedirectStandardError = $true; $info.RedirectStandardInput = $true
$clock = [Diagnostics.Stopwatch]::StartNew()
$process = [Diagnostics.Process]::Start($info); $process.StandardInput.Close()
$stdout = $process.StandardOutput.ReadToEndAsync(); $stderr = $process.StandardError.ReadToEndAsync()
$process.WaitForExit(); $clock.Stop()
[IO.File]::WriteAllText($Out, $stdout.Result + $stderr.Result)
"exit=$($process.ExitCode) wall_ms=$($clock.ElapsedMilliseconds) cpu_ms=$([int]$process.TotalProcessorTime.TotalMilliseconds)"
```

`measure.sh`: interleaved cold/warm rounds; usage `OMEGA=<exe> OUT=<new dir> [ROUNDS=3] [PROGRAMS="empty hello dungeon"] sh measure.sh`.

```sh
O="$(cygpath -wa "${OMEGA:?}")"; S="${OUT:?}"; R="${ROUNDS:-3}"; mkdir -p "$S/m"
here="$(cygpath -wa "$(dirname "$0")")"
for round in $(seq 1 "$R"); do for p in ${PROGRAMS:-empty hello dungeon}; do
  c="$S/cache/$p-$round"; rm -rf "$c"; mkdir -p "$c"
  for mode in cold warm1 warm2; do out="$S/m/$p-$round-$mode.txt"
    row=$(powershell -NoProfile -ExecutionPolicy Bypass -File "$here\\run1.ps1" -Omega "$O" \
      -Root "$(cygpath -wa build/dod/$p/main.omg)" -Cache "$(cygpath -wa "$c")" -Out "$(cygpath -wa "$out")" | tr -d '\r')
    echo "$p round=$round $mode $row $(grep 'total elapsed' "$out" | awk '{print "self_ms="$1}') image=$(sha256sum build/dod/$p/build/omega-program.exe | cut -c1-16)"
done; done; done
```

`profile.sh`: CPU samples for one compile; usage `OMEGA=<exe beside its .pdb> PROGRAM=<dir> CACHE=<dir> OUT=<new dir> sh profile.sh`.
Pass an empty `CACHE` for a cold profile, or a warm one to profile reuse.

```sh
set -e; export MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*'
VS="/c/Program Files/Microsoft Visual Studio/2022/Community/Team Tools/DiagnosticsHub/Collector"
XPERF="/c/Program Files (x86)/Windows Kits/10/Windows Performance Toolkit/xperf.exe"
mkdir -p "$OUT" "$CACHE"; out=$(cygpath -wa "$OUT")
cp "$OMEGA" "$OUT/omega-prof.exe"; cp "${OMEGA%.exe}.pdb" "$OUT/omega.pdb"
id=$(powershell -NoProfile -Command "[guid]::NewGuid().ToString()" | tr -d '\r')
export LOCALAPPDATA=$(cygpath -wa "$CACHE")
"$VS/VSDiagnostics.exe" start "$id" "/launch:$out\\omega-prof.exe" \
  "/launchArgs:--build-dir $out\\build $(cygpath -wa build/dod/$PROGRAM/main.omg)" \
  "/loadConfig:$(cygpath -wa "$VS/AgentConfigs/CpuUsageBase.json")" > /dev/null
while tasklist /FI "IMAGENAME eq omega-prof.exe" | grep -q omega-prof; do sleep 1; done
"$VS/VSDiagnostics.exe" stop "$id" "/output:$out\\cpu.diagsession" > /dev/null
"$VS/VSDiagnostics.exe" expandDiagSession "$out\\cpu.diagsession" > /dev/null
etl=$(find "$OUT/cpu" -name '*.etl' | head -1)
_NT_SYMBOL_PATH="$out" "$XPERF" -i "$(cygpath -wa "$etl")" -symbols -a stack -butterfly 20 -process omega-prof > "$OUT/butterfly.html"
# inclusive<TAB>exclusive<TAB>function, one row per function with >= 20 samples
sed 's/<\/tr>/\n/g' "$OUT/butterfly.html" | awk "/id='TblSI'/{f=1} /id='TblSN'/{f=0} f" \
 | sed 's/<\/td><td[^>]*>/\t/g; s/<[^>]*>//g; s/&amp;/\&/g; s/&lt;/</g; s/&gt;/>/g' \
 | awk -F'\t' 'NF>=4 && $2 ~ /^[0-9]+$/ {print $2 "\t" $4 "\t" $1}' | sort -nr > "$OUT/functions.tsv"
```

Read a site from `functions.tsv` by summing exclusive samples over a mangled
name pattern. Examples: `awk -F'\t' '$3 ~ /15paged_arena_set/ {s+=$2} END {print s}'`,
`/11owned_index/`, `/!_?mi_/`, `/5Clone5clone/`, `/11collections5btree/`.
For an inclusive site, take the largest first column instead. Examples:
`/28optimize_abstract_operations/`, `/13lower_machine/`,
`/14owned_interner.*17TypeIdentityTable.*8with_mut/`.
Phase scopes come from `OMEGA_PHASE_PROFILE=1 omega --timings ...`. With that
variable set, the compiler prints `retained source graph ignored: ...` whenever
the warm record is refused. Allocation totals come from a
`--features allocation-observation` build run with
`OMEGA_ALLOCATION_PROFILE=<file>`.

Raw rows, traces and phase dumps are local to the measuring host and are not
retained in git.
