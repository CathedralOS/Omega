# Epsilon evaluator edge profile

This document derives the explicit resource, request, and observation profile
of the currently composed Epsilon evaluator edge against the selected lower
chain: the Beta-authored Gamma evaluator executing a Delta-compiled
`ConformanceBytesV1` receipt under `GammaComposedV2`. It satisfies the
EPSILON-EVALUATOR obligation to name one explicit profile with exact and
adjacent refusals; every refusal below publishes no Epsilon observation.

The profile covers the *diagnostic* edge: the private tagged result produced
by the bound execution driver. The realized section-11 request and
observation envelope is now
[`EVALUATOR_ENTRY.md`](EVALUATOR_ENTRY.md): the same packed closure plus the
bound canonical entry source compiles to the canonical receipt that consumes
the EREQ envelope and publishes canonical observations or EEOUT refusal
frames. This document remains the diagnostic edge's record; the entry
document carries the canonical profile. Nothing in either grants an Epsilon
judgment beyond the evaluator's checked coverage, adds an Epsilon language
bound, or restores an Epsilon Alpha backend.

## Selected composition

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Gamma evaluator tape `gamma_evaluator_bytecode.tape` | 8,575 | `ad55c3f18d3c7bd3e1189635bf34ff6595ca97c34afe85412a6127ed2d29e015` |
| packed Epsilon evaluator closure | 617,354 | `4a8c97f9ad8f3ef5bae6c2f9a1c72f3433405e6e79610169b03b03a74217fd8e` |
| execution driver `execution_driver.delta` | 2,565 | `ba509602e6873117e59ffc544ada6c8aa16e20b08311e69a01b7cb3897199b38` |
| Delta support section | 2,998 | `cfdf07cf8010eba2fd7da47e6936ea1e237f637f4ded5791c272e03096d70255` |
| reconstructed evaluator receipt | 721,484 | `71a016f53f63501760e3a10632d86c9561aa0e8387b794b074d98ce98a823082` |

Compilation of the packed evaluator plus driver plus the bound support
section through the bound Delta compiler (`DCREQ`, profile 1) reconstructs
exactly the receipt; [`tools/bootstrap/epsilon/evaluator_env.sh`](../../tools/bootstrap/epsilon/evaluator_env.sh)
and [`tests/bootstrap/epsilon-identity.sh`](../../tests/bootstrap/epsilon-identity.sh)
bind every identity above. On macOS
arm64 the reconstruction took 222.770 seconds at the measured revision; that
is a host measurement, not a profile bound.

The receipt's first declaration is `(def $application () Int 1)`, so the
Gamma evaluator classifies it as an application program: its failures map to
the generated-program status block, and its published bytes are exactly its
declared result buffer.

## Request

The edge consumes one Gamma evaluator request:

```text
0..4    little-endian u32 receipt length = 721,484
4..     exact receipt bytes
...     sealed input = the private driver frame:

        0..4    little-endian u32 declared source length
        4..     exact Epsilon source bytes
        ...     all remaining bytes as sealed stdin
```

The evaluator realizes the receipt's sealed input as a lazy tag-3 rope view;
the driver validates `frame >= 4` bytes and `declared length <= frame - 4`,
then forms a checked `EpsilonSourceView` over `frame[4 .. 4 + length)` and
constructs the balanced stdin rope over `frame[4 + length .. end)`. A frame
that fails either check produces the private `MalformedRequest` tag `05` -
an ordinary published diagnostic result of the private adapter contract, not
an Epsilon rejection, trap, or resource verdict.

## Observation

A status-zero invocation publishes exactly the driver's returned rope:

| Tag | Result | Payload |
| --- | --- | --- |
| `00` | Exit | four-byte little-endian signed `i32` exit code, then exact stdout |
| `01` | Trap | one closed trap-kind byte, then exact stdout prefix |
| `02` | Reject | one closed rejection-reason byte, then four-byte source offset |
| `03` | Internal | four-byte source offset |
| `05` | MalformedRequest | none |

