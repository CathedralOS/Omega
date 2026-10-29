# Source graph decoding experiment

2026-10-29. Fixed baseline: `1fecc20b6f5292fa8673feed5cae2300a925433d`.

Question: can fixed-width scalar sequences be restored with one checked byte span
instead of per-element cursor checks and temporary heap vectors for fixed arrays?
The measured warm square-sum profile spent 426.234 ms in graph decoding and
799.581 ms in source resolution, out of 1,302.363 ms compiler elapsed time.
Its isolated, natively prepared graph remains 90,500,975 bytes across four runs.

Queries: `site.doc.rust-lang.org alloc collections btree map FromIterator sorted
bulk build`; `site.doc.rust-lang.org std sync Arc from Vec allocation copy uninit
slice`. Read the primary [BTreeMap implementation](https://doc.rust-lang.org/src/alloc/collections/btree/map.rs.html),
[slice fixed-size chunks](https://doc.rust-lang.org/std/primitive.slice.html#method.as_chunks)
and [iterator collection](https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.collect).

The first two leads introduce extra temporary collections, changed map validation,
or unsafe/uninitialized allocation machinery. They are not the first candidate.
The existing fixed-array decoder allocates a Vec even for a 32-byte digest and
calls the scalar cursor separately for every element. Rust's safe fixed-size
slice chunks let the scalar owner decode explicit little-endian words after one
bounded span check. Fixed scalar arrays can be constructed directly on the stack.

Hypothesis: this removes cursor/temporary-allocation work from graph restoration
without changing record bytes, semantic identities, shared-object reconstruction,
or source/authority admission. The generic record path still validates each
non-scalar element. Element, allocation and depth budgets stay in their existing
owners, including conservative allocation charging for fixed arrays.

Acceptance: malformed/truncated/budget-exceeding inputs still reject; scalar
endianness, signed/float bit patterns, shared identity and native output survive.
Two byte-identical projects, each ordinarily reviewed by its own compiler, use
separate private cache roots and identical warmup sequences. Full compile-to-image
time, native execution and exact image equality decide retention; a local decode
gain alone is insufficient. Phase capture is separate from timing comparisons.

Tool prerequisite: compiler-specific accepted locks cannot be shared between
different compiler builds. Optional `candidate_source` selects an independently
reviewed copy while retaining the common target/library/oracle and declared
source/lock identities. No compiler identity, source digest, custody or admission
check is disabled. The first refused setup attempt is kept in the local record.

## Scalar batching: rejected

Twenty measured pairs per workload, two warmup pairs, isolated cache roots and
independently reviewed identical source copies produced no established win.
Length: 1,521.513 -> 1,599.130 ms median process compile latency; paired change
interval [-1.341%, +28.869%]. Square-sum: 1,110.705 -> 1,092.112 ms; paired
interval [-12.057%, +5.664%]. Neither interval separates improvement from noise.
Separate decode profile medians were 298.706 -> 297.160 ms (length) and
296.382 -> 328.658 ms (square-sum). Do not promote the favorable square-sum
median while omitting its uncertainty or the other workload.

All 88 generated images passed their native oracle and exact equality within
each workload. Both length programs agreed between Terminal Psi and native;
both invalid twins rejected the same missing index proof. Candidate cache digest
corruption was refused, and fresh compilation produced the identical native
image. Per-role cache sizes and hashes stayed unchanged throughout measurements.
The candidate passed 73 decoder/arena tests, 32 decoder/tool tests and scoped
Clippy. The result is rejected because benefit was not demonstrated, despite
correctness. Production batching is removed; useful format/budget tests remain.

`scalar-batch-result.json`, producer identities, the exact candidate patch, and
`scalar-batch-observations.zip` retain this experiment. The archive omits generated
executables and graph payloads; complete local replay material stays in ignored
`build/performance/source-decoding/`. Its compiler-specific cache files are
separately digest-bound. These are warm repeated builds on a shared Windows host,
not cold-start or other-workload performance evidence.

Next hypothesis: force the small per-record depth wrapper into its caller so
large decoded values do not cross an extra return boundary. Preserve the original
scalar/collection implementation and measure the full workload again.

## Record wrapper inlining: rejected

The second candidate only forced the existing reader depth wrapper inline.
Twenty pairs per workload again produced no established end-to-end benefit:
length medians 1,306.696 -> 1,250.437 ms, paired interval [-4.443%, +8.643%];
square-sum 1,079.598 -> 1,147.860 ms, interval [-0.291%, +9.936%]. Separate
decode medians were 294.593 -> 302.269 ms and 296.709 -> 281.896 ms. All native,
invalid-source, cache-integrity and corruption-recovery controls passed. The
annotation is removed. `record-wrapper/` retains its exact source patch,
identities, results and raw observations. The changing shared-host load makes
sequential medians particularly misleading; neither experiment supports a win.

Next: instrument the four actual decode owners (preparation, sources, retained
checks, reader cleanup) before selecting another implementation change.

## Ownership attribution

`attribution/` binds an instrumented producer and three native-valid observations.
The last warm run decoded preparation in 60.173 ms, prepared sources in 32.356 ms,
retained checks in 203.292 ms and cleaned up the reader in 0.306 ms. It retained
two source entries, two Build checks and two Product checks. All three native
images were identical and exited zero. This is attribution, not a speedup claim.

Every nonzero `Handle<T>` consults `RecordReader::generation<T>`. The existing
outer HashMap hashes a Rust TypeId for every such handle, including semantic
arena types whose generations are deliberately preserved. The current source
graph registers only the retained-syntax-text remapping domain; the type key is
static decoder metadata, not an untrusted serialized key. A compact registry
can compare the few registered TypeIds directly while preserving each domain's
existing generation map, payload-conflict checks, and failure behavior.

Next hypothesis: remove the repeated outer type hash, retaining explicit generic
domain registration and unchanged inner generation correspondence checks. Use
the newly instrumented producer as the matched baseline for that single change.
