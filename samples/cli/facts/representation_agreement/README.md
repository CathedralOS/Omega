# Selected representation agreement

These programs require two explicitly selected conversion maps to preserve both
fields of a value and reject malformed frames. Runtime checks call the maps
through their ordinary fallible interface, then inspect the results independently
of the agreement theorem.

Parent: **LIBRARY-REPRESENTATION-FAMILIES**.

**Selected representation agreement preserves every semantic field through its exact fallible maps and rejects malformed frames.**

## What the programs establish

`Pair` contains two unrestricted `u8` fields. `FirstFrame` stores tag 165 and the
two fields in semantic order. `SecondFrame` stores tag 90 and exchanges their
storage positions. The five laws require successful encoding, encoded validity,
successful decoding of valid frames, rejection of malformed frames, and exact
preservation of both fields on a round trip. Their implementations must prove
those statements for all 65,536 possible Pairs; the runtime examples do not replace that proof.

The identity claims use `core::Identity<T>(left, right)`, the specified
mathematical identity declaration. They name the complete fallible result, so
both its case and payload must agree. One consumer uses the same claim through a
transparent mathematical wrapper. The compiler retains each formed identity
value with its exact source carrier and subjects through checked application,
substitution and erasure. Canonical receiving rechecks the kernel formation;
a separate source-bound inspection checks its correspondence with independently
retained source judgments. Forming the statement does not prove it.

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

Linux x86_64 and macOS ARM64 coverage is complete for these conversion examples.
All nine positive places have native and Psi exit 0 observations on each host,
and every decoder-only twin retains its intended rejection. The Linux evidence
combines the four-place batch with applicable retained observations of unchanged
programs; the Mac evidence comes from one batch of all eighteen projects:

| Host | Places | Native/Psi observation | Actual producer source |
| --- | --- | --- | --- |
| Linux x86_64 | Local | [42468771575](https://github.com/CathedralOS/Omega/actions/runs/42468771575) | `089c0a150e63` |
| Linux x86_64 | Field | [42507062255](https://github.com/CathedralOS/Omega/actions/runs/42507062255) | `6c7cdf842b2d` |
| Linux x86_64 | Call result, match join, selected implementation | [42475689833](https://github.com/CathedralOS/Omega/actions/runs/42475689833) | `26c39d0e6335` |
| Linux x86_64 | Parameter, constant, nested machine, other machine | [42510516060](https://github.com/CathedralOS/Omega/actions/runs/42510516060) | `18b2e57640ba` |
| macOS ARM64 | All nine places | [42519412095](https://github.com/CathedralOS/Omega/actions/runs/42519412095) | `3ec75ded8c55` |

The four-place Linux batch also checks its four corrupted-decoder projects. Each
rejects at
`FirstAgreement::round_trip`, naming `ReadFirst.try_from(frame)`. The other five
twins retain their intended checking observations from
[42468758795](https://github.com/CathedralOS/Omega/actions/runs/42468758795),
source `089c0a150e63`, and
[42475689833](https://github.com/CathedralOS/Omega/actions/runs/42475689833),
source `26c39d0e6335`. Historical observations keep those original revisions;
they are not recorded as fresh executions on the latest producer. The Mac batch
checks all nine corrupted-decoder projects against those same complete diagnostic
fragments. Its captured Rust host is `aarch64-apple-darwin`, and its producer
metadata identifies `macos_arm64`.

The frozen programs and oracles are unchanged. Optimized producer profiles,
exact source snapshots and original artifacts are retained for each run. The
README records host coverage milestones of the repair landed in
[PR #2045](https://github.com/CathedralOS/Omega/pull/2045).

Matching-host execution on Windows x86_64 and Linux ARM64 remains open.
The broader LIBRARY-REPRESENTATION-FAMILIES acceptance also remains open.
Earlier legacy-source executions do not close those requirements.

Current original reports, executable profiles, source bundles, exact input hashes
and complete compressed observations are retained in
`/Users/zcanann/Omega-build/representation-buffered-agreement-20261102/`, with
summaries and requests in `selected-agreement-20261106/`.
The later field and four-place records are in `field-continuation-20261108/`
and `linux-remainder-20261108/` under that same evidence directory. The Mac
request and results are in `macos-agreement-20261108/`.

The related controls cover missing premises, distinct selected maps, weak public
contracts, mutable and linear custody, non-repeatable results, quotient fences
and private-package access. An existing corruption counterfactual removes the
agreement claim and must produce native/Psi Trap for a broken decoder.

Canonical rules: [representation conversions](../../../../wiki/spec/layouts/codecs.md#representation-conversions),
[proof contracts](../../../../wiki/spec/proofs/contracts.md#data-identity-in-facts),
[ownership](../../../../wiki/spec/language/ownership.md), and
[transparent refinements](../../../../wiki/spec/language/conformances.md#transparent-refinements).
