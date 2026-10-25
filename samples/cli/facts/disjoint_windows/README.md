# Disjoint windows

The root `main.omg` runs with `// Expected exit: 0` and `crashes Trap` on failure:

- `items[0..2]` and `items[2..4]` — literal disjoint bounds let two
  `&mut` subslices of one base borrow at once; writes through both.

```sh
omega run --both samples/cli/facts/disjoint_windows/main.omg
```

## Nine places

For an exclusive four-element base, `split == 2` makes `[0,2)` and
`[split,4)` disjoint, permitting simultaneous exclusive loans and writes
`a[0] = 11` and `b[1] = 41`. The checked bounds are
`0 <= 0 <= 2 <= 4`, `0 <= split <= 4`, `0 < 2`, and `1 < 2`.
The boundary `2` belongs to the right half only. Both loans remain live
through both writes and subsequent readback.

| Place | Positive | Reject twin | Control twin | Fact transport |
| --- | --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | — | literal disjoint windows in root main |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | `local/controls/overlap/` | local scalar initialized to 2 |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | `field/controls/overlap/` | record field initialized to 2 |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `parameter/controls/overlap/` | parameter requiring equality to 2 |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `call_result/controls/overlap/` | result guaranteeing equality to 2 |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `match_join/controls/overlap/` | both arms establish 2; caller exercises both |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `const_name/controls/overlap/` | named constant 2 |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `other_state/controls/overlap/` | successor parameter requiring 2 |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `other_machine/controls/overlap/` | forwarding call to machine requiring 2 |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `target_body/controls/overlap/` | four target-scoped bodies requiring 2 |

Every project has an `invalid/` twin (and matching `controls/overlap/`). That twin
changes only the transported boundary to 1 (and consistent argument/contract occurrences).
Its ranges `[0,2)` and `[1,4)` remain in bounds, and both stores remain in bounds;
the second exclusive loan must reject because the first remains active.
Each positive has an exit-0 oracle that checks the borrowed writes and all
caller elements after return: `[11,20,30,41]`. The root application and nine place
projects all verify that disjointness with exit 0.

## Requirements and Evidence

Owning task: **BORROW-PROOF-CONVERGENCE** (`TASKS.md`) under
[borrowed range access and endpoints](../../../../wiki/spec/language/expressions.md#borrowed-range-access),
[collection bounds](../../../../wiki/spec/language/numeric_values.md#collection-bounds),
[borrow authority](../../../../wiki/spec/language/ownership.md#borrows-and-aliases), and
[captured loan places](../../../../wiki/spec/terminal-psi/loans.md#captured-places-and-available-premises).
This row establishes that provably disjoint mutable windows over the same base slice allow simultaneous exclusive loans and writes across all nine places.

- **Establishment:** With a four-element mutable base slice `items: &mut [i32]` (or `[i32; 4]`), the boundary fact `split == 2` establishes `0 <= 0 <= 2 <= split <= 4`, proving that the half-open intervals `[0, 2)` and `[split, 4)` have an empty intersection (`2 <= split`). This statically discharges the borrow-checker nonoverlap proof obligation at the second mutable loan `let b = &mut items[split..4]`, permitting concurrent live mutable loans `a` and `b` on the same base without post-hoc caller equality guards.
- **Identity and lifetime:** The boundary fact `split == 2` tracks the exact scalar value across local variable copy, record field projection, explicit parameter, call return guarantee (`ensures result == 2`), all-arm match join, named constant (`const SPLIT: u64 = 2`), state transition arrival, forwarding machine, and target bodies. Both mutable subslice loans `a: &mut [i32]` and `b: &mut [i32]` coexist simultaneously over distinct non-overlapping index intervals `[0, 2)` and `[2, 4)` of the same underlying storage. Writes through `a[0] = 11` mutate only index 0, and writes through `b[1] = 41` mutate only index 3 (`2 + 1`), preserving non-interfering storage versions without invalidating facts about unrelated indices.
- **Composition:** Calls substitute exact actuals (`split`, `items`), match joins verify that both branches establish `split == 2` (both executed at runtime), and state transitions prove arrival contracts before forming the disjoint loans. Both loans remain live through both writes (`a[0] = 11`, `b[1] = 41`) and subsequent readback. In every place, `<place>/main.omg` and `<place>/invalid/main.omg` (and `<place>/controls/overlap/main.omg`) differ solely in the transported boundary (`split == 2` vs `split == 1`), keeping the construct, base slice, and operations identical.
- **Soundness boundaries:** When the boundary is shifted from 2 to 1 in each reject twin (`[0, 2)` and `[1, 4)`), the intervals overlap at index 1 (`1 < 2`). Borrow checking strictly enforces loan exclusivity and fails closed at compile time: forming `let b = &mut items[1..4]` while `a` is still active rejects with `creates local borrow b while local borrow a is still active`. Borrow checking forbids aliased mutable loans to overlapping storage even when individual range bounds are within base capacity.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source range borrow operations `&mut items[0..2]` and `&mut items[split..4]` lower into canonical target-neutral Terminal Psi (`PSITERM\0`) two-word mutable slice descriptors (base pointer + element length) and indexed stores, erasing proof-only disjointness obligations while materializing reborrow restoration windows and affine loan retirement schedules. The Terminal Psi interpreter evaluates both concurrent mutable loans, executes the writes, and agrees with native execution (`omega run --both`, exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Build companion rows join `ProgramEntry` declarations across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) to the Terminal catalog without reopening source. Array layout `[i32; 4]` (`size_of = 16`, align 4, stride 4) and two-word mutable slice descriptors (`size_of = 16`, align 8) validate. Target `CallPlan`/`StatePlan` lower slice descriptor pointers and lengths across register pairs (`rcx`/`rdx` on Windows x86_64, `rdi`/`rsi` on Linux x86_64, `x0`/`x1` on AArch64) and stack slots.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Target backend lowering selects effective-address calculations (`lea` or pointer add) for subslice bases, 32-bit element stores (`mov [base + index*4], 11` on `x86_64`, `str w11, [base, index, lsl #2]` on `arm64`), and integer comparisons for verification. Activation storage allocates stack frame homes for `entries` and descriptor registers, and inserts standard prologues and epilogues honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack displacements resolve to concrete frame offsets, basic blocks lay out with branch relaxation and offset reflow around comparison jumps, transitive machine-state footprint evidence is validated, and direct PE/ELF/Mach-O images are emitted, achieving native execution agreement with Terminal Psi (`omega run --both`, exit 0) across host targets including `windows_x86_64` and `macos_arm64`.

Run the row with a fresh compiler:

```sh
python3 tools/sample_native_gate.py --filter cli/facts/disjoint_windows --cli <omega> --jobs 4
python3 tools/corpus_gate.py --filter disjoint_windows_through_
```

The mirrored corpus pairs are
`tests/omega/pass/facts/disjoint_windows_through_<place>_exit/` and
`tests/omega/fail/facts/disjoint_windows_through_<place>_overlap/`.
Source checking includes every authored target body. Native results establish
only the matching host target.
