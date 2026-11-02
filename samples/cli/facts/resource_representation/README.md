# Buffered representation contracts

Fact: **Selected buffered conversions preserve caller output custody and establish a destination only with sufficient capacity.**

This advances the explicit-resource and failure-contract requirement of
**LIBRARY-REPRESENTATION-FAMILIES** under [codecs](../../../../wiki/spec/layouts/codecs.md).
The nearest completed row is [representation_agreement](../representation_agreement/README.md):
its pure maps return fixed owned frames. This row instead transfers an exclusive
caller-supplied byte slice through ordinary requests, successful frame carriers,
and rejected outcomes. No allocator or hidden resource is available.

The ordinary `BufferedEncoding` contract names its explicit `Request` resource
carrier, semantic value type, result/error carriers and required capacity. It
inherits the exact `TryFrom` application and adds a sufficient-output operation.
The public precondition reads the request's output length. That operation promises
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
places, other hosts, arbitrary-source resource metrics, and Product remain open.

## Current evidence

Development diagnostics and failed producers are retained separately. The first
prototype used an abstract borrowed `Capacity(&Source)` in a specification. Its
signature promised termination, but no stable observation-free denotation;
selecting one stronger private body could not establish that public promise.
The request-specific contract is the spec-permitted explicit-resource form,
with the same Pair values, capacity three, exclusive slice, maps, failure
custody and runtime oracle. Its twin still changes only guard three to two.
The broader arbitrary-source metric abstraction remains open.

The previous source `83369dc50b5d8be5ee8fc074c20a53a352e6a1d2` authenticated an
opt1 Linux producer in run41468881925, cleared callback tuple/region formation,
and then refused at the denotation and returned-loan obligations. That result
does not qualify this corrected source. Scoped source/native/Terminal
qualification is pending. Raw observations and frozen inputs are retained
outside scratch under `/Users/zcanann/Omega-build/representation-resource-20261102/`.
