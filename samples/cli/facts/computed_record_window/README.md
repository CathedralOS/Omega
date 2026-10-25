# Computed record window

Parent task: **STATE-LOCAL-VALUE-FRONTIER** / **SAMPLE-CORPUS** (`TASKS.md`).

## Governing requirement

In the `which == 0` arm, `[0..which + 2]` has length 2; `hold` preserves that extent in its returned `Held.values`, through which `write_pair` writes the backing array. Every place retains this fact, bound, operation, and `f64` backing.

**In the which == 0 arm, [0..which + 2] has length 2; hold preserves that extent in its returned Held.values, through which write_pair writes the backing.**

Every place performs the same operation: compute a record window / subview (`&mut samples[0..which + 2]`) from an underlying four-element `f64` array (`samples: [f64; 4]`) in the matched `which == 0` branch, package it into a returned record `Held { values: samples }` under precondition `requires samples.len == 2` and postcondition `ensures result.values.len == 2`, write `0.0` and `1.0` through the returned field `leading.values` via `write_pair`, and verify that `samples[0] == 0.0` and `samples[1] == 1.0` while `samples[2]` and `samples[3]` preserve `9.0`. The positive oracle asserts the exact float values and exits 0 in both native and Terminal execution. Each invalid twin changes only the computed window endpoint from 2 to 1 (e.g. `[0..which + 1]`), failing the unchanged length-2 promise; the state twin fails admission to the state requiring that extent.

## Places

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| Root | [`main.omg`](main.omg) | — | Root baseline window computation across local, field, and call subjects with transition verification |
| Local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | Computed slice window returned from machine and held in a record local |
| Field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | Computed slice window stored in an enclosing record field before holding and writing |
| Parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Computed slice window passed as an explicit machine parameter |
| Call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Computed slice window returned directly from a helper call result |
| Match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Both match arms yield slice windows that join with guaranteed extent |
| Const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Named constant endpoint supplies the computed slice window bound |
| A nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | State transition transports the computed slice window into a successor machine |
| Another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Forwarding machine transports the computed slice window across machine boundaries |
| Another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Target-scoped machine bodies compute and return the slice window on each hosted target |
| Case payload | [`case_payload/main.omg`](case_payload/main.omg) | [`case_payload/invalid/main.omg`](case_payload/invalid/main.omg) | Computed slice window packaged into an enum variant payload requiring `held.values.len == 2` |

Existing controls under `controls/invalid_<place>` are preserved intact.

### Verification commands

Root `main.omg` and all place projects bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declare `Expected exit: 0`. Run any positive directly:

```sh
omega run --both samples/cli/facts/computed_record_window/main.omg
omega run --both samples/cli/facts/computed_record_window/local/main.omg
omega run --both samples/cli/facts/computed_record_window/field/main.omg
omega run --both samples/cli/facts/computed_record_window/parameter/main.omg
omega run --both samples/cli/facts/computed_record_window/call_result/main.omg
omega run --both samples/cli/facts/computed_record_window/match_join/main.omg
omega run --both samples/cli/facts/computed_record_window/const_name/main.omg
omega run --both samples/cli/facts/computed_record_window/other_state/main.omg
omega run --both samples/cli/facts/computed_record_window/other_machine/main.omg
omega run --both samples/cli/facts/computed_record_window/target_body/main.omg
omega run --both samples/cli/facts/computed_record_window/case_payload/main.omg
```

Each place has an `invalid/` twin whose computed window endpoint is 1 instead of 2, failing the length-2 promise. Check each twin with:

```sh
omega --check samples/cli/facts/computed_record_window/<place>/invalid/main.omg
```

## Measured oracle

The root, all nine places, and `case_payload` each carry `expected_native_exit.txt` containing `0` beside an empty `input.txt`. Both files are required: `tools/sample_native_gate.py` honors authored `expected_*` files only for a project that carries `input.txt`, and otherwise scores the header alone.

The oracle discriminates on both halves of the fact. Against a byte-identical copy of `local/main.omg`:

| Probe | Result |
| --- | --- |
| unmodified source | `native exit 0`, `interp exit: 0`, exit 0 |
| `write_pair` writes 5.0/7.0 instead of 0.0/1.0, every contract intact | native signal 5, interp crash Trap, exit 255 |
| zero arm becomes `samples[1..index + 3]` with the length-2 promise intact | native signal 5, interp crash Trap, exit 255 |

