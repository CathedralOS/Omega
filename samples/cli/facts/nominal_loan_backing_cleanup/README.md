# Nominal loan backing cleanup

An affine Room with count 10 and executable nominal cleanup retains original backing through a successor-state loan; successor observes 10, and owner cleans exactly once after loan closes.

The original root `main.omg` and `build.omg` preserve the reported reproduction:
a nominal hook writes 99 and the successor must still observe 10. On the starting
compiler at `7875d3411f26556b6c6711494cd659032e274abe`, checking accepted it but
`omega run main.omg` exited 200 with “state loan home requires contextual cleanup
that cannot be relocated”. Checking alone did not establish native support.

`8fd8af53161` repaired checking and loan-chain emission so the loan keeps the
original nominal backing; its places then bound only
`linux_x86_64::ProgramEntry`, so `omega run --both` on a `macos_arm64` host
still refused with "selected target `macos_arm64` has no bound required root
slot `macos_arm64::ProgramEntry`" (exit 200). Every positive place and the
root now bind `ProgramEntry` for `windows_x86_64`, `linux_x86_64`,
`macos_arm64` and `linux_arm64`, declare `Expected exit: 0`, and pass
`omega run --both` on macOS ARM64 — native stdout `OD` (`ODOD` for
`match_join`), exit 0, with the Terminal Psi leg agreeing — so retained
backing and once-only cleanup hold identically in both engines.

Each place keeps the same Room, count-10 premise and successor observation. Its
hook requires count 10 and writes `D`; the successor writes `O` while borrowing
the original Room. Exact stdout `OD` and exit 0 establish observation before
once-only cleanup. `match_join` calls both Boolean arms and expects `ODOD`.

## Places

The nine places carry the nominal loan and its cleanup obligation through each language construct:

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | original reproduction: nominal hook writes 99, successor observes 10 |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | owned local |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | owned record member |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | owned machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | checked result guarantee |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | common completed owned-member fact |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | named COUNT |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | state-local owned backing |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | helper observation under loan |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | four hosted source bodies |

Twins retain valid loans and reachable normal cleanup, but fail the count-10
hook prerequisite at automatic cleanup. The parameter twin enters with count 10
and changes it to 11 in its successor; the other twins establish count 11
through the corresponding transport. Twin observations accept 11 so their normal
cleanup remains reachable; the tested cleanup requirement stays 10. Result promises remain honest. The unequal
match control separately prevents one arm's count from becoming a join fact.

The matching corpus pairs are
`tests/omega/pass/facts/nominal_loan_backing_cleanup_through_<place>_exit` and
`tests/omega/fail/facts/nominal_loan_backing_cleanup_through_<place>_cleanup_prerequisite`.
The Linux Console provider uses the existing syscall descriptor and ordinary OS
service; the Terminal engine consumes its existing trusted host adapter.
The same authored output oracles apply to both engines. Foreign hosted bodies
are source checked; this trial does not claim foreign native execution.

## Requirements and Evidence

