# ieee_float_32bit_ordered_bound

// Fact: a 32-bit IEEE f32 ordered comparison guard (reading >= 1.25f32 && reading <= 2.75f32) establishes a finite, non-NaN sub-interval that satisfies a downstream contract requiring reading >= 1.0f32 && reading <= 3.0f32 across all nine places.

Owning Task: `CONSERVATION-CONTRACT` (`TASKS.md`)
Claim Ticket: `b5becca237e84675af8d8e357421e5a9`

## Places

| Place | Positive Program | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| root (`main.omg`) | [`main.omg`](main.omg) with [`build.omg`](build.omg) | — | Canonical root witness forwarding a bounded `f32` reading into `verify_f32_window` |
| `local` | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | The bounded `f32` reading is stored in a local variable before passing to `verify_f32_window` |
| `field` | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | A record field (`Holder.reading.value`) carries the bounded `f32` reading into window verification |
| `parameter` | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | The bounded `f32` reading is passed as an explicit machine parameter with a `requires` contract |
| `call_result` | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | A machine call (`source(reading)`) returns the bounded `f32` reading via an `ensures` contract |
| `match_join` | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Both match arms (`1.75f32` and `2.25f32`) join their bounded `f32` interval into window verification |
| `const_name` | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Named `f32` constants (`LOWER`, `UPPER`) define the interval bounds proving the downstream contract |
| `other_state` | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | A successor machine (`state deliver`) receives the bounded `f32` reading and verifies the downstream window |
| `other_machine` | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | A forwarding machine (`deliver`) forwards the bounded `f32` reading to `verify_f32_window` |
| `target_body` | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Each of four target-scoped machine bodies forwards the bounded `f32` reading to `verify_f32_window` |

## Pinned Fixtures

Each of the nine places is pinned twice in the compiler corpus:

- **Positive native-exit fixtures:** `tests/omega/pass/facts/ieee_float_32bit_ordered_bound_through_<place>_exit/` (`build.omg`, `main.omg`, `expected_native_exit.txt`), binding `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and recorded in `tests/omega/corpus_native_windows_x86_64.txt` (`built ... exit:0`) and `tests/omega/corpus_outcomes.txt` (`checked`).
- **Reject twin fixtures:** `tests/omega/fail/facts/ieee_float_32bit_ordered_bound_through_<place>_rejected/` (`build.omg`, `main.omg`, `expected.txt`), recorded in `tests/omega/corpus_outcomes.txt` (`rejected` with `cannot prove requires contract for call verify_f32_window`).

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declares `Expected exit: 0`, and carries `expected_native_exit.txt` (`0`). Run each positive program with `omega run --both`:

```sh
omega run --both samples/cli/facts/ieee_float_32bit_ordered_bound/main.omg
omega run --both samples/cli/facts/ieee_float_32bit_ordered_bound/local/main.omg
omega run --both samples/cli/facts/ieee_float_32bit_ordered_bound/field/main.omg
omega run --both samples/cli/facts/ieee_float_32bit_ordered_bound/parameter/main.omg
omega run --both samples/cli/facts/ieee_float_32bit_ordered_bound/call_result/main.omg
omega run --both samples/cli/facts/ieee_float_32bit_ordered_bound/match_join/main.omg
omega run --both samples/cli/facts/ieee_float_32bit_ordered_bound/const_name/main.omg
omega run --both samples/cli/facts/ieee_float_32bit_ordered_bound/other_state/main.omg
omega run --both samples/cli/facts/ieee_float_32bit_ordered_bound/other_machine/main.omg
omega run --both samples/cli/facts/ieee_float_32bit_ordered_bound/target_body/main.omg
```

All ten positive programs produce `native exit 0` and `interp exit: 0`.

### Measured Discrimination and Obligation Class

In every positive place, `verify_f32_window(reading: f32) -> i32` requires `reading >= 1.0f32 && reading <= 3.0f32` at compile time and executes the ordered 32-bit IEEE `f32` window comparison `transition reading >= 1.25f32 && reading <= 2.75f32` at runtime, returning `0i32` when inside the window and `1i32` otherwise. In `main`, `transition probe(...) == 0` transitions to `machine done() {}` (exit `0`) and otherwise branches to `machine bad() { crash Trap; }`.

Measured on `windows_x86_64` with `omega-latest.exe run --both` (`{"schema":"omega-compiler-build-profile-v1","opt_level":"1"}`) on `local/main.omg`:

| Probe | Perturbation | Native (`windows_x86_64`) | Terminal Psi (`interp`) |
| --- | --- | --- | --- |
| Baseline (`local/main.omg`) | none (`reading = 2.0f32`, `reading >= 1.25f32 && reading <= 2.75f32`, `probe(reading) == 0`) | `native exit 0` | `interp exit: 0` |
| 1-token demanded `f32` bound perturbation | `reading >= 1.25f32` -> `reading >= 2.5f32` in `verify_f32_window` | `native exit -1073741795` (`0xC000001D` illegal instruction trap) | `interp crash: Trap (host presentation agrees)` |
| 1-token caller return perturbation | `transition probe(reading) == 0` -> `transition probe(reading) == 1` in `main` | `native exit -1073741795` (`0xC000001D` illegal instruction trap) | `interp crash: Trap (host presentation agrees)` |

**Obligation class:** This fact belongs to the **scalar interval / call precondition (`requires` contract) obligation class** (`wiki/spec/language/numeric_values.md#ieee-comparisons`). The narrower closed 32-bit IEEE `f32` interval `[1.25f32, 2.75f32]` established by `Reading`'s `where` clause, `requires`/`ensures` contracts, or named `const` endpoints statically entails the wider finite, non-NaN call precondition `reading >= 1.0f32 && reading <= 3.0f32` on `verify_f32_window(reading: f32)`. Each `<place>/invalid/main.omg` reject twin (`sample.json` + `expected.txt`) breaks only the construct's own upper bound by 1 token (`2.75f32` -> `3.5f32`), never a caller guard, and fails closed at compile time before native realization:

