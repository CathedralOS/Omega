# Contract-proved call index

A runtime limit from 9 to 10 binds `Index<limit>`, retaining payload 8 below the exact captured limit.

Parent: `RUNTIME-VALUE-GENERICS` in [TASKS.md](../../../../../TASKS.md). This is
another production of the same runtime-index application fact, next to the
[call-computed index](../call_index/README.md). The constructor
`choose<Created: u32>()` returns `Index<(bound<Created>())>`, and
`bound<Count: u32>() -> u32 ensures result == Count` now has two returning arms
(`transition Count {9 -> 9 _ -> Count}`). The body names no single returned
value, so the call's index must come from the callee's published result
equation, not from its body. `choose` proves `8 < bound<Created>()` from
`Created >= 9`; the receiving `let indexed: Index<limit> = choose<limit>()`
proves the call-computed index equals the captured runtime `limit`.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Every reject twin changes only payload 8 to payload 10 in `choose` and rejects
with the default-domain diagnostic. Every positive binds all four hosted
entries and observes payload 8 and the exact descriptor for limits 9 and 10;
limit 9 takes the first arm of `bound` and limit 10 the fallback, so a wrong
arm or descriptor traps. Corpus pairs are pinned as
`runtime_index_application_contract_index_through_<place>_{exit,rejected}`.

## Baseline

At `4a93353f827` (macOS arm64, optimized producer built from that base) all
nine positives refused with `cannot prove runtime index descriptor equals its
captured index` at the construction in `choose` and `cannot prove runtime index
subject equality` at the receiving local: the index checker evaluated a call
index only from a callee body with one returned value.

## Requirements and evidence

- **Establishment:** the call's value is the callee's first unnamed `ensures`
  conjunct `result == value`, which the callee proves on every normal exit.
  A body with one returned value is used only when no equation is published.
- **Identity and lifetime:** the equation's value is evaluated under the
  call's exact actuals, as for the call-computed production. A published
  equation naming a different value rejects at the receiving local
  ([`runtime_call_index_contract_wrong_result`](../../../../../tests/omega/fail/generics/runtime_call_index_contract_wrong_result/main.omg),
  `ensures result == Count / 2`); the same program receiving
  `Index<(limit / 2)>` checks
  ([`runtime_call_index_contract_result`](../../../../../tests/omega/pass/generics/runtime_call_index_contract_result/main.omg)).
- **Composition:** the nine places carry the admitted limit through a local,
  a field domain, a parameter, a call result's guarantees, a join, named
  bounds, a nested machine, another machine and four Build-selected target
  implementations.
- **Soundness boundaries:** an equation whose value reads `result`, a named
  result binding, or a callee with a parameter named `result` supplies no
  value; the index stays unknown and rejects.
- **Terminal Psi and Omega realization:** the descriptor stays an ordinary
  scalar field computed by the call at run time; proof obligations erase.
  `omega run --both` observes Terminal Psi and native agreement (exit 0) for
  all nine places on macOS arm64. The other three hosted targets keep their
  authored entries and are not executed here.

Remaining open under `RUNTIME-VALUE-GENERICS`: effectful call indices, and
execution on the Windows x86-64, Linux x86-64 and Linux arm64 hosts.
