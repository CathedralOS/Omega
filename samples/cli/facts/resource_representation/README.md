# Conversions that use a caller's buffer

These examples encode a pair of bytes in two formats using storage supplied by
the caller. They check that enough space guarantees success and that failure
returns the buffer without exposing an invalid result.

Fact: **Selected buffered conversions preserve caller output custody and establish a destination only with sufficient capacity.**

This is part of **LIBRARY-REPRESENTATION-FAMILIES** under the
[conversion contract](../../../../wiki/spec/layouts/codecs.md).
The nearest completed row is [representation_agreement](../representation_agreement/README.md),
whose conversions return fixed owned frames. This row instead passes an exclusive
borrow of the caller's byte buffer through requests, successful results and
failures. It has no hidden allocator or service.

`BufferedEncoding` adds a sufficient-space operation to the ordinary fallible
`TryFrom` interface. The operation requires an output length of at least three
bytes and promises an `Ok` result for every `Pair`. It calls the same conversion
implementation; it does not make all `TryFrom` operations pure or infallible.

The two formats use different tags and field orders. `compose` decodes the first
into a `Pair`, then writes the second using the same buffer. Failure returns
that buffer and prevents the second conversion from running. A matching byte
sequence supplies no new permission to access storage.

The [local program](local/main.omg) checks unequal field values, byte endpoints,
both formats, short output, malformed input and unchanged failure storage. It
also checks nonzero canaries around each buffer. Its
[invalid twin](invalid_local/main.omg) changes only the first encoder's capacity
guard from three bytes to two. The write helper still requires three bytes, so
the twin must reject at that call.

The public capacity requirement establishes the success fact before conversion.
Calls carry the exact buffer, its lifetime and its exclusive access. The result
checks use the actual selected implementations. No caller guard supplies a
missing success proof after the conversion has finished.

## Places

Each place supplies the same semantic `Pair` to these buffered conversions. The
buffer is always a runtime input with exclusive access; a constant supplies a
`Pair`, never permission to access the buffer. Every program keeps the same
three-byte requirement and checks both ordinary and endpoint values, short
output, malformed input and buffer neighbors.

| Place | Semantic input | Reject twin |
| --- | --- | --- |
| [Local](local/main.omg) | Original local request construction | [Guard requires only two bytes](invalid_local/main.omg) |
| [Field](field/main.omg) | A field between nonzero neighbors | [Capacity obligation](invalid_field/main.omg) |
| [Parameter](parameter/main.omg) | A forwarded parameter | [Capacity obligation](invalid_parameter/main.omg) |
| [Call result](call_result/main.omg) | A completed call | [Capacity obligation](invalid_call_result/main.omg) |
| [Match join](match_join/main.omg) | Both arms of a match | [Capacity obligation](invalid_match_join/main.omg) |
| [Constant name](const_name/main.omg) | Two owned constants | [Capacity obligation](invalid_const_name/main.omg) |
| [Nested machine](other_state/main.omg) | An explicit nested machine | [Capacity obligation](invalid_other_state/main.omg) |
| [Other machine](other_machine/main.omg) | Another named implementation | [Capacity obligation](invalid_other_machine/main.omg) |
| [Selected implementation](target_body/main.omg) | An ordinary Build-selected provider | [Capacity obligation](invalid_target_body/main.omg) |

The selected provider returns its marker together with the semantic input.
Selecting the wrong body, exchanging the two inputs or taking the same match
arm twice makes the program trap. This row does not claim that a borrowed
request can be stored in a constant or that the request itself crosses every
listed boundary.

## Requirement evidence

The caller supplies the buffer and the three-byte capacity requirement. The
selected public method establishes success for every `Pair` before the caller
uses its result. Exact lifetime arguments and the returned occurrence survive
the inherited conversion interface; a parameter named `result` keeps its own
meaning. The capacity-only twins fail at the unchanged write helper's
requirement, rather than at a caller guard.

Requests, frames and failures retain the same exclusive buffer access. Both
formats run for unequal 17/42 and endpoint 0/255 inputs. Short output and a
malformed tag return distinct failures without writing the buffer. The caller
reads storage after each call and checks nonzero neighbors. Wrong tags, swapped
fields, incorrect extents, lost writes or damaged neighboring bytes make the
program trap. Both match arms are exercised with different expected bytes.
The selected provider returns its marker in the same call as the semantic
input, so selecting the wrong body also traps.

