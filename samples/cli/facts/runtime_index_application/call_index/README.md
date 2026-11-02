# Call-computed result index

A runtime limit from 9 to 10 binds `Index<limit>`, retaining payload 8 below the exact captured limit.

Parent: `RUNTIME-VALUE-GENERICS` in [TASKS.md](../../../../../TASKS.md). This is
another production of the same runtime-index application fact. The constructor
`choose<Created: u32>()` returns `Index<(bound<Created>())>`: its result index is
a call to the runtime-capable `bound<Count: u32>() -> u32 ensures result == Count`.
The receiving `let indexed: Index<limit> = choose<limit>()` must prove that the
call-computed index equals the captured runtime `limit`, and `choose` must prove
that its payload 8 satisfies `value < bound<Created>()` from `Created >= 9`.

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

Every reject twin changes only payload 8 to payload 10 in `choose`, which no
admitted `bound<Created>()` above 9 can be shown to exceed; each rejects with
the default-domain diagnostic. Every positive binds all four hosted entries and
observes payload 8 and the exact descriptor for limits 9 and 10, so a wrong
descriptor or a swapped branch traps. Matching corpus pairs are pinned as
`runtime_index_application_call_index_through_<place>_{exit,rejected}`.

## Baseline

At `c79bb826816` (shared generation producer, macOS arm64) all nine positives
rejected with `cannot prove runtime index descriptor equals its captured index`
at the construction in `choose` and `cannot prove runtime index subject
equality` at the receiving local. The index checker evaluated a call index by
reading the callee's trailing expression statement. Specialized callee bodies
now end in a value transition, so it found no returned value; the closed-const
control `tests/omega/pass/generics/runtime_computed_const_forwarded_call_exit`
(recorded `checked` at `c64728d90e8`) regressed for the same reason. The
descriptor check also had no way to evaluate a call-formed descriptor value, and
the unspecialized body could not bind `Created` to its current value.

## Requirements and evidence

- **Establishment:** `choose` establishes `8 < bound<Created>()` from its entry
  requirement and `bound`'s single returned value; the caller's `Index<limit>`
  is proved from the same returned value under the exact actual `limit`. No
  caller guard follows the construction.
- **Identity and lifetime:** the call-computed index names the call's exact
  actuals. A static actual reads the current value only for an index formed in
  that scope; a callee's binder never falls back to a same-named caller symbol.
  A callee returning a different value still rejects at the receiving local
  ([`runtime_call_index_wrong_result`](../../../../../tests/omega/fail/generics/runtime_call_index_wrong_result/main.omg)).
- **Composition:** the nine places carry the admitted limit through a local,
  a field domain, a parameter, a call result's guarantees, a join, named
  bounds, a nested machine, another machine and four Build-selected target
  implementations. Each twin differs from its positive only in the payload.
- **Soundness boundaries:** only a callee with one returned value (locals, then
  a trailing expression or a single value transition) is evaluated. Any other
  control shape or a missing operand leaves the index unknown and rejects.
- **Terminal Psi and Omega realization:** the descriptor stays an ordinary
  scalar field; proof obligations erase. `omega run --both` observes Terminal
  Psi and native agreement on macOS arm64. The other three hosted targets keep
  their authored entries and are not executed here.

Remaining open under `RUNTIME-VALUE-GENERICS`: callee result contracts as the
source of a call index (instead of the callee body), effectful call indices,
and execution on the Windows x86-64, Linux x86-64 and Linux arm64 hosts.
