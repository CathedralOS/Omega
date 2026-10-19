# Case payload transport

A multi-variant Outcome sum carries its active payload value <= 50 across the place so matching Outcome::Ready adds 10 within result <= 60.

## Parent Capability and Requirement

- **Parent Task**: `STATIC-MACHINE-MIGRATION`
- **Claim Ticket**: `7eb79955174c45268e495b9239a45bd5` (previous `f665f940b5224387926cac45f2b04b8d`)
- **Requirement**: Sum case active payload bound transport and constraint preservation across static machine places without implicit receiver or data-subject conformances.

Every place declares copyable `data Outcome [copy] { case Empty; case Ready(value: u64) where value <= 50; }` and transports the sum across the designated place. Destructuring `Outcome::Ready { value }` evaluates `value + 10` under postcondition `ensures result <= 60`. The test validates `Outcome::Ready { value: 32 }` yields `42` and `Outcome::Empty {}` yields `0`, exiting with 0.

## Nine Places and Twins

| Place | Transport Description | Positive Witness | Reject Twin |
| --- | --- | --- | --- |
| `local` | Copied into a local binding | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) |
| `field` | Stored in and read from a record field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) |
| `parameter` | Passed as an explicit machine parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) |
| `call_result` | Returned from a helper machine call | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) |
| `match_join` | Joined from conditional branch arms | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) |
| `const_name` | Named copyable constant binding | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) |
| `other_state` | Forwarded across an explicit machine state transition | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) |
| `other_machine` | Forwarded through a helper machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) |
| `target_body` | Realized across four target-scoped machine bodies | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) |

## Verification Commands

```sh
omega run --both samples/cli/facts/case_payload_transport/<place>/main.omg
omega --check samples/cli/facts/case_payload_transport/<place>/invalid/main.omg
```

## Observed Exits and Twin Diagnostics

Producer `omega 0.1.0-dev (unrecorded; aarch64-apple-darwin)` at `1976b8a4ae5`,
target `macos_arm64`, `OMEGA_LIBRARY_ROOT=<worktree>/source/library`. Root
`main.omg` and all nine places were executed with `omega run --both`; every one
reported `native exit 0` and `interp exit: 0`. The nine places declare
`Expected exit: 0` and bind `ProgramEntry` for `windows_x86_64`,
`linux_x86_64`, `linux_arm64` and `macos_arm64`.

| Place | `omega run --both` | Reject twin diagnostic |
| --- | --- | --- |
| `local` | native 0 / interp 0 | `cannot prove ensures contract for exit from eval at statement 3: result <= 60` |
| `field` | native 0 / interp 0 | `cannot prove ensures contract for exit from eval at statement 3: result <= 60` |
| `parameter` | native 0 / interp 0 | `cannot prove ensures contract for exit from eval at statement 2: result <= 60` |
| `call_result` | native 0 / interp 0 | `cannot prove ensures contract for exit from eval at statement 3: result <= 60` |
| `match_join` | native 0 / interp 0 | `cannot prove ensures contract for exit from eval at statement 2: result <= 60` |
| `const_name` | native 0 / interp 0 | `cannot prove ensures contract for exit from eval at statement 2: result <= 60` |
| `other_state` | native 0 / interp 0 | `cannot prove ensures contract for exit from eval at statement 2: result <= 60` |
| `other_machine` | native 0 / interp 0 | `cannot prove ensures contract for exit from inspect at statement 2: result <= 60` |
| `target_body` | native 0 / interp 0 | `cannot prove ensures contract for exit from eval at statement 2: result <= 60` |

The oracles were proved to discriminate before they were recorded. Rewriting the
transported payload from `32` to `31` in each place — so the same program reaches
the same destructuring and merely compares `41` where the caller requires `42` —
makes both engines trap: `native signal 5` beside `interp crash: Trap (host
presentation agrees)`, exit `255`. An exit-0 oracle that could not be broken by a
corrupted payload would have pinned nothing.

## Pinned Fixtures

Nine positive fixtures live under
[`tests/omega/pass/facts/case_payload_transport_through_<place>_exit`](../../../../tests/omega/pass/facts)
with `expected_native_exit.txt` of `0`, and nine reject twins under
[`tests/omega/fail/facts/case_payload_transport_through_<place>_rejected`](../../../../tests/omega/fail/facts).
Each twin differs from its positive place in one line — the payload bound
`value <= 50` widens to `value <= 80` — leaving the transport, the caller and the
consuming operation identical. `corpus_native_macos_arm64.txt` and
`corpus_both_macos_arm64.txt` carry the nine authored runs as `built`, `exit:0`,
`interpreter-exit:0`, `interpreter:agrees`.

| Gate | Result |
| --- | --- |
| `sample_native_gate.py --filter cli/facts/case_payload_transport` | 10/10 samples meet their documented oracle; 9/9 intended-invalid controls match |
| `corpus_gate.py --filter facts/case_payload_transport` | clean — 18 fixtures, 9 `checked`, 9 `rejected`, 0 without their expected fragment |
| `corpus_gate.py --filter facts/case_payload_transport --native --both` | 9/9 `built`, exit codes `{'0': 9}` |

## Requirements and Evidence (AGENTS.md)

- **Establishment**: `Outcome::Ready { value: 32 }` satisfies the declared payload bound `value <= 50`.
- **Identity and Lifetime**: Sum instances retain variant discriminant and payload bound facts across value copies, field storage, parameter passing, and state hops.
- **Composition**: Destructuring `Outcome::Ready { value }` introduces `value <= 50` into the branch environment, which satisfies the `ensures result <= 60` contract on `value + 10`.
- **Soundness Boundaries**: In every reject twin, the payload bound is widened to `value <= 80`. The compiler rejects checking because `value + 10` cannot prove `ensures result <= 60`.
- **Terminal Psi and Native Realization**: Every place was realized and executed rather than only checked; the native report and the admitted Terminal program agree on exit 0, and `target_body` realizes the same destructuring in each of the four target-scoped machine bodies.
- **Remaining Open**: Broader static machine migration across remaining standard library and runtime subsystems under `STATIC-MACHINE-MIGRATION`.
