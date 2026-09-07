# Derivation-checker request envelope

This defines the outer admission layer for the
[ground equality checker design](../../../wiki/architecture/bootstrap_chain/derivation_calculus.md).
The [inner theory, proposition, and certificate formats](FORMAT.md) have a
separate [physical layout traversal](LAYOUT.md) and
[conservative theory formation](FORMATION.md), followed by
[ground-term validation](GROUND.md). [Structural comparison](COMPARISON.md)
operates on that validated input. Checked template substitution, derivation
checking, and the complete proof checker remain unimplemented.
**Framed is not proof acceptance.** No canonical checker
entry or accepted artifact is supplied by this layer.

## Byte framing

All offsets are relative to one immutable Gamma sealed input, not to the Gamma
source or its four-byte evaluator frame. The envelope is:

| Offset | Width | Meaning |
| ---: | ---: | --- |
| 0 | 8 | `47 44 52 45 51 01 00 00` (`GDREQ`, version 1, reserved zero bytes) |
| 8 | 4 | Theory-section byte length |
| 12 | 4 | Independently reconstructed proposition-section byte length |
| 16 | 4 | Certificate-section byte length |
| 20 | 4 | Reserved, all zero |
| 24 | theory length | Exact theory bytes |
| next | proposition length | Exact proposition bytes |
| next | certificate length | Exact certificate bytes, followed by exact end |

Lengths are little-endian unsigned words with the high bit clear. They are
administrative extents in `0..2^31-1`, not numeric values in the proof theory.
Empty sections are physically framed; their eventual inner decoders must decide
whether they are valid. Unknown inner tags cannot be accepted on the strength of
outer framing.

The caller responsible for the artifact independently supplies the theory and
proposition sections. A certificate producer supplies only the third section.
Concatenation alone provides no authentication or subject authority. The later
artifact owner must retain and verify the exact first two sections before
interpreting a generic checker result as evidence about that artifact; a
certificate-provided digest cannot replace this ownership check.

## Deterministic admission

`admit_derivation_request()` reads the current sealed input and returns a private
owned outcome without writing bytes. Checks occur in this order:

1. A request shorter than 24 bytes is rejected at its extent.
2. Check the eight identity bytes in increasing offset order, then the reserved
   bytes at 20 through 23 in increasing order.
3. Check the high length bytes at 11, 15, and 19 in that order before decoding.
4. Compare each section length against its remaining input extent, in section
   order. Only a fitting section advances the cursor. Then require exact end.
5. Apply the request-byte provision to the structurally exact whole envelope.

Section ends are computed only after the subtractive remaining-extent check.
No declared length controls input consumption, allocation, or an unchecked
sum. A short request claiming a huge section is malformed, not a capacity
refusal. The envelope does not read section contents or allocate a row per
section byte.

The private outcomes are:

| Tag | Payload |
| ---: | --- |
| 0 Framed | Three ordered section-end offsets; the first start is 24 and each later start is the preceding end. |
| 1 Rejected | Rejection code, request-byte coordinate, zero limit, zero requested. |
| 2 Incomplete | Resource code, request-byte coordinate, limit, requested. |

Rejection codes are 1 `short_header` (coordinate input extent), 2
`identity_or_reserved` (first offending byte), 3 `length_high_bit` (offending high
byte), 4 `section_extent` (the section's length-field start), and 5
`trailing_input` (first trailing byte). Resource code 1 is `request_bytes`:
coordinate and limit 8,388,608; requested is the exact sealed-input length.
These are admission-layer outcomes, not compiler-boundary or proof judgments.

The initial provision is 8 MiB, leaving room within the selected Gamma
evaluator's 16 MiB request for checker source and framing. This is a private
implementation provision, adjustable with measured certificate requirements;
it does not restrict the calculus. Its exact and adjacent extents are tested.
The future complete checker must publish and validate its source-size and
underlying-evaluator requirements as well as its own full resource profile.
An outer evaluator refusal, trap, or host timeout is not an admission result.

## Private representation

An outcome is `(pair tag payload)`. A framed payload is
`(pair theory_end (pair proposition_end certificate_end))`; a failure payload
is `(pair code (pair coordinate (pair limit requested)))`. Named accessors own
these projections. Only tag 0 permits reading section ends. Failure outcomes
do not preserve earlier partial section cursors as valid custody.

The production entrance and helpers live in `implementation/`, selected by
`implementation.gamma.sources`. Tests provide their own explicit diagnostic
entry; no host script extracts, parses, or replaces production functions.
There is no production proof-accepting `main` until theory formation, derivation
checking, exact-root comparison, and complete failure handling exist.
