# Atlas repair queue

Start with a claim from an [area digest](../README.md), its original record, or a handoff note. Record the contract that can settle the claim. A working description, an unused API and a deliberate refusal each need different evidence from an incorrect compiler result. The survey counts describe the accuracy of spot checks at the survey revision.

| Coverage | Findings and evidence |
| --- | --- |
| 01 CLI and foundations | [Identity types, filenames and earlier CLI fixes](01-cli-foundation.md) |
| 02–05 source stages and representations | [Binder positions, proof inputs and front-half handoffs](front.md) |
| 06 Build and 15 packages | [Integer inputs, package review and remaining questions](06-15-build-packages.md) |
| 07–13 Psi semantics, native stages, representations and targets | [Native fragment replay and Psi/native handoffs](07-13-psi-and-native.md) |
| 14 interpreter | [Interpreter candidates and evidence](14-interpreter.md) |
| 16 tools | [Repository admission and changed-program selection](16-tools.md) |
| 17 Omega-written compiler | [Spelling comparisons and proof-goal capacity](17-omega-source.md) |

## Order of work

1. Start with an accepted wrong result, lost data, or a failed operation reported as success. Use the first layer whose output violates its contract. For a refused valid program, first establish what the spec or an already-supported neighboring case promises.
2. Keep the smallest fixture that fails on the parent. Change one input to establish the neighboring valid result. Fix the owning layer and run its focused suite. When the contract crosses a layer, exercise the consuming entry point as well. Use an end-to-end run when only that route can distinguish the behavior.
3. Give duplicate reports the same fix reference. For an intended behavior, identify the spec and code or test. For an earlier repair, identify the merged change and regression. An untraced claim stays open.
4. Close each area only when its actionable claims have individual dispositions. Include the underlying records, disagreements and all three handoff notes. None of the partial area reports establishes that its area is complete.

## Cross-area findings

| Handoff | Evidence |
| --- | --- |
| [Source stages](../boundaries/front_half.md) | The [front-half review](front.md) distinguishes producer guarantees, receiver checks and outstanding traces. |
| [Psi through native emission](../boundaries/psi_and_native.md) | Certificate tags that once failed to reopen were repaired in [#5181](https://github.com/CathedralOS/Omega/pull/5181). Stage 13 now checks the actual import or callback recorded for a native fragment in [#5274](https://github.com/CathedralOS/Omega/pull/5274). |
| [Outputs and drivers](../boundaries/outputs_and_drivers.md), H5 | Build accepted -129 for an 8-bit signed formal. The adapter now checks the published width. [#5273](https://github.com/CathedralOS/Omega/pull/5273) |
| Outputs and drivers, H8 | Publication reads the staged and installed bytes in `file_publication.rs`. The survey summary’s claim about no readback is stale for this path. A staging-name collision still caused the wrong member to reach replay and was fixed in [#5270](https://github.com/CathedralOS/Omega/pull/5270). |
| Outputs and drivers, H11 and H21 | The handoff note records a subsequent CLI change to compare native and Psi streams. Tools that consume the printed observations are a separate review item. |

The other outputs-and-drivers handoffs remain in the [original inventory](../boundaries/outputs_and_drivers.md). Record the exact route whenever two entry points provide different guarantees.
