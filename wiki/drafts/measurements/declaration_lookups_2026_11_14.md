# Exact declaration lookup during typing

Typing now reuses the declarations that resolved binding and field identities
already name, instead of repeatedly walking unbounded local or record sets.
One declaration owner supplies selected-body coordinates and the published
root view; it retires unused global declaration caches.
Small bodies retain a bounded scan: the measurements favored it through the
tested 32-row range. Larger bodies retain coordinates for each queried body,
including when queries alternate between bodies. These are lookup measurements,
not a claim about whole-compiler latency.

The field query borrows the unique row in the existing data-member relation.
It retains main's unique arena stamps and complete root roster. The binding
relation uses the parameter stamp, a private statement-binding token and
complete body spans. Duplicate binders, stale generations and
foreign constructor/case ownership remain refusals. Other declaration relations
are outside these measurements.

## Inputs and actual reuse

The first draft retained only the current body's rows. Temporary query/build
events during Psi 03 lowering of three existing standalone samples showed
repeated preparation when passes returned to earlier bodies:

| Existing source | Binding queries | Preparations | Queried body scopes | Scan row visits | Prepared row visits |
| --- | ---: | ---: | ---: | ---: | ---: |
| `selected_record_update/main.omg` | 44 | 6 | 4 | 262 | 35 |
| `borrowed_field_bundle/main.omg` | 30 | 9 | 6 | 158 | 38 |
| `borrowed_nested_sum_replacement/main.omg` | 41 | 8 | 5 | 342 | 42 |

Query and preparation counts came from actual entry/build events. Row visits
are derived from those scopes' exact parameter and statement slice lengths.
The sources have 22 bodies in total, with 0–3 parameters, 0–7 locals and 0–12
statements. Their data declarations have 1–3 fields. All fit the final bounded
scan route. The three runs made zero exact-field fallback queries and zero
data-member index preparations at this boundary; they do not establish field
query frequency in ordinary imported applications. No bundled core/std
workload was profiled in those initial standalone Psi 03 runs.

The retained-body change follows the observed 23 preparations for 15 queried
scopes. The final algorithm prepares selected body indexes only when queried.
Root publication streams the same declaration coordinates, borrowing already
prepared bodies, without materializing an index for every tiny body. Complete
body spans survive expression/type edits and root-roster changes. Parameter or
statement storage mutation conservatively invalidates that family; another
body's query does not. Frozen published reads remain independent of the
mutable owner's mutex.

## Measurement method

Host: Windows x86_64, Intel i9-12900K, 24 logical CPUs. One heavy chain ran at
Idle priority with affinity `0xFFFFC0`, `MBX_SCHEDULER_CPUS=18` and
`CARGO_BUILD_JOBS=18`. Rust builds used pinned mbx 1.29.2 and the development
profile with optimization level 1.

The portable Rust example `typed-trees/examples/declaration_lookups.rs`
constructs declaration-table payloads and compares the production lookup with
an equivalent exhaustive scan in the same executable. These generated
payloads isolate the relationship query; they are not complete Omega programs.
Bindings use one parameter and the remaining rows as locals. Fields hold the
queried symbol fixed while growing unrelated one-field records. The shared
omega-bench runner alternates baseline/candidate order, retains two warmups and
15 measured pairs, verifies exact work counts/checksums, and uses its existing
bootstrap statistics. Scan-visit counters are compiled out of timed loops.

Hot runs prepare once outside the timed loop. Alternating runs query two
distinct bodies in one owner. Cold runs clone an unprepared owner outside each
timed interval and time its first lookup; clone/drop time is excluded. Cold
intervals retain clock-call overhead in both roles. They are not cold OS-cache
or whole-compilation measurements.

The initial run found hot binding medians of 26.85/38.00 ns at 16 rows,
43.61/46.90 ns at 32 rows, and 84.93/49.08 ns at 64 rows (scan/current-body
cache). This motivated a conservative 32-row bounded policy, not an exact or
universal crossover. The final run also checks the boundary and alternating
scope behavior. Larger stress cases are secondary to the observed tiny bodies.

## Historical lookup measurements

These first measurements precede the shared-owner refactor below. Their source
patches and executable hashes are retained; they are not fresh results for the
submitted owner representation.

All 40 final comparisons and six targeted 33/48-row boundary comparisons
completed and passed the runner's work-count, checksum and evidence checks.
Values below are medians in nanoseconds per query. Percentage changes and
their 95% intervals are the runner's paired bootstrap observations, not ratios
of the two displayed medians.

