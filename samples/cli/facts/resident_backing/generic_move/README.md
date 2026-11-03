# Generic resident movement

Owning outcome: **BUMP-ALLOCATOR-CANARY** / **VEC-NATIVE-GROWTH**.

Fact: **A granted vacant extent retains actual backing through transport,
initialization, permitted writes, and extraction.**

This production extends the existing resident-backing row. `relocate<P,T,Role>`
extracts one complete `T`, initializes it in a second allocation window, and
returns the first window vacant beside the new resident. Both windows come from
one actual allocation. The caller verifies the payload and both neighboring
canaries (`101,17,103`), and verifies that the windows have distinct addresses.

The [cleanup composition](cleanup/main.omg) uses an affine payload with a drop
hook. Its exact stdout is `MD` followed by a newline: `M` is emitted only after
relocation and extraction; `D` is emitted by cleanup. Missing, premature or
duplicate cleanup fails that oracle. The admitted console provider only writes
bytes; it supplies no resident ownership or cleanup evidence.

The [wrong-index twin](invalid/main.omg) changes only the selected placement
identity. Both policies have the same valid geometry, but the original resident
cannot satisfy the other policy's nominal index. The
[double-consumption twin](cleanup/invalid/main.omg) consumes the extracted affine
value twice and must reject the second use.

Before repair, the generic extraction rejected apparently identical `Resident<P,T>`
types because it compared caller parameters with the implementation's unbound
parameters. The concrete control ran on both engines. After that comparison was
repaired, the returned record exposed two related defects: domain substitution
left its instance ID unchanged, and the result-authority fence compared the
selected public contract with the unsubstituted private implementation contract.
The fix retains both contracts independently under the same capture application;
it does not let the public contract supply the implementation's missing promises.

Run both positives with `omega run --both`; check both `invalid/main.omg` twins.
All four hosted targets are declared. Actual run evidence identifies its host;
target declarations alone are not cross-host execution evidence.

This establishes generic, whole-element movement and observable affine cleanup.
It does not establish a runtime-sized owned prefix or a generic Vec implementation.
Those remain the parent outcome's acceptance. The cleanup payload has fixed
runtime scalar fields; borrowed fields require their own storage/custody support.

The runtime-sized ownership carrier is an unresolved
[owner decision](../../../../../OWNER_QUESTIONS.md#7-what-represents-a-runtime-sized-collection-of-owned-non-copy-elements).
This witness proves whole-element transfer, not authority to infer a collection
of residents from a byte range and element count.
