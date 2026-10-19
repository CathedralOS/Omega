# Record layout geometry

Fact: the validated byte size and alignment of a padded structural record and fixed array combine to 100u64 when subtracting 100u64.

Owning Task: `CONSERVATION-CONTRACT` (`TASKS.md`)
Claim Ticket: `dd1724b6f7c74abca06de7fcab2ea51e`

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness evaluating size and alignment queries combining to 100u64 |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the size and alignment queries combine in local bindings before subtracting 100u64 |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | a carrier record carries the combined size and alignment before subtraction |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | a helper machine receives the combined geometry as an explicit parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a helper machine returns the combined layout geometry as its call result |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both branches of a match expression produce the combined layout geometry |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants evaluate `size_of` and `align_of` before subtraction |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a transition forwards the combined geometry to a successor state |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine delegates the subtraction to an intermediate machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of the four target bodies computes the combined layout geometry |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted
targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg`
and declares `Expected exit: 0`. `omega run --both` reports Terminal Psi interpreter
and native execution agreement:

```sh
omega run --both samples/cli/facts/record_layout_geometry/main.omg
omega run --both samples/cli/facts/record_layout_geometry/local/main.omg
omega run --both samples/cli/facts/record_layout_geometry/field/main.omg
omega run --both samples/cli/facts/record_layout_geometry/parameter/main.omg
omega run --both samples/cli/facts/record_layout_geometry/call_result/main.omg
omega run --both samples/cli/facts/record_layout_geometry/match_join/main.omg
omega run --both samples/cli/facts/record_layout_geometry/const_name/main.omg
omega run --both samples/cli/facts/record_layout_geometry/other_state/main.omg
omega run --both samples/cli/facts/record_layout_geometry/other_machine/main.omg
omega run --both samples/cli/facts/record_layout_geometry/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, `size_of<PaddedFrame>()` (88u64), `align_of<PaddedFrame>()` (8u64), and `align_of<u32>()` (4u64) evaluate to exactly `100u64`.
Exact subtraction `diff: u64 = (s + a + extra) - 100u64` evaluates to `0u64`, and the state machine verifies `diff == 0`, transitioning to `done()` with exit 0.
If layout geometry evaluation produces any other value, `diff == 0` evaluates to false, transitioning to `bad() { crash Trap; }`, resulting in native signal 5 and interpreter crash Trap (exit 255).

### Invalid controls and reject twins

Each place has an `invalid/` twin (and sibling `invalid/<place>` entry) that narrows the fixed array from `[u64; 9]` to `[u64; 8]` in `PaddedFrame`.
This reduces `size_of<PaddedFrame>()` to 80u64, bringing total layout geometry to `80 + 8 + 4 = 92u64`.
Exact subtraction `(s + a + extra) - 100u64` cannot be proved safe, rejecting at compile time with:

```
exact arithmetic may overflow
```

specifically:

```
error: exact arithmetic in machine `compute_geometry` state `entry` terminal expression may overflow `u64`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation).
```

Restoring the array bound `[u64; 9]` restores clean compilation and zero exit.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`)
- **Ticket:** `dd1724b6f7c74abca06de7fcab2ea51e`
- **Pinned fixtures:**
  - Positive passes: `tests/omega/pass/facts/record_layout_geometry_through_<place>_exit/`
  - Negative controls: `tests/omega/fail/facts/record_layout_geometry_unproved_<place>/`

### Specification and Lowering Layers

1. **Establishment:** `size_of<PaddedFrame>()` (88u64), `align_of<PaddedFrame>()` (8u64), and `align_of<u32>()` (4u64) on structural `data PaddedFrame { tag: u8; word: u64; tail: [u64; 9]; }` establish the exact compile-time byte size, alignment, and internal padding (7 padding bytes following `tag: u8` before 8-byte aligned `word: u64` and fixed array `tail: [u64; 9]`), summing to exactly `100u64` directly without any post-hoc caller equality guard (`transition x == 100`).
2. **Identity and lifetime:** The record layout geometry facts remain bound to the exact record type `PaddedFrame` and its scalar/array fields across all 9 places. Layout evaluations compute canonical target-neutral geometry values that retain their mathematical identities and static bounds across storage into locals, records, parameters, call boundaries, and state transitions.
3. **Composition:** Record layout geometry (`size_of` / `align_of`) composes through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operation `(s + a + extra) - 100u64`. Each positive `<place>/main.omg` differs from its `<place>/invalid/main.omg` reject twin strictly in the layout geometry bound (narrowing `tail: [u64; 9]` to `[u64; 8]`, producing 92u64 total geometry instead of 100u64).
4. **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `exact arithmetic in machine ... may overflow \`u64\`` (`exact subtraction underflow`, bounds `[-8, -8] lie outside [0, 18446744073709551615]`) because `92u64 - 100u64` violates the unsigned range invariant. The compiler enforces that unverified exact arithmetic is rejected.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** `size_of` and `align_of` layout queries and record projections lower into canonical `PSITERM\0` constant representations and exact integer arithmetic operations while erasing proof-only obligations. The lowered representation executes in the Terminal Psi interpreter (`omega run --both`) agreeing with native execution at exit 0.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans evaluate and validate record field offsets (`tag` at offset 0, `word` at offset 8, `tail` at offset 16), alignment padding, and total size (88 bytes with 8-byte alignment). `CallPlan`/`StatePlan` lower carriers across registers and stack slots according to target calling conventions.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering materializes layout geometry constants (`88`, `8`, `4`), assigns register homes and disjoint stack-frame storage, selects native integer addition (`add`) and subtraction (`sub`) instructions, accounts for stack usage in worst-case stack usage (WCSU), and inserts machine entry prologues and epilogues.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements and layout offsets resolve into native instructions, basic blocks and branch targets relax with offset reflow, machine-state footprint evidence validates transitive usage, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across host targets (`windows_x86_64` and `macos_arm64`).
