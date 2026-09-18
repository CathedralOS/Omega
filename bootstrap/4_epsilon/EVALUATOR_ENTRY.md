# Epsilon evaluator entry: canonical request and observation envelope

This document defines the evaluator's realized entry boundary for the selected
lower chain and derives its explicit resource, request, and observation
profile. It satisfies the EPSILON-EVALUATOR entry obligation: one explicit
profile, exact and adjacent refusals, and no Epsilon observation on any
resource or transport refusal. The private diagnostic adapter
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
| Gamma evaluator tape `gamma_evaluator_bytecode.tape` | 8,575 | `ad55c3f18d3c7bd3e1189635bf34ff6595ca97c34afe85412a6127ed2d29e015` |
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
Resource refusals publish nothing at this level at all: sealed-input extent,
publication extent, and evaluator storage exhaustion remain the lower chain's
nonzero statuses (253, 254, 252) with empty stdout — no frame, no observation,
no partial stdout.

## Exact resource counters

The entry owns no internal budget; every dynamic resource remains a counter
of the selected lower chain, now charged against the canonical receipt.

| Counter | Owner | Limit | Refusal | Witness |
| --- | --- | ---: | --- | --- |
| complete request | Gamma evaluator | 16,777,216 bytes | status 253, empty stdout | adjacent pinned by [EVALUATOR_PROFILE.md](EVALUATOR_PROFILE.md) and `tests/gamma` boundary gates |
| sealed input | `ConformanceBytesV1` adapter | 4,194,304 bytes | status 253, empty stdout | exact/adjacent executed in [`tests/epsilon/evaluator-entry/`](../../tests/epsilon/evaluator-entry/README.md) |
| EREQ envelope fields | canonical entry | fixed 52-byte header, declared sections, exact end | EEOUT outcome 1 | exact/adjacent executed in the same gate |
| published observation | `ConformanceBytesV1` adapter | 4,194,304 bytes | status 254, empty stdout | adjacent pinned by [`tests/delta/staged-compiler/run.sh`](../../tests/delta/staged-compiler/run.sh) |
| cumulative immutable pairs | Gamma evaluator | 40,265,318 nodes | status 252, empty stdout | exact/adjacent pinned by [`tests/gamma/heap-boundary/`](../../tests/gamma/heap-boundary/README.md); evaluator-level derivation below |
| live call contexts | Gamma evaluator | 256 contexts | status 250, empty stdout | measured property of this exact artifact in the entry gate |
| temporary value stack | Gamma evaluator | 524,288 entries | status 250, empty stdout | dominated by the context bound |
| lexical environment rows | Gamma evaluator | 131,072 rows | status 250, empty stdout | dominated likewise |

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
resource exhaustion mode is pair-arena status 252 with empty stdout, not an
observation.

## Refusal witnesses

Exact and adjacent pairs executed against the canonical receipt by
[`tests/epsilon/evaluator-entry/`](../../tests/epsilon/evaluator-entry/README.md)
on macOS arm64:

| Boundary | Exact (admitted) | Adjacent (refused) |
| --- | --- | --- |
| sealed input extent | 4,194,304-byte EREQ: admitted past the adapter, then EEOUT `trailing_input` at 52 | 4,194,305-byte EREQ: status 253, empty stdout |
| EREQ header | 52-byte header, zero-section request: admitted | 51-byte header: EEOUT `short_header` at coordinate = extent |
| identity | all 8 bytes exact: admitted | each adjacent identity byte: EEOUT `identity_or_reserved` at its offset |
| profile | profile 1: admitted | profiles 0, 2, 2^32-1: EEOUT `unknown_profile` at 8 |
| length high bit | high bits clear: admitted | high bit set: EEOUT `length_high_bit` at 15 or 19 |
| closure identity | bound digest: admitted | each adjacent digest byte: EEOUT `artifact_mismatch` at 20 + index |
| section extents | declared = remaining: admitted | declared one beyond: EEOUT `section_extent` at 12 or 16 |
| exact end | no trailing bytes: admitted | one trailing byte: EEOUT `trailing_input` at first extra offset |

Every refusal in the gate publishes either the exact EEOUT frame above or a
lower-chain nonzero status with empty stdout; none publishes a canonical
observation.

## What remains open

This document discharges the evaluator-entry leg: a realized section-11
envelope, its resource profile, deterministic refusal witnesses, and the rule
that no resource, transport, or request refusal publishes an Epsilon
observation. The envelope binds the exact evaluator artifact, the exact
source closure, the sealed stdin, this profile, and the complete
`RunEpsilon` observation without host parsing or policy, within the construct
coverage the evaluator currently implements. Still open under
EPSILON-EVALUATOR: witnessed checking/runtime conformance gaps, complete D
composition, and independent `RunEpsilon` refinement — section 11's final
acceptance still requires the evaluator to execute every Epsilon construct
and Console effect for the exact D source.
