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

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | `local/main.omg` | `local/invalid/main.omg` | owned local |
| field | `field/main.omg` | `field/invalid/main.omg` | owned record member |
| parameter | `parameter/main.omg` | `parameter/invalid/main.omg` | owned machine parameter |
| call result | `call_result/main.omg` | `call_result/invalid/main.omg` | checked result guarantee |
| match join | `match_join/main.omg` | `match_join/invalid/main.omg` | common completed owned-member fact |
| const name | `const_name/main.omg` | `const_name/invalid/main.omg` | named COUNT |
| another state | `other_state/main.omg` | `other_state/invalid/main.omg` | state-local owned backing |
| another machine | `other_machine/main.omg` | `other_machine/invalid/main.omg` | helper observation under loan |
| another target body | `target_body/main.omg` | `target_body/invalid/main.omg` | four hosted source bodies |

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

## What this row establishes and what stays open

This row discharges the CML4 clause keeping
`pass/facts/nominal_loan_backing_cleanup_through_*_exit` and their
cleanup-prerequisite twins for original nominal backing through successor
loans: an affine occurrence's residual custody survives until its actual
normal death — neither a selection join nor a later reread of the condition
decides hook timing — the loan carries the original `Room` storage into the
successor state, and the owner runs `Room::drop` exactly once, after the
loan closes.

- **Establishment:** `Room { count: 10 }` construction plus the surviving
  loan justify the hook's `requires room.count == 10` at the
  automatic-cleanup edge; every twin that instead establishes count 11
  rejects there.
- **Identity and lifetime:** the successor observes the same backing (10)
  the owner established, and cleanup sees that storage version exactly once —
  no duplicate, reordered or wrong-root hook.
- **Composition:** the fact survives transport through a local, a field, a
  parameter, a call result, a match join, a const name, another state,
  another machine and a per-target body; `match_join` retains only the count
  fact justified on both arms.
- **Soundness boundary:** the nine `invalid/` twins pin that a changed
  cleanup prerequisite blocks the edge rather than weakening the hook
  contract, and the unequal-match control keeps a one-arm fact out of the
  join.
- **Terminal Psi and native realization:** `omega run --both` exit 0 with
  the authored `OD` byte order shows the exact ordered normal-edge cleanup
  of `calls_and_outcomes.md#normal-cleanup` surviving lowering and native
  image emission identically on `macos_arm64`.

Still open under CML4: abandoned construction prefixes, field replacement,
selected/partial-owner cleanup, claim-bearing temporaries, and the
Windows/Linux qualification of the ordered normal-edge cleanup plans named
in the task body. Native execution rows for `windows_x86_64` and
`linux_arm64` record on their own hosts' goldens.

Canonical authority: [ownership and borrowing](../../../../wiki/spec/language/ownership.md)
retains original borrowed storage until callee completion. Its starting blob is
`7e6f9a8ad935edc4e66324dd6ced07fe0c5be7ad`.
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
