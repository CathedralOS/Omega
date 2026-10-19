# Ten-Argument ABI Multi-Arg Register and Stack Spill Realization

Fact: a ten-argument weighted scalar call preserves exact positional argument order across register and stack ABI slots when subtracting 100u64.

Every place performs the same operation: invoke a ten-argument scalar function `weighted_ten_sum(a1..a10)` with distinct constant actuals (`1, 2, 3, 4, 5, 6, 7, 8, 24, 40`) summing to `100u64`, verify contract substitution and exact positional argument correspondence across register argument slots and stack spill slots on all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), and perform exact subtraction `total - 100u64` returning 0. Only the transport mechanism of the operands and call invocation changes across places.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | ten distinct arguments are stored in locals before passing to the call |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | arguments are stored in and read from record fields |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | ten arguments arrive as explicit parameters and are forwarded directly |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | contracted helper call results supply the arguments |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms execute and join the multi-argument call |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply all ten register and stack operands |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state receives all ten arguments and invokes the call |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate machine forwards all ten arguments |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped machine bodies execute on each hosted platform |

## Execution

Each positive place is an application that exits 0, and every positive place
(and the root `main.omg`) carries an authored `expected_native_exit.txt`
containing `0` beside its entry. `omega run --both` must report Terminal Psi
and native execution agreeing, so each line below observes both engines:

```sh
omega run --both samples/cli/facts/abi_multi_arg_spill/main.omg
omega run --both samples/cli/facts/abi_multi_arg_spill/local/main.omg
omega run --both samples/cli/facts/abi_multi_arg_spill/field/main.omg
omega run --both samples/cli/facts/abi_multi_arg_spill/parameter/main.omg
omega run --both samples/cli/facts/abi_multi_arg_spill/call_result/main.omg
omega run --both samples/cli/facts/abi_multi_arg_spill/match_join/main.omg
omega run --both samples/cli/facts/abi_multi_arg_spill/const_name/main.omg
omega run --both samples/cli/facts/abi_multi_arg_spill/other_state/main.omg
omega run --both samples/cli/facts/abi_multi_arg_spill/other_machine/main.omg
omega run --both samples/cli/facts/abi_multi_arg_spill/target_body/main.omg
```

Every one of the ten places was observed with `native exit 0` and
`interp exit: 0`.

### The oracle discriminates

The authored `0` is not assumed. Corrupting the value under test in the
`local` place by inverting the entry guard to `transition probe() == 1` makes
the fact's operation unreachable at runtime: the state machine takes `bad()`
and traps. Both engines still agree — `native signal 5` and
`interp crash: Trap (host presentation agrees)` — and the run exits `255`
instead of `0`. The recorded oracle therefore separates a preserved
ten-argument call from a broken one:

| Program | Native engine | Terminal Psi | wrapper exit |
| --- | --- | --- | --- |
| `local/main.omg` (authored) | `native exit 0` | `interp exit: 0` | 0 |
| same place, entry guard inverted | `native signal 5` | `interp crash: Trap` | 255 |

The `invalid/` twins below also discriminate in the other direction: they are
refused during proof checking rather than run, so they can never be mistaken
for a passing positive.

Each place has an `invalid/` twin whose 10th argument is `39u64` instead of `40u64` (summing to `99u64`), causing exact subtraction `total - 100u64` to fail proof checking with `may overflow \`u64\`` (`exact subtraction underflow`). All nine twins were observed refused with `checked compilation failed for package
\`abi_multi_arg_spill_<place>_invalid\`` and the `may overflow \`u64\``
diagnostic naming that place's own entry point; none reaches native execution.
Check each twin with:

```sh
omega --check samples/cli/facts/abi_multi_arg_spill/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `789f09ed1a6543bb909557ecbf53ba03` (owner `DeepSeek / w22 pool`).
- **Requirement established:** End-to-end nine-place fact preservation and ABI realization for multi-argument calls exceeding register limits. Specifically, positional argument correspondence and contract substitution across both register argument slots (RCX/RDX/R8/R9 on `windows_x86_64`, RDI/RSI/RDX/RCX/R8/R9 on `linux_x86_64`, X0..X7 on `macos_arm64`/`linux_arm64`) and stack spill argument slots (args 5..10 on Windows, 7..10 on Linux x86-64, 9..10 on ARM64) survive across all nine program places with exact agreement between Terminal Psi and native execution.
- **Pinned fixtures:** the nine positive places are pinned as
  `tests/omega/pass/facts/abi_multi_arg_spill_through_<place>_exit/`, each
  byte-identical to its sample place and carrying
  `expected_native_exit.txt` with `0`. The nine reject twins are pinned as
  `tests/omega/fail/facts/abi_multi_arg_spill_unproved_<place>/`, each
  byte-identical to its `<place>/invalid/main.omg`, binding `ProgramEntry` for
  all four hosted targets and rejecting with the exact-subtraction diagnostic.
- **Obligations accounted for:**
  - *Establishment:* Concrete caller actuals satisfy callee `requires` obligations across all ten register and stack argument positions.
  - *Identity and lifetime:* Positional parameter order is preserved through ABI lowering, stack argument emission, and register allocation without clobbering or displacement.
  - *Composition:* Call results, state transitions, forwarding machines, match joins, and target bodies compose exact ten-argument evaluations and preserve the exact 100u64 contract.
  - *Soundness boundaries:* Reject twins demonstrate that an unproved ten-argument sum underflows exact subtraction and is strictly rejected during proof checking.
  - *Terminal Psi and carrier lowering:* every place reaches both engines with
    `omega run --both` reporting agreement; the discriminating control shows the
    two engines diverge from the authored outcome together, not independently.
  - *Native realization and frame/layout encoding:* the ten-argument call is
    realized on all four hosted targets with the register/stack partition each
    target owns, and the `target_body` place checks that partition per machine
    body rather than once for the host.
- **What remains open:** Aggregate struct argument splitting across register/stack boundaries, indirect large struct return pointers (`sret`), and floating-point multi-argument register/stack partition rows.