| Payload / mode | Scan ns | Lookup ns | Paired change | 95% interval |
| --- | ---: | ---: | ---: | --- |
| Binding 1 / hot | 17.22 | 19.91 | +15.63% | +13.24 to +20.93% |
| Binding 33 / hot | 45.68 | 42.63 | -3.78% | -9.68 to -1.99% |
| Binding 64 / hot | 77.12 | 42.21 | -45.10% | -48.50 to -44.00% |
| Binding 64 / alternating | 83.90 | 47.04 | -42.04% | -50.49 to -38.12% |
| Binding 1,024 / hot | 1,212.42 | 72.86 | -94.82% | -95.18 to -93.32% |
| Field 1 / hot | 32.11 | 63.70 | +103.28% | +92.45 to +117.55% |
| Field 1,024 / hot | 19,134.34 | 80.19 | -99.61% | -99.66 to -99.54% |

The policy avoids the first draft's preparation overhead on the observed
small bodies. It does not make every tiny query faster: the one-row binding
case retains about 2.7 ns of overhead, and a single-field relation is slower
even when warm. The field path's real invocation frequency was not established
by these three sources, so no general application speedup is inferred.

Cold preparation remains more expensive and creates a first-query cliff above
the bounded scan range. Binding 33 measured 110.6/487.6 ns, binding 48
124.95/526.0 ns and binding 64 187.25/866.35 ns. Binding 256 measured
597.0/2,246.2 ns. A one-field cold relation measured 107.55/451.70 ns;
1,024 fields measured 21,236.6/23,487.8 ns, with an inconclusive paired interval
of -3.89 to +26.66%. This repair favors reuse, while keeping the observed tiny
body distribution on a scan; it does not remove preparation cost.

Allocation/visit audits independently varied row counts (4, 64, 1,024) and
query counts (1, 4, 16, 64, 1,000). The exhaustive scan's actual counter grows
as rows times queries; 64 rows and 1,000 queries visited 64,000 rows. After
preparation, repeated successful lookups requested zero allocations at every
audited size/query count. The 4-row binding route also requested none cold.
Preparation for 64 binding rows requested four allocations totaling 2,432
bytes; 1,024 fields requested 12 allocations totaling 81,840 bytes. These are
requested bytes including temporary construction storage, not retained bytes.
The largest field/cold process peak-RSS medians were 6,717,440/7,278,592 bytes
(scan/lookup). No RAM reduction is claimed.

## Pre-integration owner and construction controls

This measured owner replaces the separate body cache and the old global copies of
types with selected-body coordinates and a derived root projection. A second
Rust helper, `declaration_owner`, compares actual public APIs built against
base `372860cf534b9624ee1e0130dd6819abd0e1be35` and the owner refactor. Seven
payload shapes use two warmups and nine paired observations. The common-size
control has 22 bodies: 18 with one parameter/one local, four with eight total
rows. Larger controls vary 32/128/1,024 tiny bodies, four 64-row bodies, reversed
root/selected order, no-local tiny bodies, and a secondary 128-large-body case.
These are generated metadata distributions, not observed application rosters.

All 49 comparisons passed their work/checksum/evidence checks. Seven cold
selected-family cases were rerun after strengthening the helper's checksum to
consume every result; those reruns supersede that mode's initial observations.
Each cold selected observation measures a complete family preparation batch.

| Owner workload | Baseline ns | Owner ns | Paired change / 95% interval |
| --- | ---: | ---: | --- |
| Common-size cold root | 5,970 | 3,775 | -34.18% / -48.03 to -29.43% |
| Common-size cold seal | 11,700 | 9,460 | -31.44% / -57.05 to -7.65% |
| Common-size frozen global | 14.92 | 30.38 | +101.25% / +48.87 to +108.46% |
| Common-size frozen scoped | 14.89 | 15.70 | inconclusive |
| Common-size cold selected batch | 210 | 270 | inconclusive |
| 1,024 tiny + four large, reversed, cold root | 292,055 | 131,785 | -51.84% / -56.95 to -49.66% |
| Same, cold seal | 264,760 | 132,870 | -51.68% / -59.36 to -36.58% |
| Same, cold selected batch | 4,200 | 9,115 | +195.46% / +67.14 to +290.85% |
| 128 large, reversed, cold selected batch | 84,950 | 219,180 | +142.07% / +108.64 to +196.72% |

Frozen global reads retain roughly 10–17 ns overhead in these shapes. At the
common-size medians, about 145 global queries would offset the seal saving;
that arithmetic is not a universal crossover. Cold selected-body indexing also
remains costlier, including reversed insertion into the sorted selected-family
vector. Root publication does not insert those families one by one. A separate
18-case binding rerun measured 64-row hot/alternating queries at
101.88/110.21 ns for scans and 40.89/48.10 ns for the owner; cold was
195.85/861.10 ns. No whole-compiler speedup is inferred.

