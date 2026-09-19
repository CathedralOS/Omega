# Epsilon evaluator entry: canonical request and observation envelope

This document defines the evaluator's realized entry boundary for the selected
lower chain and derives its explicit resource, request, and observation
profile. It satisfies the EPSILON-EVALUATOR entry obligation: one explicit
profile, exact and adjacent refusals, every lower-chain resource refusal
carried to the section-10 outer `Incomplete`, and no Epsilon observation on
any resource or transport refusal. The private diagnostic adapter
(`tests/epsilon/interpreted-omega-experiment/execution_driver.delta`, profiled
in [EVALUATOR_PROFILE.md](EVALUATOR_PROFILE.md)) is not this boundary; it
remains the development transport only.

The evaluator is unchanged: this document's edge wraps the same packed
`epsilon_compiler.delta.sources` closure whose checking and execution the
diagnostic profile records. What changes is the boundary. The diagnostic
adapter owns a private request frame and private observation tags; the
canonical entry owns a versioned request envelope, an artifact binding, an
exact stdin section, a canonical observation grammar, and a refusal frame that
no observation can produce.

## Selected composition

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Gamma evaluator tape `gamma_evaluator_bytecode.tape` | 8,575 | `00c05bedbe0eed665bc165a9165ecf09cd40627bc636034ccb8dbfb24df3919d` |
| packed Epsilon evaluator closure | 617,354 | `4a8c97f9ad8f3ef5bae6c2f9a1c72f3433405e6e79610169b03b03a74217fd8e` |
| canonical entry `evaluator_entry.delta` | 10,950 | `52032438c1236f51095b761afcb3111df2ae2d73ac9be7e91883bbfbd273e5e3` |
| Delta support section | 2,998 | `cfdf07cf8010eba2fd7da47e6936ea1e237f637f4ded5791c272e03096d70255` |
| canonical evaluator receipt | 729,060 | `bec9011e5216557a59ba701ac2a4112774e5f48240c729b95ffc8297f704c368` |

The canonical entry source is
[`tests/epsilon/evaluator-entry/evaluator_entry.delta`](../../tests/epsilon/evaluator-entry/evaluator_entry.delta).
The closure's source inventory owns every `.delta` file under
`bootstrap/4_epsilon/`, so the entry lives beside its boundary gate — the same
placement the diagnostic driver uses — rather than inside the closure it wraps.
The compiled Delta subject is exactly `packed closure bytes ++ entry bytes`;
compiling that subject with the bound Delta support section through the bound
Delta compiler (`DCREQ`, profile 1, `ConformanceBytesV1`) reconstructs exactly
the canonical receipt. `tools/bootstrap/epsilon/evaluator_env.sh` and
`tests/bootstrap/epsilon-identity.sh` bind every identity above; the receipt's
first declaration is `(def $application () Int 1)`, so the Gamma evaluator
classifies it as an application program.

## Request: EREQ v1

The edge consumes one Gamma evaluator request whose sealed input is the
complete EREQ envelope:

```text
0..4    little-endian u32 receipt length = 729,060
4..     exact canonical receipt bytes
...     sealed input = the EREQ v1 envelope:

 0..8    identity 45 45 52 45 51 01 00 00 ("EEREQ", version 1, reserved zero)
 8..12   evaluator profile, little-endian u32; version 1 assigns exactly 1
         (ExactConsoleV1: exact source closure, exact stdin, exact
         observation, no host parsing or policy)
12..16   source closure length, little-endian u32, high bit clear
16..20   sealed stdin length, little-endian u32, high bit clear
20..52   claimed evaluator-closure identity: SHA-256 of the packed
         epsilon_compiler.delta.sources closure this artifact was composed
         from, checked byte-for-byte against the bound identity embedded in
         the entry, so a request prepared for another artifact refuses on
         this artifact rather than evaluating the wrong boundary
52..     exact source closure bytes, then exact stdin bytes, then exact end
```

Validation order is fixed: extent `>= 52`, identity, profile, length high
bits, closure identity, source extent, stdin extent, exact end. A violation
publishes the EEOUT refusal frame below and never reaches the evaluator.

## Publication classes

A status-zero invocation publishes exactly the entry's returned rope, whose
first byte selects one of two disjoint classes — neither class is producible
by the other:

**Canonical observations** (the complete `RunEpsilon` observation):

| Tag | Result | Payload |
| --- | --- | --- |
| `00` | Exit | four-byte little-endian `i32` exit code, then exact stdout |
| `01` | Trap | one closed trap-kind byte, then exact stdout prefix |
| `02` | Reject | one closed rejection-reason byte, then four-byte little-endian source coordinate |

**Refusal frames**, always exactly 40 bytes, identity
`FF 45 45 4F 55 54 01 00` ("EEOUT" version 1) then:

```text
u8  outcome   (1 = request refusal, 3 = internal failure)
u8  space     (1 = request byte coordinate, 2 = internal diagnostic
               coordinate reported by the evaluator)
u16 reserved  (zero)
u32 code      (outcome 1: 1 short_header, 2 identity_or_reserved,
               3 unknown_profile, 4 length_high_bit, 5 artifact_mismatch,
               6 section_extent, 7 trailing_input;
               outcome 3: 1 evaluator_internal)
u64 coordinate, u64 limit, u64 requested, little-endian
```

