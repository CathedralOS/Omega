# Live-Across-Call Register Preservation and Spill/Reload

Eight simultaneously live scalar bindings preserved across an interior call sum to 100u64 when subtracting 100u64.

Every place defines eight distinct `u64` scalar bindings (`v1 = 5u64`, `v2 = 10u64`, `v3 = 12u64`, `v4 = 13u64`, `v5 = 14u64`, `v6 = 15u64`, `v7 = 16u64`, `v8 = 15u64`, summing to `100u64`) before calling an interior helper machine `clobber_call(seed: u64) -> u64` that executes intermediate arithmetic operations clobbering caller-saved registers and returns `0u64` under contract `requires seed == 1u64 ensures result == 0u64`.
After `clobber_call(1u64)` returns, all eight pre-call live values plus the call result are combined (`v1 + v2 + v3 + v4 + v5 + v6 + v7 + v8 + call_out == 100u64`), transported through the respective place, and exact subtraction `preserved - 100u64` is performed, returning `0` as `i32`.

| Place | Positive | Reject Twin |
| --- | --- | --- |
| local | `local/main.omg` | `invalid/local/main.omg` |
| field | `field/main.omg` | `invalid/field/main.omg` |
| parameter | `parameter/main.omg` | `invalid/parameter/main.omg` |
| call result | `call_result/main.omg` | `invalid/call_result/main.omg` |
| match join | `match_join/main.omg` | `invalid/match_join/main.omg` |
| const name | `const_name/main.omg` | `invalid/const_name/main.omg` |
| another state | `other_state/main.omg` | `invalid/other_state/main.omg` |
| another machine | `other_machine/main.omg` | `invalid/other_machine/main.omg` |
| another target body | `target_body/main.omg` | `invalid/target_body/main.omg` |

All nine positive places bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and pass `omega run --both` with exit 0 (Terminal Psi and native execution agreement).
Each reject twin sets `v8 = 14u64` instead of `15u64` (summing to `99u64`), causing exact subtraction `preserved - 100u64` to fail compilation with `exact subtraction underflow`.

The row is pinned under `tests/omega/pass/facts/live_across_call_spill_through_<place>_exit/` and `tests/omega/fail/facts/live_across_call_spill_unproved_<place>/`.

Parent outcome: `CONSERVATION-CONTRACT` (`TASKS.md`).
Ticket: `7a9b4fb00e034587ad28fd21781dfc63`.
