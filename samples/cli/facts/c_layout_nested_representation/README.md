# Explicit nested C representation

Fact: library C layout preserves explicit child representation values, geometry and alias identity across transport.

Parent: `LIBRARY-REPRESENTATION-FAMILIES` in [TASKS.md](../../../../TASKS.md).

## Nearest existing row

[C scalar-record layout](../c_layout_scalar_record/README.md) establishes scalar
placement. This row adds a selected child representation whose 40-byte extent
differs from the semantic child's 24-byte default storage. The outer C record
uses child offset 8, tail offset 48, extent 56 and alignment 8. Its saved scalar
values are 17, child 23/42/9 and 11. Fully initialized `[u64; 7]` backing independently
observes every selected field without reading uninitialized padding.

Local, owned-field and by-value parameter programs carry the same value and
check alias/explicit-application identity. Their twins change only the child's
declared type from `LayoutStorage<Payload, Spread>` to bare `Payload`. Equal
scalar types do not license inheriting default aggregate geometry.

## Requirement coverage

- Establishment: ordinary field writes initialize the exact declared child and
  outer representations; geometry grants no construction or backing authority.
- Identity/lifetime: transparent aliases preserve the complete child and outer
  policy applications. Shared backing observations create no new write rights.
- Composition: explicit child storage stops origin traversal. Fixed arrays
  preserve their element's origin. A bare record or reference remains default.
- Soundness: the three default-child twins and the selected-array twin reject
  instead of substituting native geometry. Scalar-array twins change only the
  receiving policy to an equal-geometry distinct implementation.
- Psi 00–04: the policy receives compiler-derived `FieldLayoutSource`, alongside
  unchanged `FieldKind`; returned plans still undergo complete validation.
- Psi 05–07: Schema ABI 4 retains origin independently from geometry. Terminal
  replay reconstructs it from exact child applications and selected target rows.
- Omega 00–02: nested field size/stride follows the validated child plan. Scalar
  arrays and owned byte arrays have focused composition controls. No compiler
  branch recognizes the C policy's name.
- Omega 03–09: copying, field and argument transport must preserve every observed
  scalar; native/Terminal disagreement, wrong placement or a wrong branch traps.

`compositions/selected_array` observes both 40-byte elements and the resulting
96-byte outer extent; its twin changes only the element to bare `Payload`.
`scalar_array` and `byte_array` check 32-byte and 16-byte C records respectively.
They exercise two different retained element carriers without a transport matrix.

## Execution frontier

Baseline `d312f53e3a50` rejects the unchanged local witness in
[pilot41306652495](https://github.com/CathedralOS/Omega/actions/runs/41306652495):
the C policy returns `entry_count 65 is outside 0..=64`.

Optimized Linux producer `b78ea1d59bc70d0a2d02d6ec34d8f1db58e0b8ac`, image
`dcd70a3060d6c39ebd933d2ef17e70e96f850d17bf086c3f0058fb23d747ce63`,
passes local/field/parameter with native and Terminal exit 0, their default-child
twins, scalar-array and byte-array composition pairs, and the existing scalar
policy pair. [Pilot41307751703](https://github.com/CathedralOS/Omega/actions/runs/41307751703)
also observes all six exact checking mirrors (default macos_arm64 target) and
all three Linux native/Terminal mirrors. This does not claim Mac native execution.
Preparation took 1048.220s, checking 28.099s, native mirrors 25.045s and samples 76.966s.

That workflow remains failed: selected-array construction exposed a copy-property
walk that recognized `LayoutStorage` only at its public entrance, missing array
elements. The repair moves that semantic lens into the recursive property walk.
A further sole-change control removes the child's `[copy]` declaration; physical
representation must never create semantic copyability. The selected-array triple
is pending qualification; earlier independent passing observations retain their
actual producer identity. Call result, match join, const name, nested machine,
other machine and selected target remain open. The full parent also retains
other-host execution, complete SDK ABI records, conversions and Product parity.