The second row shows the write through the returned record field is observable rather than assumed; the third shows a wrong window endpoint traps even when the promised length is unchanged.

## Requirements and Evidence

Owning task: **STATE-LOCAL-VALUE-FRONTIER** / **SAMPLE-CORPUS** (`TASKS.md`).
The requirement established is that a record window / subview computed at runtime from a matched arm and carried out through a returned record field is proved from the matched branch hypothesis rather than a literal `[0..2]` spelling, and that writes through the returned field are observable and preserve disjoint sibling elements across all nine places.

- **Establishment:** Computing or borrowing a record window / subview (`&mut samples[0..which + 2]`) from an underlying backing array (`samples: [f64; 4]`) inside the matched branch `which == 0` establishes the window's extent (`result.len == 2`) directly from the branch hypothesis (`which == 0` implies `which + 2 == 2`). Machine `hold<'samples>(samples: &'samples mut [f64]) -> Held<'samples>` consumes the slice view under `requires samples.len == 2` and republishes `ensures result.values.len == 2` on the returned `Held` record. The length-2 invariant is established directly by the callee contracts and match facts without any post-hoc caller equality guard (`transition x == 100`).
- **Identity and lifetime:** The computed record window retains its exact backing field identity and referent extent while unrelated sibling elements remain accessible. The returned record `Held<'samples> { values: &'samples mut [f64] }` carries an exclusive borrow of the subslice `samples[0..2]`. The borrow origin correctly tracks the parent array `samples` across local binding, record encapsulation, parameter passing, and state transitions, ensuring that writes through `leading.values` mutate only `samples[0]` and `samples[1]` while preserving `samples[2]` and `samples[3]` (`9.0`).
- **Composition:** The computed record window and its extent/loan fact compose through all nine places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`, and `case_payload`) into the consuming write `write_pair(leading.values)` and subsequent backing verification. Across all nine places, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in the computed window endpoint (e.g., `0 -> &mut samples[0..index + 1]` yielding length 1 instead of 2, or a named constant / state contract requiring extent 2 with endpoint 1), keeping the record holding and write operations identical.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins (and corresponding `controls/invalid_*` controls) fail closed at compile time: when the computed window endpoint produces a subslice of length 1, `window` cannot discharge `ensures result.len == 2`, rejecting with `cannot prove ensures contract for exit from window` (`result.len == 2`), or fails admission to a successor machine requiring that extent. Static verification prevents under-length or improperly bounded slice views from being wrapped in records or passed to `write_pair`.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Computed record windows, view descriptors, and borrow restoration windows lower into canonical target-neutral Terminal Psi (`PSITERM\0`), preserving view descriptor carriers (pointer and length pair) inside the containing `Held` record while erasing proof-only obligation tags (`proof-term-tags`). The Terminal Psi interpreter evaluates slice indexing, length checks, record packaging, and indirect writes through view descriptors, agreeing identically with native execution (`omega run --both`, exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Build companion rows join `ProgramEntry` declarations across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) to the Terminal catalog without reopening source. Layout plans validate backing array geometry (`[f64; 4]`: `size_of = 32`, `align_of = 8`), slice descriptors (`&mut [f64]`: pointer + length, `size_of = 16`, `align_of = 8`), and record `Held` (`size_of = 16`, `align_of = 8`). `CallPlan`/`StatePlan` lower slice views and record carriers across register pairs (e.g. `rcx`+`rdx` / `x0`+`x1`) and stack slots.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Target backend lowering selects integer arithmetic for window offset calculations (`which + 2`), indexed double-precision floating-point loads and stores (`movsd` / `ldr d0`, `str d0`), and assigns register homes and activation frame storage. Worst-case stack usage (WCSU) is composed, and standard function entry prologues and epilogues are emitted honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements and slice descriptor offsets resolve, basic blocks and conditional branches relax with offset reflow around exit transitions, machine-state footprint evidence is validated (`actual_transitive_footprint ⊆ permitted_transitive_use`), and direct PE/ELF/Mach-O images are emitted, achieving native execution agreement with Terminal Psi (`omega run --both`, exit 0) across host targets including `windows_x86_64` and `macos_arm64`.

The row is pinned by `tests/omega/pass/facts/computed_record_window_through_*_exit` and `tests/omega/fail/facts/computed_record_window_through_*_rejected` fixtures; every positive records native exit 0.
