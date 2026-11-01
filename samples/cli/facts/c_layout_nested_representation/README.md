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
representation must never create semantic copyability. Optimized source `c966c3b51d7a2d22f3cb3e9bd7db305ce0386c9e`, image
`3fbfbc85f402eabe1f2bda391d010bd60a87f64e14922542a67379ac76a8a1f2`,
passes the unchanged selected-array positive (Linux native/Terminal exit 0),
its default-child twin and its noncopy-child control in
[pilot41309926510](https://github.com/CathedralOS/Omega/actions/runs/41309926510).
Preparation took 683.172s and the three projects took 24.497s. Earlier passing
observations remain bound to their actual b78 producer: the private property-walk
repair changes nested representation descent, while direct child checks, scalar
arrays, policy geometry, ABI replay and their inputs are unchanged. The full parent retains
other-host execution, complete SDK ABI records, conversions and Product parity.

## Transport evidence

Call-result, match-join and named-constant places pass Linux x86_64 native and
Terminal execution with exit 0 in [pilot 41311782126](https://github.com/CathedralOS/Omega/actions/runs/41311782126).
Their three default-child twins reject with the authored `entry_count 65 is
outside 0..=64` diagnostic. The six exact corpus mirrors independently pass
checking; all three positives also execute in both engines. The match case
supplies the expected record and a first=18 decoy in opposite orders for its
two calls: both actual arms retain the same values, but wrong-arm selection traps.

Optimized source `0ca75b7285b2f2fced8ec253492d0e6e3a29d090`, CLI image
`31721e29736b827dcb3db55b34e697de54d7789265211c4c87c3fd5bf285d5b6`, verified
`opt_level=1`, took 1075.633s to prepare and 1.816s to authenticate. Corpus
checking took 30.733s, corpus native/Terminal 27.348s, and sample projects
46.812s. Canonical checking rows preserve actual raw diagnostics, timings and
the default `macos_arm64` check target under that source stamp; native execution
remains Linux x86_64 evidence. Raw reports and exportable source evidence are
retained with the pilot. This batch changes no compiler or library mechanism.
Broader parent acceptance remains open.

Nested-machine, ordinary-machine and Build-selected implementation places pass
in [pilot 41314362026](https://github.com/CathedralOS/Omega/actions/runs/41314362026).
All three positives and independent corpus mirrors exit 0 in Linux x86_64 native
and Terminal execution; all three sole-change default-child twins reject with
the authored C-policy refusal. Nested invocation takes the complete record
explicitly and owns its independent backing, with no runtime capture. Selected
providers retain distinct 17/29 markers, so choosing the wrong provider traps
even though both deliver the same record.

Optimized source `433f783bd43e4450949f4ae7ab3d4a7b77b3e137`, CLI image
`31721e29736b827dcb3db55b34e697de54d7789265211c4c87c3fd5bf285d5b6`, verified
`opt_level=1`, took 733.634s to prepare and 1.302s to authenticate. Corpus
checking took 15.233s, corpus native/Terminal execution 14.805s, and sample
projects 25.387s. Six canonical checking records retain raw diagnostics,
timings and their default `macos_arm64` check target under that source stamp;
Linux x86_64 is the actual native host. The CLI bytes match the preceding batch.
This batch changes no compiler or library mechanism; raw reports and exportable
source evidence are retained.

The complete nine-place row and its default-child twins now have focused
observations. Historical observations above keep their original producer
identities. Other-host execution, broader schema/case composition, complete SDK
ABI records, conversions and Product acceptance remain parent work.

## Linux ARM64 execution

The complete nine-place row and all nine sole-type twins pass on native Linux
ARM64 in [pilot 41317826843](https://github.com/CathedralOS/Omega/actions/runs/41317826843).
The same producer observes both C-layout rows: 18 positives execute with native
and Terminal exit 0, and all 18 twins reject their authored policy obligations.
The source programs, four target bindings, values, backing observations and
geometry oracles are unchanged.

[Source `05bfe3d4b250f158f332757d02eb9670f3e0a7e7`](https://github.com/CathedralOS/Omega/tree/05bfe3d4b250f158f332757d02eb9670f3e0a7e7)
produced authenticated opt-level 1 CLI
`2ba88ac44acb292ff35f3bcebd70e7e2d39a236acf8caf04aa71acee019678f6`.
The actual Rust host is `aarch64-unknown-linux-gnu`, the runtime target is
`linux_arm64`, and the GitHub runner is `ubuntu-24.04-arm`. Preparation took
984.501s, authentication 0.789s and the 36 projects 145.549s. The sample-only
producer has no corpus runner or runner profile. Raw summary/sample JSON and an
exportable source bundle preserve that producer identity independently of later
documentation or queue commits. This is matching-host execution, not a cross
compile or another host's checking record.

Windows x86_64 and macOS ARM64 execution, broader schema/case composition,
complete platform ABI records, conversions and Product acceptance remain parent
work. Earlier Linux x86_64 observations retain their original producer identities.
