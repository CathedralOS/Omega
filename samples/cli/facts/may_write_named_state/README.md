# A nested helper writes one input and preserves the other input’s bound

An outer helper that calls an inner helper to pick one of two mutable inputs and write through that result preserves a disjoint index-bound fact on the other input.

`relay` passes both mutable inputs and its Boolean choice to the nested machine `hand`, which returns `choose`’s selected reference. `overwrite` stores `9` through that result while evaluating an operand of `add`. The other input stays `1`, below the array length `4`, so the indexed read returns `7`. Every positive retains the marker `1`, chosen value `9`, indexed byte `7` and exit `0`. Every reject twin changes only `relay`’s choice from `true` to `false`, making the write overlap the indexed input.

The array read now precedes the runtime marker and written-slot comparisons. Those comparisons cannot supply its bound. The match place executes both choices and checks a separate path marker, so taking the wrong arm traps even though both paths return the same valid index.

## Owning requirement and source migration

This row advances **BUILD-OWNED-TARGET-SELECTION** in [TASKS.md](../../../../TASKS.md). Four identical target-prefixed `probe` declarations become one ordinary `NestedStateProbe` implementation of `SelectedProbe`. Each target has an explicit typed `Build::select_provider` row. The requirement and implementation both retain `chosen < 4 && other < 4` and an empty service ceiling (`reaches`). The public call returns a byte; it exposes no private returned-reference or write guarantee to the caller.

The nested `relay → hand → choose` calls retain explicit runtime arguments and their original storage lifetime. Ordinary helpers remain free or lexically nested machines. This source migration follows [Build-owned selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection), [nested machines](../../../../wiki/spec/language/machines.md), and [mutation and subject identity](../../../../wiki/spec/language/state_contracts.md#mutation-and-subject-identity).

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | the other input is a local of `main` |
| local | [`local/main.omg`](local/main.omg) | [`invalid_local/main.omg`](invalid_local/main.omg) | the other input is a local |
| field | [`field/main.omg`](field/main.omg) | [`invalid_field/main.omg`](invalid_field/main.omg) | the other input is a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`invalid_parameter/main.omg`](invalid_parameter/main.omg) | the other input arrives as a mutable parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`invalid_call_result/main.omg`](invalid_call_result/main.omg) | a call returns the preserved index bound |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`invalid_match_join/main.omg`](invalid_match_join/main.omg) | both match arms yield the preserved index |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`invalid_const_name/main.omg`](invalid_const_name/main.omg) | `LIMIT` names the index bound |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`invalid_other_state/main.omg`](invalid_other_state/main.omg) | a successor machine receives the index |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`invalid_other_machine/main.omg`](invalid_other_machine/main.omg) | another machine receives the index |
| selected implementation | [`target_body/main.omg`](target_body/main.omg) | [`invalid_target_body/main.omg`](invalid_target_body/main.omg) | one checked implementation is selected by four Build rows |

## Validation

The unchanged original row was measured at `45a95a51f7e69a9f311a6e0a00415847f8fa8d06`, against published base `75119cb23a6452b1889847b02a79015f2df2d5ea`, in [baseline run 41627538646](https://github.com/CathedralOS/Omega/actions/runs/41627538646). All 18 Source cases passed their oracles: nine checked positives and nine intended rejections. All nine pinned positives and all ten positive public projects, including root, produced native/Terminal exit-zero agreement. All nine invalid public projects rejected as intended. Optimized producer profiles, source hashes and raw observations were audited. The original source was already working; no compiler defect is claimed.

The migrated source `03a8bd68bb45d05f948d8d23bb1a1e172829c38d` passed [run 41629361575](https://github.com/CathedralOS/Omega/actions/runs/41629361575). Fresh measurements again show nine checked positives, nine intended bound rejections, nine pinned native/Terminal exit-zero agreements, ten positive public engine agreements including root, and nine intended public rejections. All nine downloaded file hashes, the exact source tree and both optimized profiles were verified. The 18 Source and nine Linux BOTH rows carry that observed source stamp; unrelated record bytes are preserved.

The [published source archive](https://github.com/CathedralOS/Omega/pull/1669#issuecomment-6602976319) retains the measured commit through squash publication. It is 98,502 bytes with SHA-256 `ee76744e109ecdff001712b516066202812ae4fe4c67c319933fcd66dca76c52`; the decoded live comments were checked against the retained local bundle. This archive preserves source, separately from runtime evidence.

Earlier macOS records at `b9060c9ca30d` and `8128744a9d77` remain historical observations, with their original source stamps.

All nine positive/twin Main and Build files match their pins under `tests/omega/{pass,fail}/facts/may_write_named_state_through_<place>_*`. Each twin differs only in the Boolean passed to `relay`, and retains `cannot prove` as its diagnostic fragment. Every positive and root binds `ProgramEntry` for all four hosted targets and declares `Expected exit: 0`. There is no `std` dependency.

```sh
omega run --both samples/cli/facts/may_write_named_state/target_body/main.omg
```

## Requirements established by this row

1. **Establishment.** Initialization or an explicit parameter contract establishes the untouched input’s bound before the write. The index consumes that preserved bound directly.
2. **Identity and lifetime.** The bound names the other input’s current storage. The nested returned reference keeps its exact input alternatives and lifetime; write summaries grant no loan or additional access.
3. **Composition.** The nested call and operand write preserve only the proven disjoint input. Calls, match joins and state arrivals retain their original index contracts. Each twin writes the indexed input instead and must lose its stale bound.
4. **Soundness boundaries.** Public selection publishes the original bounds and empty service ceiling. It does not infer guarantees from a selected implementation’s private body or strengthen caller authority.
5. **Checked-to-Terminal Psi.** The nested returned choice, operand write and proved array read must lower into one checked portable artifact and execute in Terminal Psi.
6. **Provider settlement, layouts and ABI.** Four Build rows select one checked implementation. The two mutable numbers and fixed byte array keep ordinary reference and value calling lanes.
7. **Instructions and activation storage.** Native execution must store `9` into the chosen input and read `7` through the untouched index. The match path marker detects incorrect branch selection.
8. **Encoding and native emission.** Actual matching-host native execution must agree with Terminal Psi on the authored exit result.

Other-host runtime qualification and the remaining library, product, source-family retirement and adversarial selection acceptance stay open under the parent task. A completed row does not close that task.
