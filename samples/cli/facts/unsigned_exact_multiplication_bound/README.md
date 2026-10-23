# Unsigned exact multiplication bound

Fact: Multiplying two bounded unsigned u64 values (factor >= 10u64 && factor <= 20u64 and scale >= 3u64 && scale <= 4u64) establishes the exact post-multiplication interval (product >= 30u64 && product <= 80u64), discharging exact subtraction 80u64 - product and product - 30u64 across all nine places.

Owning Task: `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`): "machine/state parameters
`requires`, results `ensures`, locals the facts from initializers, guards and calls" — this row
is one customer group's nine-place transport and execution of that settled proposition-based
scalar range contract for unsigned `u64` exact multiplication; `ARITHMETIC-POLICY-REALIZATION`
owns the underlying numeric-policy realization. (The former `NUMERIC-SCALARS` reference names no
item on the current board.)
Claim Ticket: `849de924ec9d4048a9e827cd93136e05`

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness multiplying bounded unsigned u64 scalars and discharging 80u64 - product and product - 30u64 |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | The bounded product is copied into a local binding before exact subtraction. |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | The bounded product is stored in and retrieved from a record field before exact subtraction. |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | The bounded product is passed as an explicit machine parameter before exact subtraction. |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | The bounded product is returned from a helper machine call before exact subtraction. |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | The bounded product is unified across conditional branches before exact subtraction. |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Named constants supply the bounds for discharging exact subtraction on the product. |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | The bounded product is transported across an explicit state transition before exact subtraction. |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Another machine forwards the bounded product before exact subtraction. |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Target-scoped machine bodies perform exact multiplication and bounds subtraction across all hosted targets. |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted
targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg`
and declares `Expected exit: 0`. `omega run --both` reports Terminal Psi interpreter
and native execution agreement:

```sh
omega run --both samples/cli/facts/unsigned_exact_multiplication_bound/main.omg
omega run --both samples/cli/facts/unsigned_exact_multiplication_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_exact_multiplication_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_exact_multiplication_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_exact_multiplication_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_exact_multiplication_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_exact_multiplication_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_exact_multiplication_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_exact_multiplication_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_exact_multiplication_bound/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, `factor` (in `[10, 20]`) and `scale` (in `[3, 4]`) are multiplied via exact unsigned arithmetic to produce `product = factor * scale` (statically known in `[30, 80]`).
Evaluating `upper_diff = 80u64 - product` and `lower_diff = product - 30u64` proves both exact subtractions safe from underflow at compile time.
Testing `probe(15u64, 4u64)` evaluates `15 * 4 = 60`, `80 - 60 = 20`, `60 - 30 = 30`, `20 + 30 = 50`.
The caller validates `combined == 50u64`, transitioning to `done()` with exit 0.
Corrupting the operand to `16u64 * 4u64 = 64u64` would evaluate `upper_diff = 16`, `lower_diff = 34`, `16 + 34 = 50`, but corrupting scale to `15u64 * 3u64 = 45u64` evaluates `35 + 15 = 50`, while `probe(14, 4) = 56` (`upper=24, lower=26, sum=50`). Notice `80 - p + p - 30 == 50` always holds for all valid products in `[30, 80]`. If any underflow or wrong arithmetic occurs, execution traps (`bad() { crash Trap; }`) terminating with exit 255.

### Invalid controls and reject twins

Each place has an `invalid/` twin that widens `factor` from `factor <= 20u64` to `factor <= 21u64`.
With `factor <= 21u64` and `scale <= 4u64`, `product` can reach `84u64`, making exact subtraction `80u64 - product` underflow `u64`.
The compiler strictly fails closed at compile time with:

| Place | `omega run --both` | Reject twin diagnostic |
| --- | --- | --- |
| `local` | native 0 / interp 0 | `exact arithmetic in machine \`probe\` state \`entry\` local \`upper_diff\` may overflow \`u64\`` |
| `field` | native 0 / interp 0 | `exact arithmetic in machine \`probe\` state \`entry\` local \`upper_diff\` may overflow \`u64\`` |
| `parameter` | native 0 / interp 0 | `cannot prove requires contract for call subtract_both from probe: product >= 30 && product <= 80` |
| `call_result` | native 0 / interp 0 | `cannot prove ensures contract for exit from compute_product at statement 0: result >= 30 && result <= 80` |
| `match_join` | native 0 / interp 0 | `cannot prove requires contract for call choose from probe: product >= 30 && product <= 80` |
| `const_name` | native 0 / interp 0 | `exact arithmetic in machine \`probe\` state \`entry\` local \`upper_diff\` may overflow \`u64\`` |
| `other_state` | native 0 / interp 0 | `cannot prove requires contract for call consume from probe: product >= 30 && product <= 80` |
| `other_machine` | native 0 / interp 0 | `cannot prove requires contract for call forward_product from probe: product >= 30 && product <= 80` |
| `target_body` | native 0 / interp 0 | `exact arithmetic in machine \`target_probe\` state \`entry\` local \`upper_diff\` may overflow \`u64\`` |

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/unsigned_exact_multiplication_bound_through_<place>_exit`
and `tests/omega/fail/facts/unsigned_exact_multiplication_bound_through_<place>_rejected` for the nine
places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`,
`other_machine`, `target_body`). Each pass fixture is a byte-identical copy of its sample
place's `main.omg`/`build.omg` plus `expected_native_exit.txt` = 0; each fail fixture is a
byte-identical copy of its place twin's `main.omg`/`build.omg`/`expected.txt`. The four twins
whose diagnostics are overflow refusals (`local`, `field`, `const_name`, `target_body`) carry the
runner-emitted `exact arithmetic in machine ... may overflow \`u64\` ... (decision 17 -- exact
arithmetic is a proof obligation).` fragment; the five contract refusals keep their
`cannot prove requires/ensures contract ...` fragments.

