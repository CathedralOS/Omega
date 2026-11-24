# Atlas repair queue

## Review status on November 23, 2026

Three of the 17 areas have a disposition for every original observation in sections 5 and 6. Those are CLI and foundations, Build, and package management. Their ledgers have no outstanding demonstrated incorrect results. The CLI's specified `source rewrite` command remains a separate release feature. The other 14 areas are under review. This snapshot includes changes merged through [#5403](https://github.com/CathedralOS/Omega/pull/5403).

| Area and evidence | Original observations | Review status at this revision |
| --- | ---: | --- |
| [01 · CLI and foundations](01-cli-foundation.md) | 62 | Each observation has a disposition. Fixes include compiler fingerprints changing on repeat builds, unequal identity tables comparing equal, and review edits losing CRLF endings. |
| [02–05 · Source stages and representations](front.md) | 223 | Integer operands no longer establish Boolean cases in generic proofs. User records named `IntervalSet` or `CountedQuantity` now specialize normally. Array element names in diagnostics remain under review. [#5395](https://github.com/CathedralOS/Omega/pull/5395), [#5402](https://github.com/CathedralOS/Omega/pull/5402) |
| [06 · Build and 15 · Package management](06-15-build-packages.md) | 170 | Both candidate indexes have individual dispositions. The Build signed-width repair and package review evidence are recorded in the joint ledger. |
| [07–13 · Psi and native compilation](07-13-psi-and-native.md) | 513 | Recovered register choices now honor the active allocator policy. Native behavior checks follow the exact selected provider when packages reuse a machine name. Other findings remain under review. [#5396](https://github.com/CathedralOS/Omega/pull/5396), [#5400](https://github.com/CathedralOS/Omega/pull/5400) |
| [14 · Psi interpreter](14-interpreter.md) | 59 | Repairs cover implementation-table indexes, UTF-8 byte stores and resumed execution. Open traces include forwarded host values and structural copies. |
| [16 · Repository tools and benchmarks](16-tools.md) | 67 | The benchmark validator now checks recorded compiler input identities. Sample selection, fleet results and repository checks still have open findings. [#5397](https://github.com/CathedralOS/Omega/pull/5397) |
| [17 · Omega-written compiler and bundled libraries](17-omega-source.md) | 68 | Discarding a linear operator result now produces a diagnostic. Whole-subject scalar bindings reuse the evaluated subject. Library findings include oversized input to the staged SHA-256 implementation. [#5390](https://github.com/CathedralOS/Omega/pull/5390), [#5399](https://github.com/CathedralOS/Omega/pull/5399) |
| **All areas** | **1,162** | **232 observations in the three reviewed areas. The other 930 are in areas with partial coverage, including fixes already merged.** |

The counts include each top-level bullet or numbered observation in sections 5 and 6 of the [original area digests](../digests/) once. One observation can raise several questions, and several can refer to the same defect. The records and cross-area handoff notes also contribute findings. The ledgers do not yet establish a total number of outstanding defects.

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