```sh
omega --check samples/cli/facts/ieee_float_32bit_ordered_bound/<place>/invalid/main.omg
```

All nine reject twins refuse with `cannot prove requires contract for call verify_f32_window ... (needs fact \`reading >= 1.0 && reading <= 3.0\` here; establish it before the call via a prior contract guarantee, domain membership, or guard)`.

## Requirements and Evidence

Owning requirement: `wiki/spec/language/numeric_values.md#bounds-as-propositions`, `wiki/spec/language/numeric_values.md#ieee-comparisons`, and `wiki/spec/terminal-psi/integer_certificates.md#canonical-scalar-goals`.
Parent task: **CONSERVATION-CONTRACT** (`TASKS.md`).
This row establishes that a 32-bit IEEE `f32` ordered comparison guard establishing `reading >= 1.25f32 && reading <= 2.75f32` refines a finite, non-NaN sub-interval that strictly satisfies a downstream contract requiring `reading >= 1.0f32 && reading <= 3.0f32` across all nine places.

- **Establishment:** Constructing a bounded `Reading` (`data Reading where value >= 1.25f32 && value <= 2.75f32`) or declaring a contract `requires reading >= 1.25f32 && reading <= 2.75f32` establishes a closed, finite 32-bit IEEE `f32` interval. Because `1.25f32 >= 1.0f32` and `2.75f32 <= 3.0f32`, this interval strictly entails the downstream precondition `reading >= 1.0f32 && reading <= 3.0f32` at `verify_f32_window(reading)`. No post-hoc equality guard is required.
- **Identity and lifetime:** The IEEE `f32` scalar value preserves its exact 32-bit IEEE 754 single-precision bit pattern (1 sign bit, 8-bit exponent, 23-bit fraction) across register passing and memory storage. It retains non-NaN status, sub-interval validity, and value identity across local bindings, struct fields (`Holder`), parameter passing, machine calls, match arms, and target-scoped bodies without precision drift or mutation.
- **Composition:** The narrow 32-bit float interval `[1.25f32, 2.75f32]` composes directly through all nine program places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`), satisfying the consuming contract `[1.0f32, 3.0f32]` and returning exit code 0. In each place, the positive and reject twin differ solely in the upper bound (`2.75f32` vs. `3.5f32`), keeping all other operations identical.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time: widening the upper bound to `3.5f32` violates the precondition `reading <= 3.0f32`, causing the compiler to reject strictly with `error: cannot prove requires contract for call verify_f32_window ... (needs fact \`reading >= 1.0 && reading <= 3.0\`)`. Rejecting unproved wider float intervals prevents out-of-range or NaN values from bypassing downstream contracts.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** 32-bit float interval bounds and contract propositions lower into target-neutral Terminal Psi (`PSITERM\0`), maintaining ordered IEEE comparison semantics (`OrderedFloatLessThanOrEqual`, `OrderedFloatGreaterThanOrEqual`) and erasing proof-only obligation tags. The Terminal Psi interpreter evaluates the float parameter passing and verification call in exact agreement with native execution (`omega run --both`, exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Companion build bindings join `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Standard calling conventions (`CallPlan`) assign 32-bit floating-point registers: `XMM0` on `windows_x86_64` and `linux_x86_64`, `S0` (the lower 32 bits of `V0`) on `macos_arm64` and `linux_arm64`. Value layout geometry for primitive `f32` (`size_of = 4`, `align_of = 4`) and `Reading` validates.
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** The backend selects IEEE 754 32-bit ordered comparison instructions: on `x86_64`, `ucomiss` with parity flag check (`jp`/`jbe`) ensuring unordered/NaN results branch to rejection, and scalar move instructions (`movss`); on `arm64`, `fcmp s` setting condition codes (checked via `b.ls`/`b.hi` respecting unordered/NaN) and `fmov` (single-precision). Register homes in 32-bit floating-point registers and stack frames with 4-byte/8-byte alignment are assigned.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Relocation fixups resolve symbolic displacements and float literal byte-pool references, basic blocks and branch offsets reflow across entry and return epilogues, transitive machine-state footprint evidence is validated (`actual_transitive_footprint ⊆ permitted_transitive_use`), and native executable binaries (PE/ELF/Mach-O) are emitted, passing native execution and Terminal Psi agreement (`omega run --both`, exit 0) across all hosted targets.

What remains open: Mixed-width float interval delivery (`f32` to `f64` conversion refinement) and non-standard subnormal flush-to-zero hardware modes under `CONSERVATION-CONTRACT`.
