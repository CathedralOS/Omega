# Borrowed opaque component exchange

Owning task: **NORMALIZED-ABI-LOWERING**, retaining the borrowed/view-backed
input acceptance of **OPAQUE-BY-VALUE-BOUNDARY-ABI**. This is a shared descriptor
crossing a real independent component boundary, distinct from an owned Copy
argument. The governing contracts are [structural access](../../../../wiki/spec/terminal-psi/structural_access.md),
[loans](../../../../wiki/spec/terminal-psi/loans.md),
[foreign storage](../../../../wiki/spec/build/foreign_storage.md), and
[opaque representations](../../../../wiki/spec/build/opaque_representations.md).

A call-scoped shared view preserves its exact opaque child across an independent component while the original remains usable and sibling host 513 stays distinct.

The original `AddressBox` stores an established Copy opaque address with host
66051. A second box stores host 513. Each place transports a shared view of the
first box into the same consuming operation. The checked independent provider
copies the opaque child through the permitted ordinary field read and passes
that Copy occurrence to libc `inet_lnaof`. The original is observed again and
remains usable after the independent call; the sibling stays distinct. Actual
foreign inputs, closed nested C representation and scalar observations remain
unchanged across places.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [exclusive demand](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [exclusive demand](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [exclusive demand](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [exclusive demand](call_result/invalid/main.omg) |
| Match join | [both arms](match_join/main.omg) | [exclusive demand](match_join/invalid/main.omg) |
| Const name | [named construction inputs](const_name/main.omg) | [exclusive demand](const_name/invalid/main.omg) |
| Nested machine | [explicit arrivals](other_state/main.omg) | [exclusive demand](other_state/invalid/main.omg) |
| Other machine | [relay](other_machine/main.omg) | [exclusive demand](other_machine/invalid/main.omg) |
| Target body | [four checked bodies](target_body/main.omg) | [exclusive demand](target_body/invalid/main.omg) |

The positive and twin consumers are identical. The twin's valid producer changes
only the bridge's host formal and matching implementation from `&AddressBox` to
`&mut AddressBox`; the caller still lends shared access. It must reject with the
exclusive-demand and immutable-access diagnostic, rather than malformed producer
formation. The operation and payloads remain unchanged. Positives and twins are
pinned under `tests/omega/{pass,fail}/facts/opaque_borrowed_carrier_through_*`.
The const place names runtime foreign construction inputs; it does not claim a
borrow of foreign-created runtime storage is a compile-time constant.

The [identity control](identity/main.omg) additionally compares caller and
independent-provider `address_of` observations for both the original box and its
opaque child. The provider reports those inert addresses through two ordinary
checked scalar results; they grant no reference or lifetime. Its unchanged
consuming operation and [exclusive-demand twin](identity/invalid/main.omg)
retain the same fact. The initial frozen compiler checks this control but
native admission refuses its frameless checked leaf with “same-stack admission
has zero WCSU”; that prerequisite and the original-view namespace/path join are
part of the same unfinished acceptance. The corresponding native corpus oracle
requires all four actual component exports.

The [overlapping-view control](overlapping_views/main.omg) passes a shared pair
and views of both of its fields to the same independent invocation. Both fields
contain legitimate Copy occurrences with equal opaque host tokens. The provider
must observe the original field addresses as equal to their corresponding views
and distinct from each other. This prevents equal immutable payloads from hiding
an alias-coordinate collision. Its [twin](overlapping_views/invalid/main.omg)
retains the same immutable-access rejection. The frozen baseline refuses the
positive earlier, while decoding a complete derived component identity longer
than 1024 bytes; that serialization prerequisite remains part of this witness.

The required establishment and identity are the actual caller's live root,
selected path and checked shared loan. Copying an opaque child is permitted by
its checked Copy property; copying a pointer or layout cannot grant access or
duplicate the caller's owner. A cross-instance view must retain the original
owner namespace, backing and catalog correspondence through suspension and
return acknowledgement. Caller claims/frontier remain caller-owned. A fresh
owned referent snapshot or public entry zero initialization cannot stand in for
that view. Existing read/write authority, carry, claim and lifetime checks still
apply, including to stronger actual memberships.

The source-to-Terminal repair must preserve those carriers and their actual
backing. Native calling plans already pass the original address word into the
checked provider; native layout, target realization and emission remain covered
by actual execution rather than compilation alone. The same selected opaque
application governs the provider's later by-value child copy. This row grants
neither persistent retention nor independent mutable access.

Before any borrowed-path implementation, the frozen optimized
`b1b40166d772a4f8423edec2100bb8f1a65c5c70` compiler checks all nine positives and
executes each natively at 0 on Windows x86-64. Terminal refuses all nine at
`component input lacks exact owned closed Inert type/custody`. All nine twins
reject because the caller lends only immutable access to an `&mut` formal.
Exact source hashes, producer profile and raw observations are retained under
the owning claim's `build/opaque-copy/borrowed-b1-baseline/` capture. The initial
smaller same-fact witness is retained in `borrowed-component-baseline-v1/`.

All positive entries bind the four hosted targets and declare exit 0. The row
at `e2d769a35bd6` now executes all nine places and the original/child address
control with native 0 and Terminal 0 on Windows x86-64; all eleven exclusive-demand
controls reject for the intended immutable loan. The additional overlapping-view
positive still refuses at description decoding, and matched fixture gates and
final independent execution review remain unfinished. Actual binary profile,
hashes and raw observations are retained in
`build/opaque-copy/borrowed-e2d769a35bd6-20261025T221119Z/`.

The implementation gives each imported view a private reference-descriptor home;
it retains the original owner namespace, root and path for address observations.
The descriptor home never becomes another owned referent or opaque C token.
Source-only independent review found and then verified the correction of an
overlapping root/subview coordinate collision before this execution capture.

Other-host execution, general nested-machine migration,
publication, remaining descriptor lanes, edge drift and retained outbound backing
remain open parent acceptance.
