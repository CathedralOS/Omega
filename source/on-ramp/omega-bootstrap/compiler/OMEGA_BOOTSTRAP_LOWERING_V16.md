# Omega bootstrap lowering envelope version 16

[`CKIR15`](OMEGA_BOOTSTRAP_CHECKED_IR_V15.md) |
[`OMGRSW4`](OMEGA_BOOTSTRAP_RESOLUTION_V4.md)

`OMGLOWG` version 16 is the private producer envelope pairing exact
`OMGRSW4` selector 4, or exact `OMGRSW7` selector 7 when the optional inherited
full-width arithmetic family is used, with CKIR schema major 15. It inherits
the framing and capacity limits of the earlier lowering envelopes. Changing
only the outer magic, version, selector, witness identity, or emitted CKIR
major is invalid. Selector 4 is the least view-only relation; selector 7
composes the already-closed CKIR14 arithmetic custody without requiring an
arithmetic occurrence in every CKIR15 carrier.

This cut selects one machine and the exact generalized shared-byte-view source
relation in CKIR15. The guarded value is one direct exact `&[u8]` machine or
state parameter. Each arm uses direct, pairwise-distinct pass-through binders;
after removing exact `v[0]` and later exact `v[1..]` from the true vector, the
remaining binder identities and order equal the false vector. At least two
guarded occurrences and one pass-through position are required.

Interval replay records those exact authored vectors without allocating CFG
rows. After the fixed point completes, the producer appends one synthetic
block per occurrence in source/block order. Its parameters are exactly
`(v, P...)`; it emits `SliceHead` then `SliceTailOne` and jumps to the authored
true target with the original interleaving. The authored false edge bypasses
the synthetic block. The bounded generalized-view tables share one disjoint,
32,768-word metadata carrier so persisted-Gamma calls thread one array slot
rather than eight independent slots. `StaticByteView` and CKIR14 arithmetic
are optional.

Malformed framing, witness pairing, binder identity/order, target type/arity,
head/tail form, occurrence count, or synthetic custody selects status 251.
Declared resource exhaustion selects status 252. Failure publishes no CKIR.

Persisted-Beta elaboration of the current shared lowerer produces a measured
2,375,541-byte Gamma program. The CKIR15 meaning gate therefore uses a
2,800,000-byte version-local ceiling (with 2,800,001 as the adjacent rejected
tooth); historical meaning modes retain their 2,300,000-byte ceiling.

The product-shaped execution fixtures and no-`StaticByteView`
runtime-parameter carrier remain as frozen inputs. Their producer-dependent
driver was removed with the external Delta producer; replay resumes only after
canonical Delta publication. CKIR12 and CKIR14 remain frozen on their prior
outer identities.