`coordinate` is the offending request offset for outcome 1 — the header
extent for `short_header`, the first differing byte for
`identity_or_reserved` and `artifact_mismatch`, the field offset for
`unknown_profile`, `length_high_bit`, and `section_extent`, and the first
trailing byte for `trailing_input`. `section_extent` additionally reports
`limit` = bytes remaining for that section and `requested` = the declared
length; all other v1 frames carry zero `limit`/`requested`. For outcome 3,
`coordinate` is the evaluator's reported diagnostic source coordinate.

The layout mirrors the lower chain's `DCOUT` outcome frame so the family
stays machine-distinguishable: a refusal frame's `FF` lead cannot be an
observation tag, and no observation tag can be `FF`.

Internal contradictions — `EpsilonEvaluatorSliceInternal` — publish EEOUT
outcome 3, not an observation: a staging internal is not an Epsilon judgment.

**Incomplete transport outcomes.** The evaluator program cannot intercept a
lower-chain refusal: the generated `ConformanceBytesV1` adapter refuses an
oversized sealed input or output before or after `main` runs, and evaluator
context, stack, or pair exhaustion halts the process outright. These paths
therefore publish no bytes. At the edge boundary a nonzero status with empty
stdout is the section-10 outer `Incomplete(resource, limit, requested,
coordinate?)` — never an Epsilon observation, never an EEOUT frame, and never
a partial stdout. The status and the requester's own submitted extents carry
the fields:

| Status | Resource | Limit | Requested | Coordinate |
| ---: | --- | ---: | --- | --- |
| 250 | evaluator live call contexts | 256 contexts | 257 — the refused context | none |
| 252 | cumulative immutable pair nodes | 3,422,453,760 nodes | 3,422,453,761 — the refused node | none |
| 253 | sealed input bytes | 4,194,304 | the exact submitted sealed-input extent | none |
| 254 | published observation bytes | 4,194,304 | 4,194,305 — the refused byte | none |

For the execution-side counters the transport cannot carry the program's
complete demand, so `requested` reports the refused unit — the least demand
the bound refuses — not a projected total. The sealed-input refusal instead
reports the exact submitted extent, which the requester itself knows. One
status can back several counters; the dominated-counter derivations in the
resource table below name the resource that actually binds in this
composition. The 256-context bound trips before the 524,288-entry value stack
or the 131,072 environment rows, and the adapter's 4,194,304-byte input and
output bounds trip before the evaluator's own 16 MiB transport bounds — a
request beyond 137,363,456 bytes is refused before the adapter sees its input
and classifies as `Incomplete(complete request, 137,363,456, submitted request
extent)` under the same status 253. A changed evaluator or adapter
composition must re-derive which counter behind each status binds first; the
mapping above belongs to this exact receipt.

**Internal failure.** Every other process observation carries section 10's
`InternalFailure`: a nonzero status outside the four named above (248
evaluator internal, 249 authored trap in the evaluator's own Delta code, 132
Alpha VM illegal instruction, or an unassigned value), a status-zero
invocation with empty stdout or an unassigned lead byte, or a nonzero status
with nonempty stdout. None is an Epsilon judgment, a partial observation, or
an EEOUT frame.

The classification is complete and disjoint: every process observation of
this edge is exactly one of a canonical observation, an EEOUT refusal frame,
an `Incomplete` transport outcome, or `InternalFailure`. No evaluator-internal
budget is added — the envelope classifies every named refusal path, so the
fallback clause of the board item does not apply, and no hypothetical
dense-storage sizing pass is introduced.

## Exact resource counters

The entry owns no internal budget; every dynamic resource remains a counter
of the selected lower chain, now charged against the canonical receipt.

| Counter | Owner | Limit | Refusal | Witness |
| --- | --- | ---: | --- | --- |
| complete request | Gamma evaluator | 137,363,456 bytes | `Incomplete` via status 253, empty stdout | adjacent pinned by [EVALUATOR_PROFILE.md](EVALUATOR_PROFILE.md) and `tests/gamma` boundary gates |
| sealed input | `ConformanceBytesV1` adapter | 4,194,304 bytes | `Incomplete` via status 253, empty stdout | exact/adjacent executed in [`tests/epsilon/evaluator-entry/`](../../tests/epsilon/evaluator-entry/README.md) |
| EREQ envelope fields | canonical entry | fixed 52-byte header, declared sections, exact end | EEOUT outcome 1 | exact/adjacent executed in the same gate |
| published observation | `ConformanceBytesV1` adapter | 4,194,304 bytes | `Incomplete` via status 254, empty stdout | adjacent pinned by [`tests/delta/staged-compiler/run.sh`](../../tests/delta/staged-compiler/run.sh) |
| cumulative immutable pairs | Gamma evaluator | 3,422,453,760 nodes | `Incomplete` via status 252, empty stdout | boundary no longer executable in gate time; [`tests/gamma/heap-boundary/`](../../tests/gamma/heap-boundary/README.md) crosses the entire retired V4 arena and [`tests/epsilon/pair-boundary/`](../../tests/epsilon/pair-boundary/README.md) retains the refused fixture for opt-in execution |
| live call contexts | Gamma evaluator | 256 contexts | `Incomplete` via status 250, empty stdout | exact/adjacent executed in the entry gate: 34 nested calls admitted, the 35th refused |
| temporary value stack | Gamma evaluator | 524,288 entries | `Incomplete` via status 250, empty stdout | dominated by the context bound |
| lexical environment rows | Gamma evaluator | 131,072 rows | `Incomplete` via status 250, empty stdout | dominated likewise |

