# Selected representation contracts

This consumer checks that an owned frame and a caller-owned byte buffer preserve
the same semantic value through selected conversions. Its reject twin supplies
encoder and decoder implementations that always return an error. An independent
echo still proves that it preserves its input; that promise cannot certify those
implementations.

Parent: **LIBRARY-REPRESENTATION-FAMILIES**. This milestone adds the library
interface. The full representation task remains open, including its nine
transport places and host coverage.

`RepresentationEncoding` and `RepresentationDecoding` strengthen the exact
selected `TryFrom` requirements. Encoding must succeed, produce a valid frame
and relate it to the borrowed input value. Decoding must accept valid frames,
reject invalid frames and relate each successful value to its input frame.
`Representation` groups these checked applications and their shared model.
There is no separate round-trip implementation that can certify the encoder or
decoder.

The model queries borrow both the frame and semantic value. Its uniqueness law
establishes semantic identity without cloning either owner or granting custody.
Encoding takes explicit resources; decoding borrows its frame and takes separate
materialization resources. Successful decoding returns its declared resources.
Errors retain their selected types and ordinary ownership obligations.
Neither conversion is required to be a repeatable mathematical function.

The program uses the public guarantees to prove that the decoder's result is
identical to the encoder's input. It also checks runtime values independently:
17 and 255 come from main's literals, and a tag-zero owned frame must fail.
The buffered case writes 42, decodes it, writes through the still-owned frame,
and checks both neighbor bytes. The buffer's ordinary domain requires two bytes;
the interface supplies no allocator, service reach or resource authority.

Broader resource-dependent bounds, consuming/view protocols, representation
families and the parent's full acceptance remain open.

Canonical rules: [representation conversions](../../../../../wiki/spec/layouts/codecs.md#representation-conversions),
[proof contracts](../../../../../wiki/spec/proofs/contracts.md#data-identity-in-facts),
and [ownership](../../../../../wiki/spec/language/ownership.md).