Common-size cold root allocation requests changed from 27/13,704 bytes to
12/5,692 bytes; sealing from 47/16,856 to 28/7,412. With 1,024 no-local tiny
bodies and four large bodies, root preparation changed from 49/376,624 to
12/20,624. These include temporary construction storage, not retained memory.
All audited 1,000-query hot global streams requested zero allocations.

Before the typed call-edit projection, one temporary event run through the existing `borrowed_field_bundle` product
source route, including package/library processing, passed native/Psi exit 0.
Package update made 30,638 scoped queries, 12 selected preparations and two
root publications/preparations. The run command made 30,666 scoped queries,
12 selected preparations and four root publications/preparations. Both made
zero global unique/list queries. This event format did not retain body sizes,
so it cannot establish a tiny/indexed query histogram. The events were removed
before final validation; their source patch and raw stderr are retained.

Body lowering inserts its completed statement span before cohort closing, but
constructor inference can query a binder and then annotate a call statement.
The original general mutable accessor invalidated binding rows on each such
annotation. A canonical 64/128-local control with 128 annotations and queries
requested 640 allocations and 319,488/581,632 bytes after initial preparation.
The typed call-payload accessor now retains binding coordinates and requested
zero in both controls. It still advances the ordinary statement revision;
general statement/local/variant replacement invalidates the binding family.
The binding token has no consumer requiring complete call contents. These
annotation changes do not alter the timed lookup branches above.

## Final integration with main's checked spans

The required checked-span contract is integrated at `72401d8d9e2c9774b53b1ce2cb332e5b8f823933`.
The owner retains main's unique revisions and refuses malformed rosters through
checked views; root/field relations never publish a valid prefix of bad input.
Earlier owner and stamp-only integration numbers remain historical.
Seven owner, three binding and four field comparisons passed. Owner comparisons
against the archived fixed base include main's stamp prerequisites; lookup/scan
comparisons use the same final executable. Medians are ns per query or cold batch:

| Workload | Baseline/scan ns | Final ns |
| --- | ---: | ---: |
| Common-size cold root / seal | 5,530 / 8,415 | 3,425 / 6,225 |
| Common-size frozen global / scoped | 14.26 / 13.20 | 26.76 / 13.73 |
| Binding 64: hot / alternating / cold | 101.13 / 104.80 / 186.45 | 35.85 / 37.34 / 932.45 |
| Field 1: hot / cold | 19.97 / 68.50 | 20.29 / 286.95 |
| Field 4: hot / cold | 54.26 / 103.20 | 21.41 / 297.15 |

Cold root and one-field hot results are inconclusive. Cold seal improved 29.19%
(95% interval -34.83 to -15.00%); tiny frozen scoped reads retain 0.53 ns overhead,
global reads about 12.5 ns. Cold selected indexing and field preparation cost more.
The final allocation and call-annotation audits reproduce the counts above.
Raw reports retain paired intervals; no whole-compiler latency claim is made.

## Correctness checks

Fourteen declaration-lookup tests pass, including small/indexed routes, duplicate
and foreign binders, stale symbols/types/rosters, same-shape storage
replacement, retained/frozen snapshots, expression edits and alternating body
scopes, malformed root/member/payload refusals, Record/equality neutrality, published/detached views and frozen reads while the
mutable owner's mutex is deliberately held. Psi 03's `typing_queries` tests pass 8/8 and
`domain_cast_accounting` passes 7/7; Psi 04's `record_patterns` consumer passes
2/2. Ten nearby declaration-index tests and one statement-revision test pass.
Both changed crates pass all-target Clippy with warnings denied. Later call-annotation conflict integration at `28659ccd3c0675a1678b6016d2872ed32ed1c486` preserves the tested owner-file blobs and passes two complete call-family controls plus typing-crate Clippy.

Before the shared-owner refactor, `mbx test -p typed-trees --lib` passed
127/129 at checkpoint `237b884bbf3e9f9f9992ea217fc426a829abd581`. The two
failures are the existing `instantiation_refuses_a_binder_spelling_an_array_length`
and `instantiation_applies_the_substitution_to_domain_arguments` expectations.
They exercise only the untouched type-reference table and symbol builder. The
base queue records the same failures at
`44ed2132219fb20261abeab4ef3e61b4ab0f845a`; that historical observation is not a
fresh baseline run at this candidate. No expectation was changed.

