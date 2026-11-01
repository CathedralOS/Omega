# Selected layout in active-case visitation

Parent: `LIBRARY-REPRESENTATION-FAMILIES`; composition of the existing
[active-case fact](../../README.md), preserving its fact sentence and operation.

A common field and an active payload use
`LayoutStorage<Payload, Spread<40u64>>`. Typed visitation preserves the exact
policy application while borrowing its fields. The callback observes all saved
scalars, common-before-payload order, exactly one selected case, and the selected
40-byte/8-byte geometry. A nullary case visits its common field exactly once.
Independently initialized array backing observes all three sparse fields.

The [invalid twin](invalid/main.omg) changes only the callback constraint to
`Spread<48u64>`. Both applications intentionally return identical geometry;
the callback body accepts both and remains well formed. Only coverage of the
actual field application's exact identity is removed.

Schema construction and replay read const/value roles from the owning generic
declaration's telescope. A prepared const-only recast produces its exact loan
application; runtime-value targets keep source-capture checks. Runtime storage
keeps a case namespace on structural payload descendants, distinct from a
common field with the same stable number. Direct scalar/byte payload cells keep
their existing namespace. Packed-array views retain their authenticated storage
home through reference formation and primitive reads. These paths change no
source, schema, wire or physical layout identity and grant no new authority.

## Execution evidence

[Successful pilot 41334202369](https://github.com/CathedralOS/Omega/actions/runs/41334202369)
observes the unchanged positive at native and Terminal exit 0 and its twin at
the intended callback-policy refusal. Actual source
`7bdb57d398086add9aed482efe344c5cc515b215` produced verified opt-level 1 CLI
`fffa21fd201ecb5d5b8bc9d1b6208b61cff56ad78bdd9387141f7df167e7cdd1`,
Rust host `x86_64-unknown-linux-gnu`, target `linux_x86_64`. Preparation took
147.796s, authentication 0.941s and the two projects 12.726s. This samples-only
producer has no corpus runner or runner profile. Raw reports/profiles and an
exportable source bundle retain the measured identity.

[Source 620e929d2870e9c95653383215ddf44f140196e2, pilot 41332724163](https://github.com/CathedralOS/Omega/actions/runs/41332724163)
observes the existing scalar active-case and stored-loan positives at native and
Terminal exit 0, with both authored twins rejecting as intended. Its selected
layout positive still declined a packed-array read, so that run remains failed.
The final repair changes only the primitive reader's collection-home transfer;
those record/case/loan controls have no such array view and retain their original
observations. The unexercised field-helper/store variation was removed.

Two canonical checking rows preserve actual source
`dd3a6c22f839ebb2c7d45df8b54938df8d5ef6a0` and the raw default
`macos_arm64` checking target. That producer also rejects existing const-policy
and captured-runtime-index mismatch controls. Later changes affect interpreter
storage only; fixture sources and the checking path remain unchanged. These
checking records are separate from Linux native execution.

Retained baseline source `3d5c5150f08151ca111b2e6b31545d1646085611`, CLI
`090ee60d87c91f8e13b23f0bd14c8f323ed4e3946f524ae7833557cfae419bee`,
refused the positive at nominal reference `40`. Subsequent failed observations
remain tied to their actual producers. Private reductions confirmed that legal
common/payload `#0` identities collided when the interpreter dropped the case
namespace; the active case read payload 42 twice while the idle common retained 7.
The original positive, policy twin, saved values and runtime oracle were not
rewritten to avoid any failure.

Other-host execution and broader schema/case, alias, platform ABI, conversion and
Product acceptance remain parent work. This does not add programmable union
layout or extend C record policy to sums.
