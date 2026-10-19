# Borrowed projection cleanup

Seed.code == 7 survives each place into a shared loan of an anonymous Pair child, whose whole owner cleans after the Unit call.

Every positive keeps code 7 into the same projected shared argument of a Unit
call. Its normal continuation disposes the complete Pair, including the unselected
linear sibling. Each `invalid/` twin breaks only the supplied code fact; both
child hooks retain `requires receipt.code == 7`. All nine positives must return
0 with `omega run --both <place>/main.omg`; each twin must refuse cleanup.

The root keeps the original fixture's ordered effectful construction, explicit
and implicit loans, returned aggregate and fixed-array projection. Before this
repair native returned 0, while Terminal refused final completion with unsettled
backing-owner claims. This row discharges that CML4 normal-continuation gap; it
does not establish the board's remaining cross-host and suspension acceptance.

## Places

The nine places carry `Seed.code == 7` into the borrowed projection of an anonymous Pair child and verify deterministic cleanup:

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | original witness: ordered effectful construction, explicit/implicit loans, returned aggregate and fixed-array projection |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | local `Seed` binding supplies code 7 |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | record field (`Holder.seed`) supplies code 7 |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | callee machine parameter receives code 7 |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | helper call returns code 7 |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | match branches converge on code 7 |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | named constant supplies code 7 |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | state parameter carries code 7 |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | intermediate machine forwards code 7 |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | per-target machine body produces code 7 |

## Requirements and Evidence

Owning spec: [ownership and borrowing](../../../../wiki/spec/language/ownership.md), [structural access](../../../../wiki/spec/terminal-psi/structural_access.md), and [nominal cleanup](../../../../wiki/spec/terminal-psi/ownership.md#nominal-cleanup).
This row establishes deterministic whole-owner cleanup and storage restoration after borrowed field projections across normal calls and control-flow edges.

Full 8-layer specification and lowering evidence:

- **Establishment:** Borrowing a projected field/subplace (`&Pair { first: ..., second: ... }.second`) and retiring or completing the borrow carrier restores whole-owner access and preserves post-borrow invariants directly without any post-hoc caller equality guard (`transition x == 100`).
- **Identity and lifetime:** The projected loan tracks the exact borrowed field path on the root owner (`Pair.second`), preventing conflicting access to that path or unselected sibling fields during the active loan window while restoring full access once the borrow ends and cleanup executes.
- **Composition:** The borrowed projection and its cleanup/restoration window compose across all nine standard places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`). In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in supplying an invalid code (`code: 6` vs `code: 7`) that violates the cleanup hook contract (`Receipt::drop requires receipt.code == 7`) or conflicting access while the projected loan is active, leaving the construct, bound, and operation identical.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time (`--check`) with `cannot prove automatic cleanup requires Receipt::drop clause receipt.code == 7` when the owner or projected path violates the drop contract before the projection loan finishes.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Projected borrows and scope exits lower without loss into explicit affine cleanup actions (`DiscardRoot`, `DiscardResidual`, `InvokeNominal`, `RetireHome`) and borrowed-storage restoration windows (`reborrow restored call uses`) in canonical target-neutral Terminal Psi (`PSITERM\0`), executing with agreement in the Terminal Psi interpreter (`omega run --both` exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Record field offsets and pointer/carrier layout geometry validate (`Pair` storing two `Receipt` linear fields, stride 8, align 8), and `CallPlan`/`StatePlan` lower borrowed projection reference carriers cleanly across registers and stack slots.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects field-offset effective-address computation (`lea` on `x86_64`, `add` on `arm64`), indirect loads/stores, and cleanup hook calls. It assigns disjoint stack-frame homes for the backing record and projection carrier, and inserts standard entry prologues and multi-return restoration epilogues honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements and field offsets resolve, basic blocks and cleanup edges lay out with branch relaxation and offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across host targets including `windows_x86_64` and `macos_arm64`.

## What remains open

Still open under CML4: multi-level projected mutable loans, dynamic runtime-sized projection slices, and foreign non-hosted native execution.

Canonical authority: [ownership and borrowing](../../../../wiki/spec/language/ownership.md)
retains original borrowed storage until callee completion.
[Normal cleanup](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md#normal-cleanup)
requires exact ordered normal-edge actions and outgoing capture before cleanup.

```sh
omega run --both samples/cli/facts/borrowed_projection_cleanup/local/main.omg
omega --check samples/cli/facts/borrowed_projection_cleanup/local/invalid/main.omg
python3 tools/sample_gate.py --filter cli/facts/borrowed_projection_cleanup --cli <omega>
python3 tools/sample_native_gate.py --filter cli/facts/borrowed_projection_cleanup --cli <omega>
python3 tools/corpus_gate.py --filter borrowed_projection_cleanup
python3 tools/corpus_gate.py --native --filter borrowed_projection_cleanup
python3 tools/corpus_gate.py --both --filter borrowed_projection_cleanup
```
