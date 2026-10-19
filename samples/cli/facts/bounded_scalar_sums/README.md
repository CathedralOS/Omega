# Bounded Scalar Sums

Bounded `u64` operands `a <= 50` and `b <= 50` make the exact sum
`a + b` fit `result <= 100`; the body stays a plain `a + b`, with no
`Wrapping` domain and no `as` widening.

Every place performs the same operation: add two operands whose declared
bound is `<= 50` and return the sum under `result <= 100`. The oracle runs
both the interior `30 + 40 == 70` and the upper endpoint `50 + 50 == 100`
and exits 0 only when both hold. Only the transport of the two operands changes.

| Place | Project | Transport |
| --- | --- | --- |
| local | `local/` | both bounded parameters are copied into locals |
| field | `field/` | bounded record fields carry the operands |
| parameter | `parameter/` | the adding machine reads its bounded parameters |
| call result | `call_result/` | calls with declared `result <= 50` results supply both operands |
| match join | `match_join/` | both arms keep the operands in range, and both arms run |
| const name | `const_name/` | `const HIGH: u64 = 50` bounds both operands |
| another state | `other_state/` | a successor state receives both bounded operands |
| another machine | `other_machine/` | a forwarding machine transports the operands |
| another target body | `target_body/` | each of four target bodies adds |

Every place (and the root `main.omg`) binds `ProgramEntry` for all four hosted
targets in its `build.omg`, declares `Expected exit: 0`, and carries
`expected_native_exit.txt` containing `0`. Each is an application that runs
under both engines:

```sh
omega run --both samples/cli/facts/bounded_scalar_sums/main.omg
omega run --both samples/cli/facts/bounded_scalar_sums/local/main.omg
omega run --both samples/cli/facts/bounded_scalar_sums/field/main.omg
omega run --both samples/cli/facts/bounded_scalar_sums/parameter/main.omg
omega run --both samples/cli/facts/bounded_scalar_sums/call_result/main.omg
omega run --both samples/cli/facts/bounded_scalar_sums/match_join/main.omg
omega run --both samples/cli/facts/bounded_scalar_sums/const_name/main.omg
omega run --both samples/cli/facts/bounded_scalar_sums/other_state/main.omg
omega run --both samples/cli/facts/bounded_scalar_sums/other_machine/main.omg
omega run --both samples/cli/facts/bounded_scalar_sums/target_body/main.omg
```

Measured on `macos_arm64` at `fdabffc8a0e` with an opt-level 1 producer built
from that revision: all ten report `native exit 0` and `interp exit: 0`
(`rc=0`). `sample_native_gate.py --filter cli/facts/bounded_scalar_sums` reports
10/10 samples meeting their documented oracle and 9/9 intended-invalid controls
matching their authored rejection; the native corpus rows read `built … exit:0`.

Each place has an `invalid/` child that widens a bound so the exact sum is no
longer provably in range. The control must reject with `may overflow \`u64\`` at
`rc=1`. Check a twin with `omega --check <place>/invalid/main.omg`; all nine
twins were measured rejecting with that fragment.

The row is pinned by `tests/omega/{pass,fail}/facts/bounded_scalar_sums_through_*`
fixtures. The nine positives are `*_exit` fixtures carrying a four-target
`build.omg` and `expected_native_exit.txt`; every one records native exit 0.
The nine negatives are `*_rejected` fixtures whose golden rows carry the
decision-17 diagnostic.

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`).
- **Requirement family IDs:** `language-numeric-values-001`,
  `language-numeric-values-006`.
- **Ticket:** `7bbdd20aa82e4150ac40f12c4dd79750`.
- **Requirement established:** a declared operand bound (`a <= 50`, `b <= 50`)
  makes the exact sum `a + b` provably fit `result <= 100` end to end — source
  checking, proof, Terminal Psi and native emission/execution — and that proof
  survives the nine transports of the two operands.

Obligations accounted for:

- *Establishment:* Each `add`/`probe` pair declares `requires a <= 50 && b <= 50`
  and `ensures result <= 100`; the caller's constant actuals (`30, 40` and the
  upper endpoint `50, 50`) discharge those `requires` clauses, so the fact is
  established by the callee contract rather than by a post-hoc caller guard.
- *Identity and lifetime:* The same two operand values reach the addition in
  every place; `local`, `field` and `other_state` copy them into fresh storage
  whose versions are the ones added, and `other_machine`/`call_result` prove the
  bound on the arriving value, not on a discarded one.
- *Composition:* Calls substitute exact actuals (`call_result`, `other_machine`),
  the match join keeps the bound on both incoming arms (`match_join`), the
  successor state proves its own arrival obligation (`other_state`), and the
  consuming `a + b` is proved directly from the carried fact.
  `git diff` confirms each `<place>/main.omg` vs. `<place>/invalid/main.omg`
  differs only in the fact's own input or contract — bounds removed, or in
  `const_name` the ceiling widened to `u64::MAX`.
- *Soundness boundaries:* The nine twins widen the bound (or drop `requires`
  entirely) and the exact sum becomes unprovable, so checking rejects at `rc=1`
  with `may overflow \`u64\`` (decision 17). The twin diagnostic is the semantic
  obligation, never a missing import or unrelated refusal.
- *Terminal Psi and carrier lowering:* Every positive runs under `--both` with
  Terminal Psi and native execution agreeing; the `target_body` place keeps one
  machine body per hosted target and all four are checked in every compilation.
- *Native realization, frame/layout encoding, image emission:* The nine `*_exit`
  fixtures bind `ProgramEntry` for `windows_x86_64`, `linux_x86_64`,
  `macos_arm64` and `linux_arm64`; on `macos_arm64` each emits an image, executes
  and records `exit:0`.

The oracle discriminates. Corrupting the interior expectation to
`probe(30, 40) == 71` makes the `bad()` arm reachable: native reports
`native signal 5` and the interpreter reports `interp crash: Trap`, both engines
agreeing, process exit 255. The authored `== 70 && … == 100` test therefore
fails whenever the sum is wrong; it is not a vacuous assertion.