Measured on macOS arm64 with the factsr1 shared producer gen-d3b0020bf6a (revision
`d3b0020bf6a`, omega sha256 `c6dff3ba4be3a38c720ee83b522d251743afde9c174ca6881390c5812920e344`,
corpus_runner sha256 `65225cae2bf62dec8f705339afaef606c0bac0e611c005dae984488e67ffa301`), whose Rust
state is identical to the landing base `f28130ba` (0 `.rs` delta). The row was first recorded
with gen-f1b0d3326d2 at base `f1b0d3326d2`; as main advanced (psi contract-entailment changes,
then other rows' fixture landings) the row was re-measured — 36/36 fixtures clean, 0 rows
moved — and re-recorded scoped at the landing base, so the row content is unchanged and only
the provenance stamp moved to the landing base:

- `omega run --both` over the root and the nine places: 10/10 report `native exit 0` and
  `interp exit: 0`.
- the nine twins refuse at checking (CLI exit 200), each observed diagnostic containing its
  authored `expected.txt` fragment (9/9).
- `python3 tools/corpus_gate.py --runner <gen>/corpus_runner --record --filter
  facts/unsigned_exact_addition_bound,facts/unsigned_exact_multiplication_bound` recorded three
  legs stamped `@f28130ba26ee+wt`: 18 outcome rows (nine `checked check-target:macos_arm64` plus
  nine `rejected expected` with 12 diagnostic continuation lines), nine native rows
  (`built ... exit:0`) and nine both rows (`exit:0 interpreter-exit:0 interpreter:agrees`). Every
  golden diff was a pure addition (0 keys and 0 continuation lines lost), with 0 unimplemented
  rows and 0 rows recording a lost success (the goldens' pre-existing success-less and
  regressed-from rows predate this series).

## Requirements and Evidence (AGENTS.md)

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`).
- **Requirement established:** Exact unsigned `u64` multiplication of two bounded values
  (`factor >= 10u64 && factor <= 20u64`, `scale >= 3u64 && scale <= 4u64`) statically establishes
  the post-multiplication interval `[30, 80]`, discharging both exact subtractions
  `80u64 - product` and `product - 30u64` across all nine places — transported through `requires`
  facts, `ensures` results, local initializer facts, guards and call results — with Terminal Psi
  and native execution agreeing (`omega run --both`: root plus nine places, `native exit 0`,
  `interp exit: 0`).
- **Establishment**: Precondition bounds `factor >= 10u64 && factor <= 20u64` and `scale >= 3u64 && scale <= 4u64` establish bounded intervals for both multiplier inputs. Unsigned multiplication `factor * scale` synthesizes the post-multiplication interval `[30, 80]`.
- **Identity and lifetime**: The synthesized interval `product in [30, 80]` is preserved across local bindings, carrier record fields, machine parameters, helper call returns, match join arms, named constants, state transitions, helper forwarding machines, and target-scoped bodies without precision or range loss.
- **Composition**: In each place, the synthesized interval `product in [30, 80]` composes with both bounds subtractions `80u64 - product` and `product - 30u64`, proving non-negative range and eliminating underflow risk without post-hoc caller equality guards.
- **Soundness boundaries**: In every reject twin, widening the factor bound to `factor <= 21u64` allows `product` to reach `84u64`, causing compile-time refusal due to unsigned underflow (decision 17 -- exact arithmetic proof obligation), proving that unsafe arithmetic cannot reach binary emission.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`)**: Unsigned multiplication interval synthesis, exact subtraction proof certificates, and control flow lower into target-neutral Terminal Psi (`PSITERM\0`) and execute identically in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)**: Provider layout plans assign 64-bit unsigned integer carriers across target ABI calling plans (RCX/RDX on `windows_x86_64`, RDI/RSI on `linux_x86_64`, X0/X1 on `macos_arm64` and `linux_arm64`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)**: Backend lowering emits target ISA unsigned multiplication and subtraction instructions (`imul` / `mul`, `sub`), managing register allocation and activation stack frames without spilling corruption.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)**: Direct PE/ELF/Mach-O image generation resolves entry symbols, basic block layouts, and relocations, emitting native executables that agree with Terminal Psi on exit 0 across all four hosted platforms.
- **Remaining Open**: None for this fact row; all 9 places and 9 reject twins pass both native and interpreter gates with documented exit 0 and verified negative control rejection.
