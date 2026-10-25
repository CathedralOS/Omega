# Checked opaque copies across an independent component

Parent: **NORMALIZED-ABI-LOWERING**, extending the existing closed-carrier fact.
The source explicitly marks the opaque and its complete inert carrier copyable.
The unchanged consuming operation checks libc host numbers and replaces one
field without disturbing its sibling. The caller then uses its retained copy;
the invalid twin removes only the opaque copy permission in its producer.

**A checked copy of a closed Inert opaque preserves libc host numbers 66051,
513 and replacement 1027 across an independent consuming call while the
original remains usable.**

The [producer](producer/main.omg) declares the opaque and both nested carrier
records `[copy]`. Each consumer selects its representation and the real
`AddressBridgeProvider` with `CompositionMode::Independent`. The copied address
crosses that component's owned input, reaches the actual `inet_lnaof` C call,
and leaves the caller's original address usable after consumption and field
replacement. Guest code cannot inspect the representation or manufacture an
address. Constants name C constructor inputs, not constant foreign results.

| Place | Positive | Copy-only reject |
| --- | --- | --- |
| Local | [local](local/main.omg) | [invalid](local/invalid/main.omg) |
| Field | [field](field/main.omg) | [invalid](field/invalid/main.omg) |
| Parameter | [parameter](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| Call result | [call result](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| Match join | [both arms](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| Const name | [constructor inputs](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| A nested machine | [arrival](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| Another machine | [forwarding](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| Target body | [four bodies](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Each reject changes only `InternetAddress [copy]` to `InternetAddress` in
[its producer](producer_invalid/main.omg). Its consuming operation remains
identical and rejects reuse after transfer. The nine pairs are pinned under
`tests/omega/{pass,fail}/facts/opaque_independent_copy_through_*` with ordinary
self-contained package dependencies and all four hosted entry bindings.

Fresh published main `4b80a922ef2f` accepted the source and ran the original
root natively at zero; Terminal failed because the inet adapter required an
affine result. With actual optimized producer `bca639682c50` (including main's
descriptor accounting through `4d987059e88`), the unchanged root and all nine
positive places execute native 0 / Terminal 0 on macOS ARM64 through ordinary
package update/review/accept. All ten copy-only controls reject as consumed.
The nine corpus pairs have checking, native and combined macOS pins.
Other matching-host observations remain pending.

Checked semantic copies receive ordinary distinct guest homes. They may retain
the same immutable actual C payload; copying its four bytes grants no guest
permission. The runtime checks both the exact selected opaque application and
the admitted multiplicity before reading its stored value. Affine inputs still
take the nonrecycled slot before invoking C. Equal layout, wrong application,
wrong ownership mode, consumed tokens, loans, claims and unsupported backing
do not gain admission. Preparing an independent call preserves the old type,
custody, actual-host-atom and installed-child checks; only affine parameters
retire the caller's frontier.

This establishes the checked Copy input/result lane for closed Inert atoms on
the observed host. Linear, qualified and view-backed inputs, Linux ARM64
execution, full composition drift and replacement lifecycle acceptance remain
separate parent requirements. No owner-level language rule changes here.