Every refusal below returns a nonzero process status with empty stdout: no
tagged result, no partial stdout, and therefore no Epsilon observation of
any kind. Divergence is actual evaluator nontermination; the selected
profile has no observation-step counter, consistent with section 11.

## Exact resource counters

The evaluator owns no internal budget today; every dynamic resource is a
counter of the selected lower chain, consumed by the running receipt. Each
counter names its owner, limit, refusal observation, and witness.

| Counter | Owner | Limit | Refusal | Witness |
| --- | --- | ---: | --- | --- |
| complete request | Gamma evaluator | 16,777,216 bytes | status 253, empty stdout | adjacent executed: a 16,777,217-byte request refuses in 4.46 s |
| sealed input | `ConformanceBytesV1` adapter | 4,194,304 bytes | status 253, empty stdout | exact/adjacent executed below |
| driver frame fields | private driver | `frame >= 4`, `length <= frame - 4` | tag `05` | exact/adjacent executed below |
| published observation | `ConformanceBytesV1` adapter | 4,194,304 bytes | status 254, empty stdout | adjacent refusal pinned by [`tests/delta/staged-compiler/run.sh`](../../tests/delta/staged-compiler/run.sh) (`bytes_concat` doubling over a 2,097,153-byte input) |
| cumulative immutable pairs | Gamma evaluator | 40,265,318 nodes | status 252, empty stdout | exact/adjacent pinned by [`tests/gamma/heap-boundary/`](../../tests/gamma/heap-boundary/README.md); evaluator-level exhaustion is derived below |
| live call contexts | Gamma evaluator | 256 contexts | status 250, empty stdout | exact/adjacent executed below |
| temporary value stack | Gamma evaluator | 524,288 entries | status 250, empty stdout | dominated by the context bound in this composition |
| lexical environment rows | Gamma evaluator | 131,072 rows | status 250, empty stdout | dominated likewise |
| nested source lists | Gamma evaluator census | 255 levels | status 3 at census, before the marker classifies | fixed property of the admitted receipt source |
| buffered output | Gamma evaluator | 16,777,212 bytes | status 254, empty stdout | unreachable here: the adapter's tighter output bound always refuses first |

Two bounds therefore dominate this composition by construction:

- The adapter's 4,194,304-byte sealed-input bound binds before the request
  extent can: the largest admitted request is `4 + 721,484 + 4,194,304 =
  4,919,792` bytes, far below 16,777,216. The request extent refuses only
  requests the adapter would already refuse, so it is a dominated outer
  bound - still pinned by the executed adjacent witness.
- The adapter's 4,194,304-byte output bound binds before the evaluator's
  16,777,212-byte buffered-output limit for the same reason.

Effective Epsilon-facing extents follow directly: `source + stdin <=
4,194,300` bytes, and a published `Exit` observation carries at most
4,194,299 stdout bytes after its five-byte tag/code header.

## Storage account

