# Repeated integer premise preparation: attribution, no retained optimization

Decision: reject exact duplicate-input removal for this workload; shared guard
premise preparation remains inconclusive. No implementation candidate was built
or timed. The temporary diagnostic patch is removed from production source.

## Fixed experiment

Base `88a0a52d5ea0a7d86df92724e31299ce7e4ccf5f`, Windows x86_64, retained
`contract-query-publication` worktree. The unchanged
`tests/omega/pass/calls/runtime_numeric_operand_order_exit` workload uses a fresh
project copy and independent package cache. The optimized dev producer has
`opt_level=1`; its SHA256 is
`939cd33c33f9b88082e478f81c5e9df97c06f8df023a3947d0a649486c267ff0`.
[Identity](identity.json) binds the producer and diagnostic patch;
[inputs](diagnostic-inputs.json) pin the workload and standard-library sources.
The build/profile chain checked the exact compiler patch before and after running.

The [predeclared research and acceptance](research.md) called for attribution
before selecting a candidate. Its prospective 10% A/B screen was not run.
An earlier query-local operand cache remains rejected and absent; this experiment
does not rehabilitate that patch.

## Observation

The first update reached the expected review boundary (exit 3) in **127.65 s**.
Review resume succeeded in **5.14 s**. These are single instrumented observations,
not speedup measurements. Shared-slot queue wait was about 1,547 seconds; the
optimized build took 4m17s. Neither is included in the update timing. Other
worktrees were active on the host, and the diagnostic itself adds overhead.

| Scope | Inclusive seconds | Calls |
| --- | ---: | ---: |
| Integer predicate observation query | 149.911 | 44,885 |
| Premise projection | 91.279 | 44,264 |
| Premise installation/admission | 42.795 | 44,264 |
| Goal projection | 15.610 | 44,885 |
| Retained comparison copy | 0.087 | 44,264 |

These scopes nest and can re-enter integer judgment through resolution. They
must not be added as disjoint wall time or converted into an achievable speedup.
Projection and installation deserve further investigation; comparison copying
is a poor target on this trace.

The global projection counter observed **787,550 input visits**, with **zero
duplicate input pairs within an individual projection list**. The duplicate
counter is absent from the profiler output because it is incremented only on a
match. Both predicate and truth use `same_resolution_payload`; semantically
equal terms with different captured payloads are not treated as duplicates.
The diagnostic still processes every input. This rules out exact list deduplication
as a useful mechanism here; it does not measure reuse between different queries.

Query-local counters summed 670,349 projected comparisons and 122,341 registered
operations. Across all installation consumers, there were 117,326 fixed-point
rounds and 375,014 operation visits. These populations differ: they are work
counts, not distinct premises, cache misses, or a cross-query cache hit rate.
See [scope totals](diagnostic-scopes.json), [counters](diagnostic-counters.json),
and the [raw first update](cases/diagnostic-runtime_numeric_operand_order_exit/00.txt).

## Correctness and reuse boundary

The actual command was `omega run --both --keep main.omg`, without `--target`.
It observed native exit 0 and Terminal interpreter exit 0 in 9.18 s;
[the raw transcript](cases/diagnostic-runtime_numeric_operand_order_exit/02.txt)
contains both engine results. The emitted native image SHA256 is
`7ccbfb89151d286167c97d51c39092b03e33ccf79762dbd9802205578209eb66`, matching the
prior fixed-source iteration's images. This establishes this diagnostic witness,
not broader compiler acceptance. No refusal suite, unit suite, full corpus,
other-host run, independent candidate review, or production qualification was run.

Source review found a material distinction between the general guard route and
`prepare_integer_premises`. Guards project the goal first and can retain a usable
subset of independently admitted comparisons. Prepared premises require complete
projection and admission before dropping operation worklists. Their substitution,
carrier-bound, and opaque-operation interactions cannot be assumed equivalent.
The general route also supplies admitted EQ/NE premises for tri-valued complement
reasoning; a Boolean positive-proof cache is not a replacement.

Resolution can re-enter under a different active application chain. Existing
application recursion checks compare full payloads plus immutable context owners,
storage epochs and modes. A shared premise cache keyed only by premise count or
rewrite revision would not establish that same validity. No such cache was added.

## Next decisive question

Measure repeated **completed projection requests under the same full resolution
context and active application chain**, separating exclusive projection work from
nested resolver work. Count potential reuse without consuming cached answers;
keep the diagnostic bounded and avoid retaining a judge through its own cache.
Only a substantial measured hit opportunity justifies implementing shared reuse
and running the predeclared uninstrumented A/B and formation/storage/refusal gates.
If exact-context reuse is rare, investigate installation/admission instead.

Replay: apply [diagnostic.patch](diagnostic.patch) to the fixed base, copy
`diagnostic-chain.py` and `diagnostic-profile.py` into
`build/integer-premise-preparation/`, then run the chain under one build-slot hold.
The case directory must be absent so the cache is fresh. The recorded shared mbx
cache path is host-local; the Cargo target belongs to this worktree. Binaries and
caches remain ignored locally; only executed diagnostics and their records are
archived. Unexecuted candidate harness drafts remain scratch files.
