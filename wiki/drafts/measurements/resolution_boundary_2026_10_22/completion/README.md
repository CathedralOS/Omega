# Deferred completion storage, 2026-10-23

Body completion already owns a clone of its resolved predecessor. Its final
normalizer then copied the predecessor's normalized statement and type tables
again, replacing and dropping those existing successor copies. Transfer the
successor's tables into the normalizer with `mem::take` instead.

The [source audit](audit.md) establishes the ordering: selected bodies and
new declarations extend declaration storage; normalized-table routing runs after
this append. Provisional machine-equation resolution uses separate lowerers.
The predecessor snapshot, source custody checks, selection identities, append
order and failure transaction remain intact. This changes no parser, syntax-page
or shared-arena implementation. Research reuses the [ownership-transfer
investigation](../normalization/research.md).

## Measured result

The diagnostic producer is based on
`4c23cc3391c2e539d4065563ac95de7b18f2fc67`. It compares the complete successor
against the old normalizer source read directly from that Git revision, and
first asserts that both seeded tables still equal the predecessor. The comparison uses existing
carrier equality, which ignores lookup caches and integer-literal landing metadata;
see the [equality limits](equality-limits.md). Each process then
consumes either the frozen clone-based result or the transfer result.

All **100 comparisons** matched across the two accepted 14-input runs.
Seven programs executed with native exit 0 and Terminal exit 0 in each mode;
the machine-constant fixture checked successfully. All five invalid controls
retained exact diagnostics. No source or oracle was edited.

| Workload | Completion batches | Statement rows no longer copied | Type-reference rows no longer copied |
| --- | ---: | ---: | ---: |
| Length | 2 | 924 | 4,017 |
| Dynamic dispatch | 2 | 924 | 4,015 |
| Nested record | 2 | 924 | 4,031 |
| Returned placed view | 5 | 936 | 8,686 |
| Forwarded placed view | 5 | 936 | 8,706 |
| Nested placed view | 5 | 936 | 8,721 |
| Nested machine equation | 2 | 924 | 4,017 |

These are main-row copy counts, summed across completion batches. A table also
owns auxiliary storage; these counts are neither its byte size nor peak RSS.
The diagnostic clones and compares complete carriers and cannot establish a
speedup. **No whole-compiler timing or memory reduction is claimed.** The full
predecessor clone still serves its transactional contract.

The initial harness expected `computed_constant_initializers` to pass because
it is stored under `tests/omega/pass`. It instead rejected an array-valued
canonical const parameter on the clone-based baseline. The [failed initial
attempt](baseline-attempt-1.json) is retained. Both subsequent modes reproduce
that same diagnostic, with equal complete carriers; this is a known baseline
failure, not passing coverage or a regression caused by this transfer.

## Integration and evidence

The production source integrates main at
`9b001b8f6afa2cdac67fd580033c3560d906a5d5`, which introduces inline integer-literal
storage. That update changes literal consumers but not the completion seed,
normalized table owner, append ordering or final-routing boundary. The historical
100 comparisons retain their original producer identity. Current production
execution and source checks have separate records; historical observations are
not relabeled as current executions.

The regression test completes two bodies in separate batches, checks the
first body's retained statement handles and contents, preserves both predecessor
snapshots, and verifies a refused duplicate completion cannot mutate the
published snapshot. No diagnostic branch or equality walk ships in production.

Evidence: [row counts](summary.json), [baseline](baseline-observations.json),
[transfer](transfer-observations.json), [diagnostic producer](instrumented-producer.json),
[temporary oracle patch](instrumented.patch.gz), [frozen old normalizer](legacy-normalizer.rs.gz),
and [implementation mechanism](candidate.patch.gz). The frozen executable and
build receipts are retained in `build/deferred-completion-storage` in this lane.
[setup.py](setup.py), [build.py](build.py), [freeze.py](freeze.py) and
[capture.py](capture.py) reproduce the diagnostic on its recorded base with a
fresh output directory. Preserve existing producer artifacts rather than
replacing them during replay. The production path uses the candidate source and
runs `capture.py production` against the immutable baseline record.

This is one bounded ITERATION-PERFORMANCE repair. It closes neither that outcome
nor a compiler layer. It demonstrates a resolver-owned storage improvement that
can coexist with the separate layer-0 session.

Scoped source validation on the integrated candidate: `Summary [   0.061s] 30 tests run: 30 passed, 0 skipped`.
Crate-scoped Clippy (including test targets), formatting and patch whitespace
checks pass; existing unrelated warnings remain in the logs.

## Qualified production result

The clean production compiler at `60c36e25e468fe7b10767006f09f385fcb590b2f` integrates main
`05580b165045143fb36176fe424470f53a7d5d51`. Seven frozen native images execute with exit 0; all seven
programs also produce native exit 0 and Terminal exit 0 through `run --both`.
The machine-constant check passes. Five invalid controls and the pre-existing
computed-constant refusal preserve exact baseline diagnostics. Pinned
qualification exits 0 with no capture invalidations.

The 30 passing crate tests and scoped Clippy retain their actual observation at
`db1367c3d94414b765562f5f0db97304c1f7682e`. The later incoming changes are in
downstream checking/lowering and application examples; the tested resolver,
representation, parser/foundation dependencies and build configuration are
unchanged. Production was rebuilt and replayed for those downstream changes.
Formatting and whitespace checks pass. Existing warnings remain in the logs.

The earlier production capture remains under [integrated](integrated/production-observations.json);
the later one is under [production](production/production-observations.json),
with its [producer receipt](production/candidate-producer.json),
[native products](production/products.json) and
[qualification summary](production/qualification-summary.json). The active warm
lane qualification uses the supported history-preserving amendment protocol:
the prior published outcome keeps its immutable receipts, and this outcome has
a distinct check ID and fresh executions. An earlier superseded production
build was interrupted before publishing an executable; its record is retained.

Retain this change for the measured removal of duplicate table copies. The next
performance decision should use a fresh whole-compile profile after upstream
work settles; these row counts do not establish the dominant remaining cost.

Publication integration at `18266fffbe74e01a6c8e2d65c464b815b64fe83e` includes main `840c4f8cf03f9b69a72c222f20e1525be8b8344e`,
including the newer call-result qualification and aggregate-address interpreter
paths. A fresh optimized compiler repeats all 14 outcomes: seven dual-engine
programs, one passing constant check, five refusal controls and the unchanged
computed-constant baseline failure. Seven frozen native images separately exit 0.
The [publication capture](publication/production-observations.json),
[producer](publication/candidate-producer.json) and
[qualification receipt](publication/qualification-summary.json) retain this
execution separately from the earlier producers. No source/oracle changes were
needed. Raw Clippy output is compressed to preserve suggestion whitespace.
