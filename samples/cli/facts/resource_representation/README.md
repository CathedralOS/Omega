# Buffered representation contracts

Fact: **Selected buffered conversions preserve caller output custody and establish a destination only with sufficient capacity.**

This advances the explicit-resource and failure-contract requirement of
**LIBRARY-REPRESENTATION-FAMILIES** under [codecs](../../../../wiki/spec/layouts/codecs.md).
The nearest completed row is [representation_agreement](../representation_agreement/README.md):
its pure maps return fixed owned frames. This row instead transfers an exclusive
caller-supplied byte slice through ordinary requests, successful frame carriers,
and rejected outcomes. No allocator or hidden resource is available.

The ordinary `BufferedEncoding` contract takes the complete source type, an
explicit pure capacity query, and the required capacity. It inherits the exact
`TryFrom` application and adds a sufficient-output operation. That operation promises
success for every semantic value under the declared capacity bound and delegates
to the same fallible map. It does not impose purity or totality on base `TryFrom`.
The two authored formats use different tags and field orders. Core `compose`
decodes the first through `Pair` and reuses the supplied output for the second.

[Local](local/main.omg) observes unequal fields and byte endpoints, both output
formats, insufficient capacity, malformed input, unchanged failure storage, and
nonzero neighboring canaries. Its [twin](local/invalid/main.omg) changes only the
first encoder's capacity guard from three bytes to two. The body and its exact
write precondition remain unchanged.

The sufficient-output guarantee is the static coverage obligation. Ordinary
loans retain the source identity, lifetime, exclusive access, and returned
resource; matching bytes create no new authority. Failure returns the original
slice, exposes no destination, and does not invoke the second conversion.
The unrestricted semantic Pair may be discarded; this does not promise to
return it as owned failure payload.
Runtime observation receives the actual selected maps, not a post-hoc theorem
that could make a corrupt result pass.

This is the first place, not a completed resource-backed Representation row or
the general reflection-specialized standard. Allocator resources, external
services, work sponsorship, arbitrary schemas, full round-trip agreement laws,
optional total forwarding beyond this declared capacity contract, remaining
places, other hosts, and Product remain open.

## Current evidence

Development diagnostics refused before checking on retained optimized Mac producer
`b89a9a700f56c93f9f7e39da3af38b783eea911f` (CLI
`c505b5b8172f83b5021ba8868040a16e5b76a168cc84d1adf9914d2efbe33c32`):
named implementation `satisfies` arguments rejected the literal capacity,
and trait headers did not intake an ordinary machine binder contract. The
repairs reuse the existing type/domain static-argument reader and ordinary
machine-contract intake, retaining recipes, full source types and supplier
identity for resolution and formation. Earlier draft refinement syntax and
private named-capacity diagnostics are retained separately; none qualifies
the final program.
Scoped source/native/Terminal qualification is pending. Raw observations and
frozen inputs are retained outside scratch under
`/Users/zcanann/Omega-build/representation-resource-20261102/`.
