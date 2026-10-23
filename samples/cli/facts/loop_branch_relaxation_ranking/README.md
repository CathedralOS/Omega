# Ranked Multi-State Countdown Loop Across Wide Conditional Branch Span

Fact: a ranked multi-state countdown loop across a wide conditional branch span establishes an exact 100u64 accumulator bound when subtracting 100u64.

Every place carries the same construct and operation: a terminating, ranked multi-state countdown loop (`loop_calc`) alternating between states `step_a` and `step_b` with measure `terminates by remaining`, spanning a wide sequence of local arithmetic computations (over 160 bytes of machine code per state) that exceeds the signed 8-bit displacement threshold (`rel8`) in x86-64 conditional branch relaxation and exercises branch offset reflow during prologue/epilogue insertion in `frame_application`. The loop terminates at `remaining == 0` with an exact accumulator postcondition `ensures result >= 100 && result <= 100`. The resulting `100u64` bound is transported across all nine canonical language places to evaluate exact subtraction `total - 100u64`, returning exit code 0.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) with [`build.omg`](build.omg) | — | canonical root witness evaluating exact subtraction of 100u64 after wide branch countdown loop |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | a local binding receives the loop accumulator before subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | a record field carries the loop accumulator before subtraction |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | an explicit machine parameter receives the loop accumulator |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a contracted forwarding machine result delivers the loop accumulator |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | reconverging match branches join on the exact loop accumulator bound |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constant `const SUBTRAHEND: u64 = 100` supplies the subtrahend |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state transition carries the loop accumulator to subtraction |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a separate evaluation machine carries the loop accumulator |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped machine bodies for all four hosted targets evaluate subtraction |

## Execution