Under [section 10](LANGUAGE.md#10-resource-classification), the profile
accounts for what the evaluator actually allocates, not for hypothetical
dense storage. The receipt realizes every Delta value - syntax nodes,
checked ledgers, runtime roots, values, snapshots, views, and the stdin and
output ropes - as nodes in the evaluator's cumulative immutable pair arena.
Gamma cannot reclaim pairs, so demand is the sum of checking-phase and
execution-phase allocations.

Sparse runtime storage keeps declared extents exact without dense
allocation:

- `epsilon_runtime_array_index_valid` enforces the declared extent `E in
  1..2147483647` on every access; unwritten indexes read the typed zero home
  and allocate nothing.
- `epsilon_runtime_array_insert` descends at most 31 canonical-midpoint
  partitions, pushing one descent frame per level, then rebuilds exactly the
  visited path and shares every untouched sibling. One indexed update
  therefore allocates `O(log E)` new nodes - a bounded path count
  independent of `E` - plus the value and place-path wrappers. Record fields
  behave the same way over their sparse child list.

Executed workload witnesses (macOS arm64, the bound receipt above):

| Workload | Observation | Time |
| --- | --- | ---: |
| `[i32; 2147483647]`, two writes and seven reads across the extent | `00` + exit 0 + stdout `A` | 1.43 s |
| `[i32; 2147483647]`, 5,001 scattered updates then `values[0] == 0` | `00` + exit 0 + stdout `B` | 151.25 s |
| `[i32; 4]`, 20,001 sequential updates then exact final cells | `00` + exit 0 + stdout `A` | 505.66 s |

The first workload cannot execute under any dense realization - the declared
extent alone would exceed the arena - so its exact result demonstrates the
sparse account. The last two show declared bounds and value semantics
remaining exact under cumulative per-update allocation. Their refusal mode
is pair-arena exhaustion: `status 252` with empty stdout, the same boundary
[`tests/gamma/heap-boundary/`](../../tests/gamma/heap-boundary/README.md)
executes at the whole-node maximum and one
beyond. A direct evaluator-level 252 witness is analytically derivable
(updates times bounded path nodes against 40,265,318) but too slow to pin
inside one session; it remains a recorded pending pin.

## Refusal witnesses

Exact and adjacent pairs executed against the bound receipt on macOS arm64:

| Boundary | Exact (admitted) | Adjacent (refused) |
| --- | --- | --- |
| sealed input extent | `4,194,304`-byte frame: status 0, tagged `02` rejection of the empty declared source (the ~4.19M-byte stdin rope fits the arena) in 664.88 s | `4,194,305`-byte frame: status 253, empty stdout in 1.42 s |
| request extent | 16,777,216-byte request: admitted past the extent check, then the tighter adapter bound refuses (status 253, empty stdout) in 4.62 s | 16,777,217-byte request: status 253, empty stdout in 4.46 s |
| driver source-length field | declared length equal to the remaining frame: executes to `00` + exit 0 + `A` | declared length one beyond the remaining frame: tag `05` |
| live call contexts | 34 nested non-tail machine calls: `00` + exit 34 + `A` | 35 nested calls: status 250, empty stdout, ~1.6 s |

The context boundary is a measured property of this exact artifact: each
Epsilon invocation consumes several evaluator-internal Gamma contexts, so
the 256-context bound admits 34 nested machine calls and refuses the 35th
before any observation. It is not an Epsilon language limit; a different
receipt may shift it.

Refusals inherited from the lower chain rather than re-executed here: the
adapter output extent (254) and authored-trap (249) statuses are pinned by
[`tests/delta/staged-compiler/run.sh`](../../tests/delta/staged-compiler/run.sh);
the pair-arena maximum (252) is pinned
exact/adjacent by [`tests/gamma/heap-boundary/`](../../tests/gamma/heap-boundary/README.md);
evaluator-owned statuses 1,
2, 3, 4, and 248 belong to scalar, pre-application, and unclassified paths
of [`bootstrap/2_gamma/EVALUATOR_PROFILE.md`](../2_gamma/EVALUATOR_PROFILE.md),
and status 132 remains the
Alpha VM's illegal-instruction refusal, never an Epsilon outcome.

## What remains open

This profile discharges the derivation leg only: one explicit
resource/request/observation profile, its counters, deterministic refusal
witnesses, and the rule that every resource or transport refusal publishes
no Epsilon observation. The evaluator-entry leg is now realized separately
in [`EVALUATOR_ENTRY.md`](EVALUATOR_ENTRY.md): a versioned EREQ request
envelope, canonical observation grammar, and EEOUT refusal frame bound to a
canonical receipt, still within the evaluator's current construct coverage.
That document also carries each lower-chain refusal status to the outer
`Incomplete` at the edge boundary; the envelope classifies every named
refusal path, so no evaluator-internal budget layer was needed. Still open
under EPSILON-EVALUATOR: witnessed checking/runtime conformance
gaps, the pending direct evaluator-level status-252 pin, complete D
composition, and independent `RunEpsilon` refinement — section 11's final
acceptance still requires executing every Epsilon construct for the exact D
source.
