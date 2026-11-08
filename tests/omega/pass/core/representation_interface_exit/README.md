# Selected representation contracts

This consumer uses the public representation interface with an owned frame and
with a caller-owned byte buffer. It checks the restored semantic value, rejects
an invalid owned frame, and uses the returned buffer before checking its neighbor
bytes. The decoder-only control changes its returned semantic value to zero.

Parent: **LIBRARY-REPRESENTATION-FAMILIES**. This is one library-interface
milestone. It does not close the existing nine-place rows or their host coverage.

`RepresentationRequest<Value, Resources>` keeps resources explicit. A resource
type's ordinary domain states its admission bounds; here the buffer requires at
least two bytes. Encoding and decoding remain the exact selected `TryFrom`
applications. Their error types remain separate in `RepresentationError`.
The completed `Representation::round_trip` contract promises termination and a successful result
with the same semantic value and the declared returned resource type. It grants
no allocator, service reach or custody beyond those ordinary contracts.

`ValueRepresentation` additionally requires encoding coverage, valid encoded
frames, successful decoding of valid frames, malformed-input rejection and exact
agreement between those selected maps. Its `ValueConversion` bounds permit the
repeated mathematical calls used by those laws. The mutable-buffer example uses
the completed-call base instead and never repeats a mutable conversion in a
mathematical expression.

The positive runs the maps independently of their theorem evidence. Values 17 and
255 come from main's literals, and a tag-zero frame must fail. The buffered case
round-trips 42, writes through the returned owner, and checks both neighbor bytes.
These executable checks complement the universal checked contracts; they do not
replace them. No source comparison grants resource identity or authority.

Broader resource-specific coverage/failure contracts, other representation
families and the parent acceptance remain open.

Canonical rules: [representation conversions](../../../../../wiki/spec/layouts/codecs.md#representation-conversions),
[proof contracts](../../../../../wiki/spec/proofs/contracts.md#data-identity-in-facts),
and [ownership](../../../../../wiki/spec/language/ownership.md).