Checked lowering retains the slice's current pointer, length and access when
constructing requests and results. Cleanup uses the stored descriptor type and
the exact moved field. Indexed stores capture their view before later operands
run. Native lowering checks the full projected source, including owned records;
Terminal construction uses the same descriptor-transfer rule while checking the
operand's declared type, multiplicity, backing, predicates and extent. Runtime
custody stays explicit in every place.

The [renamed-member control](../../../../tests/omega/pass/traits/requirement_evidence_renamed_regions_exit/main.omg)
keeps a public evidence promise while renaming both invocation lifetimes. Its
[reject twin](../../../../tests/omega/fail/traits/requirement_evidence_wrong_region/main.omg)
changes only the provider bound to the other lifetime. Both bodies leave that
evidence unused, so validation must enforce the public promise independently of
body calls or the one caller's chosen lifetime arguments.

## Accepted results

All nine places return **native 0 and Terminal 0 with agreement** on Linux
x86-64 using the integrated compiler. All nine capacity-only twins reject for
their intended `emit` requirement. The programs and twins are mirrored under
`tests/omega/{pass,fail}/facts/resource_representation_buffered_*`. Their actual
records retain the observed producer revisions.

[Run 41527523198](https://github.com/CathedralOS/Omega/actions/runs/41527523198)
used optimized source `0f36ecb1ab02eefdb07fb1ed8b7728eabf1b8381`. It passed ten
source outcomes, five native/Terminal agreements and six sample projects. These
cover the original place and twin, borrowed refinement, selected-value calls,
inherited lifetime identity, public/default/unused evidence and the unused
stronger-provider reject. Integration changed the Cargo inputs, so compiler
preparation began without a matching cache and took 1,218 seconds. Source
checking took 47 seconds, both engines 34 seconds and samples 58 seconds.

[Run 41531563574](https://github.com/CathedralOS/Omega/actions/runs/41531563574)
used `b9975b628a048654b6cd6f98e38d2e6c65c83512`, with identical compiler and
library inputs. It passed eighteen source outcomes and nine native/Terminal
agreements: the remaining eight places and twins, plus the renamed-member
lifetime/evidence pair. The wrong-region twin rejects at the provider's exact
public evidence promise. Preparation reused the saved cache and took 185
seconds; source checking took 102 seconds and both engines 60 seconds. Together
the integrated runs establish 28 source outcomes and 14 execution agreements.

The last main integration receives focused observations in
[run 41534411326](https://github.com/CathedralOS/Omega/actions/runs/41534411326)
on `b07784918316aaa5e1387326285c4d775517a990`: three source outcomes, two
native/Terminal agreements and four CLI sample projects. The frozen conversion
exercises stores and returned-view retirement; the selected-provider place also
checks current provider preparation and runtime admission. Unchanged lifetime
and evidence admission retain the applicable earlier integrated observations.
Preparation took 823 seconds, source checking 15 seconds, both engines 9 seconds
and samples 24 seconds. The cache fallback restored the prior compiled objects
across the Cargo-input change; ordinary build and producer checks still ran.

All eighteen buffered sample projects meet their authored oracles. The original
and selected-provider pairs have fresh observations above. The remaining
fourteen retain
[run 41522299500](https://github.com/CathedralOS/Omega/actions/runs/41522299500)
on `6b941db428c6e25d7b2556df3930f8e25d315697`; their programs and Build files are
unchanged and match the mirrors executed by the integrated compiler. The fresh
sample projects also exercise current CLI review and execution.

Earlier support records retain their actual observations: shared partial-case
cleanup, qualified bytes, outer references and write-only arrays from
[run 41515819925](https://github.com/CathedralOS/Omega/actions/runs/41515819925),
and exclusive byte-field cleanup from
[run 41520174252](https://github.com/CathedralOS/Omega/actions/runs/41520174252).
The canonical files contain 37 source records and 19 execution agreements for
this change, including these retained controls. Earlier Terminal declines are
historical failures superseded by the repaired observations; result-shadowing
controls retain their separate source observations.

The original program, Build files and guard-only twin remain unchanged from
`10fba736d8eb1ed0de892a360d8fb6dba4882599`. The initial baseline refused five
success postconditions. Raw reports, compiler profiles, source bundles and the
evidence ledger remain outside git in
`/Users/zcanann/Omega-build/representation-resource-20261102/`.

The broader resource-backed `Representation` interface remains open, including
its agreement laws, reflection-based adapters, other resources and formats,
other hosts and the Omega-written compiler.
