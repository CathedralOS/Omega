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

## Current result

The original local program now exits 0 natively. Terminal execution still
stops while constructing a mutable byte field, so this place and the complete
row remain unfinished. The other eight places and their reject twins are
authored and await execution.

[Run 41515819925](https://github.com/CathedralOS/Omega/actions/runs/41515819925)
used source `3139db74cf0d37dcb6ba4cbb2c61bc74ded684ce` and an authenticated,
optimized Linux x86-64 compiler. All eleven source outcomes matched: six
positives checked and five invalid controls rejected for their expected reasons.
All six positives exited 0 natively. Four agreed with Terminal at exit 0; the
original program and exclusive byte-field control stopped with
`VerifiedOperationMalformed` during Terminal execution. Sample execution was
not reached.

Compiler preparation took 662 seconds, source checks 75 seconds and native/
Terminal observations 47 seconds. The next repair uses the existing descriptor
transfer check when a canonical descriptor value moves into a stored field.
The operand's actual multiplicity, mutable backing, predicates, extent and
ownership transfer remain mandatory.

The frozen program, Build files and guard-only twin remain unchanged from
`10fba736d8eb1ed0de892a360d8fb6dba4882599`. The earlier baseline
[41489778010](https://github.com/CathedralOS/Omega/actions/runs/41489778010)
rejected the five success proofs; the subsequent return-binding repair removed
those refusals. Raw reports and compiler identities are retained outside git in
`/Users/zcanann/Omega-build/representation-resource-20261102/`.

[PR #1461](https://github.com/CathedralOS/Omega/pull/1461) remains a draft. Its
next step is to finish Terminal execution for the unchanged encoder and run the local
program with both engines, then complete the nine-place row and invalid twins.
No completed place or canonical success record is claimed.

The broader resource-backed `Representation` interface remains open, including
its agreement laws, reflection-based adapters, other resources and formats,
other hosts and the Omega-written compiler.
