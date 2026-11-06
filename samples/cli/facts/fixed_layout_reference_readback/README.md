# Fixed-layout reference readback

These programs store a reference beside an integer, load both, and return them
after the temporary storage holder is gone. The caller checks that the reference
still reaches the original value and that the neighboring integer is unchanged.
The returned reference must retain both its input lifetime and its exact layout
policy; matching size and alignment do not make two policies interchangeable.

Parent: **LIBRARY-REPRESENTATION-FAMILIES**.

**A reference loaded from fixed-layout storage retains its selected representation policy and live referent while the adjacent integer field keeps its value.**

`update` writes the input pointer and scalar 7 into `EnvelopeLayout`, then returns
them in `Readback`. The caller checks pointee 43 and scalar 7. A separate input
with value 23 gives the rejection controls an independent lifetime to name.
The integer occupies eight bits at offset 0; the reference starts at offset 8.
The selected policy remains `PayloadLayout<7>` throughout the positive programs.

The [root program](main.omg) and these nine places carry the same fact:

| Place | Positive | Wrong returned lifetime |
| --- | --- | --- |
| Local | [program](local/main.omg) | [control](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [control](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [control](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [control](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [control](match_join/invalid/main.omg) |
| Named constant inputs | [program](const_name/main.omg) | [control](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [control](other_state/invalid/main.omg) |
| Another machine | [program](other_machine/main.omg) | [control](other_machine/invalid/main.omg) |
| Build-selected implementation | [program](target_body/main.omg) | [control](target_body/invalid/main.omg) |

The [root lifetime control](../../../../tests/omega/fail/facts/fixed_layout_reference_readback_wrong_region_rejected/main.omg)
changes only the returned carrier's lifetime from the pointer input to the
independent storage input. It must reject because the returned view still borrows
`pointer`. The [policy control](../../../../tests/omega/fail/facts/fixed_layout_reference_readback_wrong_policy_rejected/main.omg)
changes only the loaded reference's destination policy to `PayloadLayout<8>`.
Policies 7 and 8 both have size 8 and alignment 8, but the reference must retain
its exact policy application.

The November 6 runs below executed all ten corpus positives with native and
Terminal Psi exit 0. Each also ran the projects for another machine and the
selected implementation through ordinary CLI package admission and
`omega run --both`, with both engines exiting 0. The programs, Builds and expected results were frozen
before execution and were unchanged between these producer revisions.

| Native host | Producer revision | Run |
| --- | --- | --- |
| Linux x64 | `d06663879aa63b6bc34b3b7cb2ab49b3e3851ad7` | [42285229154](https://github.com/CathedralOS/Omega/actions/runs/42285229154) |
| Linux ARM64 | `bc53b23707f6d8db0843cf7e4d65f8bae61fecc0` | [42289025627](https://github.com/CathedralOS/Omega/actions/runs/42289025627) |

Both full runs remain **failed**: all eleven invalid controls rejected, and all
ten lifetime controls matched their required diagnostics, but the policy control
missed its required message fragment. Its pin says `PayloadLayout<const 8>`;
the compiler reports `PayloadLayout<8>` in the same type-mismatch diagnostic.
That pin is unchanged, the failed control is excluded from new source records,
and this runtime evidence does not close the full row or the parent task.
The existing platform goldens retain actual runtime results with their measured
producer revision; source-check records include only the twenty successful rows.

Earlier Mac ARM64 evidence remains tied to its original producers. Producer
`a070c131d52ba72ae94e6824ef2d8d0099fa697b` (image
`84888d4736958ccf25572325932c9c1d7cac39ebd63d3c833b29708a52dffd08`)
ran the root and seven places. Producer
`363cb9e6a2200663ea536e700352df48167908a9` (image
`1c69c8ae906f093b759f4d0094a613f5105a12fd7a4f7a1c21895edc2a84a1b9`)
ran the relay and selected implementation after their compiler repairs. The
original mixed runs that found those refusals remain failed. These are retained
Mac observations, not executions of the November 6 producer.

All four hosted ProgramEntry bindings and corpus mirrors are present. The match
place supplies inputs for both branch conditions, but its identical arm results
do not test branch discrimination. Broader reference compositions, arbitrary
descriptor bytes, mutable references, byte roundtrips and authority reconstructed
from an address are outside this evidence.

Rules: [representation applications](../../../../wiki/spec/layouts/plans.md#representation-applications),
[returned views](../../../../wiki/spec/language/lifetimes.md#returned-views), and
[carried-loan transport](../../../../wiki/spec/language/lifetimes.md#carried-loan-transport).