Effective Epsilon-facing extents follow directly: `source + stdin <=
4,194,252` bytes inside one 4,194,304-byte sealed input after the 52-byte
header, and a published `Exit` observation carries at most 4,194,299 stdout
bytes after its five-byte tag/code header.

## Storage account

The evaluator is unchanged, so the diagnostic profile's storage account
carries over verbatim: every Delta value is a node in the evaluator's
cumulative immutable pair arena, sparse runtime storage keeps declared extents
exact without dense allocation (`O(log E)` new nodes per indexed update,
sharing every untouched sibling), and declared bounds and value semantics stay
exact under cumulative per-update allocation. The canonical receipt adds only
the entry's fixed header validation and the stdin section rope — bounded,
header-sized work outside the evaluator's own allocation pattern. The same
sparse-array and repeated-update witnesses apply to this receipt; their
resource exhaustion mode is `Incomplete` via pair-arena status 252 with empty
stdout, not an observation. That mode is executed directly against this
canonical receipt by
[`tests/epsilon/pair-boundary/`](../../tests/epsilon/pair-boundary/README.md):
a bounded sparse-write loop is the admitted control, and its unbounded
counterpart reached the refused 40,265,319th pair node inside the execution
phase — status 252, empty stdout, empty stderr, 4,250.673 seconds on macOS
arm64.

## Refusal witnesses

Exact and adjacent pairs executed against the canonical receipt by
[`tests/epsilon/evaluator-entry/`](../../tests/epsilon/evaluator-entry/README.md)
on macOS arm64:

| Boundary | Exact (admitted) | Adjacent (refused) |
| --- | --- | --- |
| sealed input extent | 4,194,304-byte EREQ: admitted past the adapter, then EEOUT `trailing_input` at 52 | 4,194,305-byte EREQ: `Incomplete` via status 253, empty stdout |
| EREQ header | 52-byte header, zero-section request: admitted | 51-byte header: EEOUT `short_header` at coordinate = extent |
| identity | all 8 bytes exact: admitted | each adjacent identity byte: EEOUT `identity_or_reserved` at its offset |
| profile | profile 1: admitted | profiles 0, 2, 2^32-1: EEOUT `unknown_profile` at 8 |
| length high bit | high bits clear: admitted | high bit set: EEOUT `length_high_bit` at 15 or 19 |
| closure identity | bound digest: admitted | each adjacent digest byte: EEOUT `artifact_mismatch` at 20 + index |
| section extents | declared = remaining: admitted | declared one beyond: EEOUT `section_extent` at 12 or 16 |
| exact end | no trailing bytes: admitted | one trailing byte: EEOUT `trailing_input` at first extra offset |
| live call contexts | 34 nested non-tail machine calls: `Exit` 34, stdout `A` | the 35th nested call: `Incomplete` via status 250, empty stdout |

Every refusal in the gate publishes either the exact EEOUT frame above or an
`Incomplete` transport outcome — a lower-chain nonzero status with empty
stdout; none publishes a canonical observation.

## What remains open

This document discharges the evaluator-entry leg: a realized section-11
envelope, its resource profile, deterministic refusal witnesses, the carry of
every lower-chain refusal to the outer `Incomplete`, and the rule that no
resource, transport, or request refusal publishes an Epsilon observation. The
envelope binds the exact evaluator artifact, the exact source closure, the
sealed stdin, this profile, and the complete
`RunEpsilon` observation without host parsing or policy, within the construct
coverage the evaluator currently implements. The last two legs are now
exercised gates on this unchanged composition:
[`tests/epsilon/d-composition/`](../../tests/epsilon/d-composition/README.md)
checks and executes the whole 525,334-byte D closure — all eight manifested
members — through this canonical edge, including D's own
`OmegaScalarCompiler::compile` publishing the exact emitted Alpha tape, and
records the checking-versus-execution allocation split (8e63b21300), while
[`tests/epsilon/refinement/`](../../tests/epsilon/refinement/README.md)
refines `RunEpsilon` over the exact D closure member sources: the
contract-derived model's observations agree with this edge byte-for-byte on
the same eight customers, with discriminating member mutations spanning
every member (fc23c46e4f). What remains under EPSILON-EVALUATOR is the
standing conformance clause: witnessed checking/runtime defects are
corrected when a D slice or contract control witnesses one, and none is
recorded.
