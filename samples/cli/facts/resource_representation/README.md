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
[invalid twin](local/invalid/main.omg) changes only the first encoder's capacity
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
| [Local](local/main.omg) | Original local request construction | [Guard requires only two bytes](local/invalid/main.omg) |
| [Field](field/main.omg) | A field between nonzero neighbors | [Capacity obligation](field/invalid/main.omg) |
| [Parameter](parameter/main.omg) | A forwarded parameter | [Capacity obligation](parameter/invalid/main.omg) |
| [Call result](call_result/main.omg) | A completed call | [Capacity obligation](call_result/invalid/main.omg) |
| [Match join](match_join/main.omg) | Both arms of a match | [Capacity obligation](match_join/invalid/main.omg) |
| [Constant name](const_name/main.omg) | Two owned constants | [Capacity obligation](const_name/invalid/main.omg) |
| [Nested machine](other_state/main.omg) | An explicit nested machine | [Capacity obligation](other_state/invalid/main.omg) |
| [Other machine](other_machine/main.omg) | Another named implementation | [Capacity obligation](other_machine/invalid/main.omg) |
| [Selected implementation](target_body/main.omg) | An ordinary Build-selected provider | [Capacity obligation](target_body/invalid/main.omg) |

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

## Accepted results

All nine places return **native 0 and Terminal 0 with agreement** on Linux
x86-64. All nine capacity-only twins reject for their intended `emit` requirement.
The eighteen sample projects meet their authored runtime or rejection oracles.
The programs and twins are mirrored under
`tests/omega/{pass,fail}/facts/resource_representation_buffered_*`, with actual
records in `corpus_outcomes.txt` and `corpus_both_linux_x86_64.txt`.

[Run 41520174252](https://github.com/CathedralOS/Omega/actions/runs/41520174252)
observed the frozen local place, exclusive byte-field control and
borrowed-refinement pair using optimized source
`60b8f1171d451eed48cc168832ccfc6275ae2ee2`. It passed five source outcomes,
three native/Terminal agreements and the original two sample projects.
[Run 41522299500](https://github.com/CathedralOS/Omega/actions/runs/41522299500)
observed the remaining eight places and twins using
`6b941db428c6e25d7b2556df3930f8e25d315697`. It passed sixteen source outcomes,
eight native/Terminal agreements and sixteen sample projects. Preparation took
361 seconds, source checking 135 seconds, both engines 73 seconds and samples
181 seconds. Formatter output and removal of temporary workflow diagnostics
do not change these consumers. Final cleanup also removes a redundant
missing-declaration refusal; the successful catalog path is unchanged. Records
retain the producer revisions actually observed.

Four unchanged controls retain their applicable native/Terminal agreements from
[run 41515819925](https://github.com/CathedralOS/Omega/actions/runs/41515819925):
shared partial-case cleanup, qualified bytes, outer references and write-only
arrays. Its historical Terminal declines are superseded by the repaired local
and exclusive-byte observations. Capacity, lifetime, shared-access and stored
backing controls retain their actual rejection diagnostics. Together these
records cover 29 source outcomes and 15 native/Terminal agreements; earlier
result-shadowing controls retain their applicable source observations separately.

The original program, Build files and guard-only twin remain unchanged from
`10fba736d8eb1ed0de892a360d8fb6dba4882599`. The initial baseline refused five
success postconditions. Raw reports, compiler profiles, source bundles and the
evidence ledger remain outside git in
`/Users/zcanann/Omega-build/representation-resource-20261102/`.

The broader resource-backed `Representation` interface remains open, including
its agreement laws, reflection-based adapters, other resources and formats,
other hosts and the Omega-written compiler.