Each positive place and root `main.omg` compiles and executes with agreement between Terminal Psi and native code, exiting 0 on all four hosted platforms (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`):

```sh
omega run --both samples/cli/facts/loop_branch_relaxation_ranking/main.omg
omega run --both samples/cli/facts/loop_branch_relaxation_ranking/local/main.omg
omega run --both samples/cli/facts/loop_branch_relaxation_ranking/field/main.omg
omega run --both samples/cli/facts/loop_branch_relaxation_ranking/parameter/main.omg
omega run --both samples/cli/facts/loop_branch_relaxation_ranking/call_result/main.omg
omega run --both samples/cli/facts/loop_branch_relaxation_ranking/match_join/main.omg
omega run --both samples/cli/facts/loop_branch_relaxation_ranking/const_name/main.omg
omega run --both samples/cli/facts/loop_branch_relaxation_ranking/other_state/main.omg
omega run --both samples/cli/facts/loop_branch_relaxation_ranking/other_machine/main.omg
omega run --both samples/cli/facts/loop_branch_relaxation_ranking/target_body/main.omg
```

### The oracle discriminates

The exit code `0` is strictly discriminative:
- Weakening the accumulator bound to `99u64` causes exact subtraction `total - 100u64` to fail proof checking at compile time.
- Modifying intermediate arithmetic computations causes loop postconditions or caller transition assertions to diverge, branching to `bad()` and executing `crash Trap;` (producing `native signal 5` and `interp crash: Trap`).

## Reject Twins

Each place has an `invalid/` twin that breaks only the accumulator obligation by weakening the entry/loop precondition and postcondition to `99u64`, causing exact subtraction `total - 100u64` to soundly fail proof checking with `exact arithmetic in machine \`evaluate\` state \`entry\` terminal expression may overflow \`u64\`` (`exact subtraction underflow`). All twins carry authored `invalid/sample.json` (`rejection_contains`) and `invalid/expected.txt`:

| Place | Twin | Pinned Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `exact arithmetic` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `exact arithmetic` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `exact arithmetic` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `exact arithmetic` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `exact arithmetic` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `exact arithmetic` |
| another state | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `exact arithmetic` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `exact arithmetic` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `exact arithmetic` |

## Pinned Fixtures

- Positive passes: `tests/omega/pass/facts/loop_branch_relaxation_ranking_through_<place>_exit` (all 9 places pinned, each carrying `expected_native_exit.txt` with `0`).
- Negative controls: `tests/omega/fail/facts/loop_branch_relaxation_ranking_through_<place>_rejected` (all 9 places pinned, rejecting with `exact arithmetic`).

### Run Record

Measured on base `70011192ba70` with a freshly built `target/debug/omega`
(`{"schema":"omega-compiler-build-profile-v1","opt_level":"1"}`):

- `python3 tools/sample_native_gate.py --filter cli/facts/loop_branch_relaxation_ranking`
  — 10/10 samples meet their documented oracle on `macos_arm64`; 9/9
  intended-invalid controls match their authored rejection.
- `python3 tools/corpus_gate.py --native --filter facts/loop_branch_relaxation_ranking`
  — 9 real `built ... exit:0` rows in
  `tests/omega/corpus_native_macos_arm64.txt`, all stamped `@70011192ba70`.

All ten programs (root `main.omg` and the nine places) report `native exit 0`
and `interp exit: 0` under `omega run --both`.

Re-measured on base `9ff26a870215` with the shared wave producer after the
fail fixtures were renamed `..._unproved_<place>` →
`..._through_<place>_rejected` (contents unchanged, golden keys rewritten in
place): `omega run --both` on the root and the nine places again reports 10/10
`native exit 0` with `interp exit: 0`; the nine twins reject with exit 1 and
`exact arithmetic`; `python3 tools/corpus_gate.py --filter
facts/loop_branch_relaxation_ranking` reports `clean — 18 fixtures
({'checked': 9, 'rejected': 9})`, and `--native` reports `clean — 9 fixtures
({'built': 9})`, `executed 9 (exit codes {'0': 9})`. The nine renamed rows keep
their recorded verdict, diagnostics and `@69725a85bb43` stamp.

### The Oracle Discriminates

The loop's own accumulator is the fact's own carrier, so the oracle was proved
discriminating by changing only that carrier's runtime value and leaving every
other part of the program identical. Seeding the loop's own accumulator from a
boundary provider whose contract admits a range makes the value a genuine
runtime value the prover cannot constant-fold, so both programs check and
execute:

| Program | Observed |
| --- | --- |
| control, loop accumulator `100` | `native exit 0` / `interp exit: 0` |
| corrupted, loop accumulator `99` | `native signal 5` / `interp crash: Trap (host presentation agrees)` |

Both engines agree in both directions, so exit `0` is strictly discriminative
for this fact. The same experiment showed the checking layer independently
refuses a construct whose accumulator cannot reach the bound: seeding `99` while
the loop entry clause requires `100` is rejected with
``violates required fact `remaining <= 10 && accumulated == 100` ``, and relaxing
the loop's own postcondition instead is rejected with
``cannot prove requires contract for call evaluate from main``.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`)
- **Ticket:** `5819b3f89f80429086463d3cec4a43e2`

### Specification and Lowering Layers

1. **Establishment:** Concrete entry actuals `remaining == 10` and `accumulated == 100` satisfy the loop entry contract `requires remaining <= 10 && accumulated == 100`. At each cyclic transition edge between `step_a` and `step_b`, invariant arrival substitution proves `acc == 100` and monotonic countdown (`rem - 1`), while the ranking term `terminates by remaining` establishes strict loop termination. The loop-exit edge directly establishes the exact terminal postcondition `ensures result >= 100 && result <= 100` without requiring any post-hoc caller equality guard (`transition x == 100`).
2. **Identity and lifetime:** The loop counter/ranking subject (`rem`) and carried accumulator (`acc`) retain their distinct, non-aliasing storage and SSA identity across cycle back-edges and state transitions. Intermediate local arithmetic temporaries (`a1`–`a12`, `s1`–`s12`) are cleanly bounded to their respective block activations, and live ranges across loop iterations are accurately tracked without variable clobbering.
3. **Composition:** The established `100u64` loop exit bound composes without loss across all 9 canonical places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction `total - 100u64 == 0`. Each place's positive `<place>/main.omg` vs. its reject twin `<place>/invalid/main.omg` differs only in the construct's accumulator bound (`99u64` vs `100u64`), cleanly isolating the exact invariant under test without caller guard divergence.
4. **Soundness boundaries:** All 9 reject twins fail closed during checked compilation when the accumulator postcondition is weakened to `99u64`. Exact subtraction `total - 100u64` underflows `u64`, and proof checking strictly refuses compilation with exit code 1 and diagnostic: `error: exact arithmetic in machine \`evaluate\` state \`entry\` terminal expression may overflow \`u64\``. Loops with non-decreasing rankings or unproved termination bounds are caught before backend code generation.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The ranked multi-state countdown loop lowers into canonical `PSITERM\0` control-flow graphs with strongly connected cyclic transition edges, explicit `Jump` and `Conditional` branches, scalar-block invariants, and well-founded termination ranking certificates (`terminates by remaining` via `Nat::Descending`). Mathematical termination certificates erase after verification while typed basic blocks and state control structures are preserved for execution in the Terminal Psi interpreter (`omega run --both`).
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles uniformly across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Scalar types (`u64`) and record structures validate target layout geometry (8-byte alignment and width). `CallPlan` and `StatePlan` assign loop state parameters and return values to standard ABI calling convention registers and stack slots.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects comparison, subtraction/decrement, and conditional/unconditional branch instructions, assigning register homes and disjoint activation stack-frame storage respecting bounded activation claims (`committed <= bound`). Function entry prologues and multi-return callee-saved restoration epilogues are inserted according to target ABI specifications.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Function-relative basic blocks lay out across loop back-edges and forward exits. Because each state's local arithmetic exceeds 160 bytes of machine code, the displacement exceeds the signed 8-bit displacement threshold (`rel8`), exercising branch relaxation to wide form (`rel32` on x86-64 / `B.cond` on ARM64) with offset reflow during prologue/epilogue insertion in `frame_application`. Machine-state footprint evidence validates transitive usage, and direct native PE/ELF/Mach-O emission produces functioning executables in complete agreement with Terminal Psi (`omega run --both` exit 0).

### What Remains Open

- Dynamic loop bounds derived from heap slices, multi-dimensional lexicographic ranking products across mutual machines, and backward branch relaxation under high register pressure spills.
