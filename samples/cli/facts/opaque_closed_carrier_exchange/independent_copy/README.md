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

The same nine fixtures now have matching-host native 0 / Terminal 0 observations
on Windows x86-64 and both Linux targets. Linux run
[40220498665](https://github.com/CathedralOS/Omega/actions/runs/40220498665)
used the retained optimized producers `bca639682c50` (x86-64) and
`213f298c1854` (ARM64); their five compiler input trees are identical. Each
host checked 36 positives and 36 intended rejects and ran all 36 positives in
both engines, including this Copy row and the direct and independent Affine
rows. The Copy and independent Affine captures each retain two actual native
component images and their descriptions. Harness source `65110bf15220` is
recorded separately from the actual compiler producers. Missing ARM runtime
history is explicitly unknown; authored numeric, diagnostic and component
publication oracles still apply.

Windows run [40215318892](https://github.com/CathedralOS/Omega/actions/runs/40215318892)
remains failed: its Copy-local native process timed out, while the other eight
Copy places and all nine Terminal executions succeeded. The failed local PE
is byte-identical to two successful places. The fixed, finite diagnostic run
[40219949587](https://github.com/CathedralOS/Omega/actions/runs/40219949587)
used the original optimized `bca639682c50` compiler: all five native/Terminal
pairs succeeded, including three local executions. Three direct launches of
the byte-verified local executable and its actual published DLL bundle also
exited 0 under the unchanged 20-second deadline. Component filenames and
receipts bind the exact emitted bytes; the short-lived processes did not yield
loaded-module path samples. Independent review accepted the Windows functional
scope with the original unexplained timeout retained. No environmental cause
or success for the original failed run is inferred.

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
all four matching hosted targets. Linear, qualified and view-backed inputs,
full composition drift and replacement lifecycle acceptance remain
separate parent requirements. No owner-level language rule changes here.
