# Nominal reference cleanup

A nominal owner retains its exclusive output reference through a shared call, then normal cleanup writes captured code 7 to the original backing.

This establishes CML4's intact reference-bearing nominal receiver requirement
under [ordinary borrowed calls and cleanup](../../../../wiki/spec/language/ownership.md#consumers-and-cleanup)
and [nominal cleanup](../../../../wiki/spec/terminal-psi/ownership.md#nominal-cleanup).
The borrowed call preserves the owner's custody; cleanup substitutes that exact
owner into the hook's prerequisite and retains its original output reference.

The nine projects exercise local, field, parameter, call result, match join,
const name, a nested machine, another machine, and another target body. Each copies
that sentence and lends the same fresh `Keeper` to the same shared `observe` call.
Its hook requires code 7 and writes through the original exclusive reference.
The following check observes 7 in the original `Counter`, so a skipped hook,
wrong backing or copied referent cannot satisfy the runtime oracle.

## Places

| Place | Positive Program | Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| (root) | [main.omg](main.omg) | [invalid/main.omg](invalid/main.omg) | root baseline: local nominal keeper holding exclusive reference |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | seed code copied into a local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | seed code stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | seed code passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | seed code returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | seed code unified across match branches |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | seed code named and constrained by a constant |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | seed code transitioned to successor machine argument |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | seed code forwarded through an intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | seed code consumed across target-scoped machine bodies |

## Execution

Every positive place (and the root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares `Expected exit: 0`.
`omega run --both` reports Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/nominal_reference_cleanup/main.omg
omega run --both samples/cli/facts/nominal_reference_cleanup/local/main.omg
omega run --both samples/cli/facts/nominal_reference_cleanup/field/main.omg
omega run --both samples/cli/facts/nominal_reference_cleanup/parameter/main.omg
omega run --both samples/cli/facts/nominal_reference_cleanup/call_result/main.omg
omega run --both samples/cli/facts/nominal_reference_cleanup/match_join/main.omg
omega run --both samples/cli/facts/nominal_reference_cleanup/const_name/main.omg
omega run --both samples/cli/facts/nominal_reference_cleanup/other_state/main.omg
omega run --both samples/cli/facts/nominal_reference_cleanup/other_machine/main.omg
omega run --both samples/cli/facts/nominal_reference_cleanup/target_body/main.omg
```

All 10 positive entries check and execute with native exit 0 and Terminal exit 0.

### The oracle discriminates

The runtime check verifies the writeback to the original `Counter` after cleanup.
If the cleanup hook is skipped, if writes occur to a duplicate/copied referent, or if
an incorrect code is written (e.g. `6` instead of `7`), the downstream transition guard
`output.value == 7` branches to `bad()` and triggers `crash Trap;`.
Both engines agree on the non-zero exit (`native signal 5` and `interp crash: Trap`).

### Reject twins

Each place (and the root witness) has an `invalid/` twin that supplies code `6`
while leaving the cleanup hook's `requires keeper.code == 7` requirement intact.
Because automatic cleanup must satisfy the hook's prerequisite at scope exit,
the compiler rejects all 10 invalid controls at compile time:
`cannot prove automatic cleanup requires Keeper::drop clause keeper.code == 7`.

## Pinned Fixtures

- Positives: `tests/omega/pass/facts/nominal_reference_cleanup_through_<place>_exit` (9 places pinned, each byte-identical to `<place>/main.omg` with native exit 0).
- Reject twins: `tests/omega/fail/facts/nominal_reference_cleanup_through_<place>_prerequisite` (9 places pinned, each byte-identical to `<place>/invalid/main.omg`, rejecting with `cannot prove automatic cleanup requires Keeper::drop clause keeper.code == 7`).

### Recorded native evidence (macos_arm64)

Measured on `macos_arm64` at revision `fa5d68d80e9a` with the producer built
from that revision (`opt_level 1`):

```sh
python3 tools/corpus_gate.py --native --filter pass/facts/nominal_reference_cleanup_through
```

Before the record the gate reported all nine positives as `new fixture
(built)`, and the run reported `9 fixtures ({'built': 9})`,
`executed 9 (exit codes {'0': 9})`, 0 rejected without their expected fragment
and 0 fail fixtures that did not reject, so the gap was the durable native row,
not a compiler refusal. The nine rows now read `built ... exit:0
@fa5d68d80e9a` in `tests/omega/corpus_native_macos_arm64.txt`, spliced in as a
pure addition (+9 lines, 0 deletions, no timeout rows, LOST 0).

The `--both` leg already carried this fact's nine agreement rows (`built ...
exit:0 interpreter-exit:0 interpreter:agrees @d7a10cf8ff64+wt` in
`tests/omega/corpus_both_macos_arm64.txt`), so native-versus-interpreter
agreement was recorded before this record; what this row adds is the native leg
its own acceptance names.

The mechanism is exercised on this host rather than merely compiled for it:
only `target_body` is target-scoped, and its `macos_arm64` body is where the
cleanup runs — the lent `Keeper` reaches `Keeper::drop`, which writes
`keeper.output.value = keeper.code` through the retained exclusive reference,
and the following `check` observes `7` in the original `Counter`. The other
eight places are target-neutral.

The record discriminates in both directions. Corrupting the authored
observation in the local positive (`transition output.value == 7` to
`== 8`) says nothing statically; the program builds and traps, and the gate
reports `expected native 0, observed built {'exit': 'signal', 'signal': '5'}`
with the structural move `exit:0` to `exit:signal signal:5` (exit 3).
Restoring the fixture byte-for-byte returned the same filter to `clean — 1
fixtures ({'built': 1})`. Corrupting the hook's subject instead
(`Seed { code: 7 }` to `Seed { code: 8 }`) is refused before execution with
exactly this row's obligation: `cannot prove automatic cleanup requires at
selected edge from main state entry after statement 2: Keeper does not
establish Keeper::drop clause `keeper.code == 7``. A compiler commit landed
between the record and publication (`ff96e24570b8`, 2 Rust files in the scalar
conditions check), so the leg was re-measured with a producer rebuilt from that
base: `clean — 9 fixtures ({'built': 9})`, `executed 9 (exit codes {'0': 9})`,
0 rejected without their expected fragment, 0 fail fixtures that did not
reject.

The nine reject twins were re-verified on the check leg: `clean — 9 fixtures
({'rejected': 9})`, 0 rejected without their expected fragment, 0 fail
fixtures that did not reject.

Remaining open on other hosts: `tests/omega/corpus_native_linux_x86_64.txt` and
`tests/omega/corpus_native_windows_x86_64.txt` carry no row for this fact and
need those hosts; `linux_arm64` has no native golden in this checkout.

## Requirements and Evidence

- **Parent outcome:** `CML4` intact reference-bearing nominal receiver requirement ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place reference preservation, nominal wrapper ownership, deterministic cleanup writeback, and native execution agreement for reference-bearing nominal receivers.

Full 8-layer specification and lowering evidence:

- **Establishment:** Constructing the nominal reference wrapper `Keeper { output: &mut output, code: seed.code }` and establishing the prerequisite `seed.code == 7` guarantees that the cleanup hook `Keeper::drop(keeper: &mut Keeper) requires keeper.code == 7` is statically satisfied. Scope exit deterministically triggers `Keeper::drop`, executing `keeper.output.value = keeper.code;` to write 7 into the original `Counter`. The static prerequisite is established at wrapper introduction and carrier transport, without any post-hoc caller equality guard (`transition output.value == 7` only checks the observed post-cleanup state).
- **Identity and lifetime:** The exclusive reference `output: &mut Counter` held within `Keeper` preserves exact referent identity and exclusive borrow custody throughout the shared `observe(&keeper)` call. The reference remains valid until `Keeper::drop` executes at scope exit and retires the home. The backing storage is neither duplicated nor relocated, and no intervening access invalidates the loan.
- **Composition:** The nominal reference carrier and its established seed code compose across all 9 standard places:
  - `local`: seed code is stored in an intermediate local variable.
  - `field`: seed code is packed into and projected from an aggregate record field (`Holder.seed.code`).
  - `parameter`: seed code is received via a callee machine parameter contract (`requires seed.code == 7`).
  - `call_result`: seed code is returned from a helper function with an exit postcondition (`ensures result.code == 7`).
  - `match_join`: seed code is unified across conditional match branches.
  - `const_name`: seed code is bound to and constrained by a named constant (`const code: u64 = 7;`).
  - `other_state`: seed code is passed as an argument across state machine transitions (`transition { _ -> consume(seed) }`).
  - `other_machine`: seed code is forwarded through an intermediate machine call chain.
  - `target_body`: seed code is received and verified across target-scoped machine bodies for all four hosted platforms.
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in supplying code 6 instead of 7, isolating the exact cleanup contract violation.
- **Soundness boundaries:** All 10 reject twins (`invalid/main.omg` and `<place>/invalid/main.omg`) fail closed during type and proof checking. When `seed.code` is 6, the compiler detects that `Keeper::drop` requires `keeper.code == 7`, rejecting compilation with `cannot prove automatic cleanup requires Keeper::drop clause keeper.code == 7`. Borrow checker rules strictly prevent concurrent access to `output` while borrowed by `Keeper`, and unproved prerequisites cannot escape to runtime.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Nominal reference wrappers lower without semantic loss into canonical target-neutral Terminal Psi (`PSITERM\0`). Lowering materializes explicit affine cleanup actions (`InvokeNominal` for `Keeper::drop`, `DiscardResidual`, `RetireHome`) and manages borrow restoration windows (`reborrow restored call uses`) for the borrowed `Counter`. The program executes identically in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Build companion rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate the geometry of `Keeper` (16 bytes: 8-byte pointer to `Counter` + 8-byte `u64` code, alignment 8) and `Counter` (8 bytes). `CallPlan` and `StatePlan` lower nominal wrapper references and cleanup hook calls across ABI parameter registers and stack slots according to target calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions (`x86_64` and `arm64`) for pointer loads and indirect stores (`mov [rax], rcx` on `x86_64`, `str x1, [x0]` on `arm64`), as well as the cleanup hook invocation. Register allocators assign register homes and disjoint stack-frame activation storage for the local `Counter` referent and `Keeper` carrier, inserting function prologues and multi-return restoration epilogues that respect `StatePlan` stack bounds.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack displacements for local referents resolve into concrete frame offsets. Function basic blocks, cleanup edges, and transition jumps lay out with branch relaxation and offset reflow. Transitive machine-state footprints validate, and direct PE/ELF/Mach-O emission produces native executables agreeing with Terminal Psi (`omega run --both` exit 0) across host targets.

Remaining open on CML4: residual-cleanup, crash routes, fuel/retry, suspension, and cross-host acceptance; CML4's remaining residual-cleanup and computed-call requirements stay open on the board.