`mbx xtask sample-gate --filter samples/cli/probes/borrowed_field_bundle`
passes on `windows_x86_64` with native exit 0 and Psi exit 0. The initial
`selected_record_update` gate at the fixed base refused during package checking with
`type mutable typed sequence requires its actual exclusive backing and loan`.
Its authored `&mut [Record]` meets the existing explicit refusal in unchanged
Psi 05 `read/arrays.rs::source_sequence_view`; this repair does not supply that
missing lowering capability. The sample and its expected result remain
unchanged. Native execution on the other hosted targets was not run here.

## Reproduction and custody

Build the Rust examples from the checkout root:

```text
mbx build -p typed-trees --example declaration_lookups
mbx build -p typed-trees --example declaration_owner
mbx build -p symbol-resolved-trees-to-typed-trees --example declaration_shapes
```

Apply the host budget from `tools/docs/build_slot.md` and optimization level 1
before building/running. The examples and plan generation are Rust; they do
not require PowerShell or Python. The executable suffix follows the host.

```text
./target/debug/examples/declaration_shapes samples/cli/probes/selected_record_update/main.omg
./target/debug/examples/declaration_lookups plans build/performance/declaration-lookups/plans
omega-bench suite build/performance/declaration-lookups/plans/suite.json build/performance/declaration-lookups/run
./target/debug/examples/declaration_lookups audit bindings 64 1000
./target/debug/examples/declaration_lookups plans build/performance/declaration-lookups/boundary-plans 33,48 bindings
./target/debug/examples/declaration_owner plans build/performance/declaration-lookups/owner-plans BASELINE_EXECUTABLE CANDIDATE_EXECUTABLE
./target/debug/examples/declaration_lookups construction 64 128 broad
./target/debug/examples/declaration_lookups construction 64 128
```

For the owner comparison, build the same helper with the five typed-tree
production files (`lib`, `declared_bindings`, `declaration_tables_and_index`,
`calls_and_control_flow`, `operator_declarations`) at the fixed base, then with
their submitted versions, preserving each executable. The narrow call-edit
audit compares general and typed accessors in the final executable.

Raw journals, captures, plans, allocation observations, events, source patches,
archived executables and build/test logs remain ignored under
`C:/Projects/Omega/.agents/worktree/worker-30/build/performance/declaration-lookups/`.
The fixed source base is `372860cf534b9624ee1e0130dd6819abd0e1be35`.
Rust was `1.100.0-nightly (a69a63265 2026-09-03)` with LLVM 23.1.1.
Each journal retains its exact executable/input hashes. Later untimed helper
additions and the now-test-only unused revision getter do not alter measured
lookup branches. SHA-256 custody:

| Artifact | SHA-256 |
| --- | --- |
| Initial source patch | `90687db587ee4003a9bdfb6dae65d45e3be54177c1a5f8278edf0032d30a8061` |
| Historical 40-case source patch | `0fb409ed2bcd8820ad5646150c22ae60cff0749bf3004fe7150bdaf1f682ddf4` |
| Historical 40-case executable | `82fa158e6f52265d476abdd814f6e55b6efc36c5b030936c017852ace2d83859` |
| Pre-integration owner executable | `19068be2bdc6822235079657ba8e6a140e7447585326f364c22956449b1ea99d` |
| Its initial baseline executable | `34ef042fe1f7143df4ef04c4854681b45a6f56759a81fabd5411d702ca3cabe8` |
| Owner-event source patch | `03823b9a47b1b03139898094d16d029a6c35aa2adbbeefbeeb1d7d6f7f95568c` |
| Pre-integration production/test patch | `226fadb140d6b04a956dbfff652382f50e82ff54a8c280b73db1386c157f2d48` |
| Stamp-only integration production/test patch | `bc6fa545b57f1fabb931fc47d30feaad46b94682b12e756ea1867cc954b6f714` |
| Checked-span production/test patch | `be45b8396ae082f471c1655c1f8e04e37ab950ba9a7b303bb9be01bcf0709bf1` |
| Checked-span owner executable | `9efbc431bc3eb28ccbe5824d1c019162f4a3c4990ef988e51e1fb9d57ac577e4` |
| Strengthened fixed-base baseline | `efa600fca274071a6802b199c39882df5fe39866bdb797806d55f0eb6e7ebb0a` |
| Checked-span lookup executable | `2bf2349899ccdf97453f5a946041bd492b876daa337c1e14419ddd0b9c232ba3` |
| Reused omega-bench runner | `82df5142ad8de9441b99798c0374094881b55ef4ff7451b9fb9f7a4b2cb3312c` |

The claim note retains the custody location; these observations remain tied to
their recorded source patches/executables rather than becoming fresh results
on later main revisions.