Owning spec: [ownership and borrowing](../../../../wiki/spec/language/ownership.md) and [nominal cleanup](../../../../wiki/spec/terminal-psi/ownership.md#nominal-cleanup).
This row discharges the CML4 clause keeping
`pass/facts/nominal_loan_backing_cleanup_through_*_exit` and their
cleanup-prerequisite twins for original nominal backing through successor
loans: an affine occurrence's residual custody survives until its actual
normal death — neither a selection join nor a later reread of the condition
decides hook timing — the loan carries the original `Room` storage into the
successor machine, and the owner runs `Room::drop` exactly once, after the
loan closes.

Full 8-layer specification and lowering evidence:

- **Establishment:** Constructing `Room { count: 10 }` (or binding count 10 via parameter, call result, const name, or matching arm) and passing a loan `&mut room` into the successor observation state satisfies the nominal owner's drop prerequisite `requires room.count == 10`. This establishes deterministic cleanup writeback/release when the owner's scope ends, without any post-hoc caller equality guard (`transition x == 100`) after the construct completes.
- **Identity and lifetime:** The loan `&mut room` inside the state parameter or nominal carrier remains bound to the exact backing storage identity of `room` for the entire lifetime of the nominal owner. The successor machine observes the exact original count (10) before `Room::drop` executes, and cleanup sees that exact storage version once (writing 99 or outputting `D`), preventing conflicting access until nominal cleanup completes.
- **Composition:** The nominal loan carrier and its cleanup obligation compose across all nine standard places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`). In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the violated cleanup prerequisite (e.g. establishing count 11 instead of 10 while retaining the hook's `requires room.count == 10`) or conflicting borrow use while the nominal owner is still live, never in a caller guard.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time (`--check`) with `cannot prove automatic cleanup requires Room::drop clause room.count == 10` (exit 1) when the caller accesses the borrowed backing before the nominal owner is cleaned up or violates the cleanup contract. The unequal-match control separately prevents one arm's count from becoming an unjustified join fact.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Nominal scope exit lowers into explicit `InvokeNominal` / `RetireHome` affine cleanup actions and borrow restoration windows in canonical target-neutral Terminal Psi (`PSITERM\0`). Transition edges, state-parameter loans, and normal-edge drop calls lower without loss into canonical IR, executing identically in the Terminal Psi interpreter (`omega run --both`, exit 0, stdout `OD` / `ODOD`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate nominal record/pointer layout geometry (`Room` struct containing `u32 in Wrapping` count, 64-bit reference addresses), and `CallPlan`/`StatePlan` lower nominal carriers and cleanup hook invocations across registers and stack slots.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions (`x86_64` / `arm64`) for indirect pointer loads/stores and nominal cleanup calls. It assigns disjoint stack-frame homes for the backing local and nominal owner across calls, ensuring callee and cleanup stack homes do not alias or clobber live state, and inserts entry prologues and multi-return restoration epilogues honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve for the borrowed backing, basic blocks and cleanup edges lay out with branch relaxation and offset reflow after prologue/epilogue insertion, machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0, stdout `OD` / `ODOD`) on `windows_x86_64` and `macos_arm64`.

## What remains open

Still open under CML4: abandoned construction prefixes, field replacement,
selected/partial-owner cleanup, claim-bearing temporaries, and the
Windows/Linux qualification of the ordered normal-edge cleanup plans named
in the task body. Native execution rows for `windows_x86_64` and
`linux_arm64` record on their own hosts' goldens.

Canonical authority: [ownership and borrowing](../../../../wiki/spec/language/ownership.md)
retains original borrowed storage until callee completion. Its starting blob is
`7e6f9a8ad935edc4e66324dd6ced07fe0c5be7ad`.
[Nominal cleanup](../../../../wiki/spec/terminal-psi/ownership.md#nominal-cleanup)
governs nominal drop actions and borrow restoration.
[Normal cleanup](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md#normal-cleanup)
requires exact ordered normal-edge actions and outgoing capture before cleanup;
its starting blob is `7e1a0a1fda3b3d8e90276d01fa7cccc4fe473bf8`.
No cleanup successor exists for a crash.

```sh
omega run --both samples/cli/facts/nominal_loan_backing_cleanup/local/main.omg
omega --check samples/cli/facts/nominal_loan_backing_cleanup/local/invalid/main.omg
python3 tools/sample_gate.py --filter cli/facts/nominal_loan_backing_cleanup --cli <omega>
python3 tools/sample_native_gate.py --filter cli/facts/nominal_loan_backing_cleanup --cli <omega>
python3 tools/corpus_gate.py --filter nominal_loan_backing_cleanup
python3 tools/corpus_gate.py --native --filter nominal_loan_backing_cleanup
python3 tools/corpus_gate.py --both --filter nominal_loan_backing_cleanup
```
