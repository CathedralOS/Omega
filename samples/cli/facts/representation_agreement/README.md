# Selected representation agreement

These programs prove that two explicitly selected conversion maps preserve both
fields of a value and reject malformed frames. Runtime checks call the maps
through their ordinary fallible interface, then inspect the results independently
of the agreement theorem.

Parent: **LIBRARY-REPRESENTATION-FAMILIES**.

**Selected representation agreement preserves every semantic field through its exact fallible maps and rejects malformed frames.**

## What the programs establish

`Pair` contains two unrestricted `u8` fields. `FirstFrame` stores tag 165 and the
two fields in semantic order. `SecondFrame` stores tag 90 and exchanges their
storage positions. The five checked laws establish successful encoding, encoded
validity, successful decoding of valid frames, rejection of malformed frames,
and exact preservation of both fields on a round trip. The laws cover all
65,536 possible Pairs; they do not rely on enumerating the runtime examples.

The caller cites those laws through the exact selected encoder, decoder and
validity implementation. Each law's premises must hold before its conclusions
become available. Subsequent citations consume those conclusions directly.
A checked, fact-only citation performs no runtime write; an abstract subject
alone cannot invalidate the facts it establishes. Mutable, linear and effectful
calls retain their ordinary ownership and mutation obligations.

Every positive exercises unequal fields 17/42 and endpoints 0/255. The consumer
first cites the agreement laws and observes both conversions through weak
`TryFrom` contracts. It then compares both transported fields with independently
supplied expected scalars. This detects the wrong branch or returned value even
when that wrong value could itself round-trip successfully. Both malformed-frame
decodes must fail. The selected-target program also checks distinct 17/29 markers
from ordinary Build-selected implementations.

The invalid twin changes only the first decoder's last output field to its first
input field. Encoding coverage and invalid-tag rejection remain valid, while the
concrete round-trip law must reject. Its required diagnostic names
`FirstAgreement::round_trip` and `ReadFirst.try_from(frame)`.

These owned three-byte frames need no allocator, external buffer or host service.
Resource-backed conversions and the standard Representation interface remain
part of the parent outcome. The neighboring
[representation_conversion](../representation_conversion/README.md) row tests
endpoint selection; this row tests whole-value agreement between maps with the
same endpoint types.

## Existing witnesses and controls

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [local](local/main.omg) | [corrupted decoder](local/invalid/main.omg) |
| Field | [field](field/main.omg) | [corrupted decoder](field/invalid/main.omg) |
| Parameter | [parameter](parameter/main.omg) | [corrupted decoder](parameter/invalid/main.omg) |
| Call result | [call_result](call_result/main.omg) | [corrupted decoder](call_result/invalid/main.omg) |
| Match join | [match_join](match_join/main.omg) | [corrupted decoder](match_join/invalid/main.omg) |
| Constant | [const_name](const_name/main.omg) | [corrupted decoder](const_name/invalid/main.omg) |
| Nested machine | [other_state](other_state/main.omg) | [corrupted decoder](other_state/invalid/main.omg) |
| Other machine | [other_machine](other_machine/main.omg) | [corrupted decoder](other_machine/invalid/main.omg) |
| Selected implementation | [target_body](target_body/main.omg) | [corrupted decoder](target_body/invalid/main.omg) |

Each project binds `ProgramEntry` for all four hosted targets and declares exit 0.
The nine positive programs and nine decoder-only controls are mirrored under
`tests/omega/{pass,fail}/facts/representation_agreement_through_*`.

## Validation

The current compiler repair and stronger runtime oracles still require their
recorded execution results before this row can claim fresh acceptance. Original
Windows run 42302120700 and Linux ARM64 run 42303719206 rejected all nine valid
callers after losing facts across citations; all nine decoder controls retained
their intended rejection. Neither baseline reached native or Terminal execution.

Historical macOS ARM64 observations belong to the original programs and their
recorded producers, beginning with `8c2fec05b2cabe2eef32c8a9fa714a0f2e8b3513`.
Later program cohorts retained their own `d38849d4cb02`, `c4dff827e4da`,
`7879363aac49` and `4c1f16dc8b68` identities. Those results do not qualify the
new consumer checks. Their raw reports and producer profiles remain in
`/Users/zcanann/Omega-build/representation-agreement-20261101/`.

Current original reports, executable profiles, source bundles, exact input hashes
and complete compressed observations are retained in
`/Users/zcanann/Omega-build/representation-buffered-agreement-20261102/`, with
summaries and requests in `selected-agreement-20261106/`.

The related controls cover missing premises, distinct selected maps, weak public
contracts, mutable and linear custody, non-repeatable results, quotient fences
and private-package access. An existing corruption counterfactual removes the
agreement claim and must produce native/Terminal Trap for a broken decoder.

Canonical rules: [representation conversions](../../../../wiki/spec/layouts/codecs.md#representation-conversions),
[proof contracts](../../../../wiki/spec/proofs/contracts.md#data-identity-in-facts),
[ownership](../../../../wiki/spec/language/ownership.md), and
[transparent refinements](../../../../wiki/spec/language/conformances.md#transparent-refinements).
