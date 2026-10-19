# IEEE Float Ordered Comparison Guard and Interval Refinement

Parent task: **CONSERVATION-CONTRACT** (`TASKS.md`).

## Governing requirement

An IEEE `f64` ordered comparison guard (`reading >= 1.5 && reading <= 2.5`) establishes a finite, non-NaN sub-interval that satisfies a downstream contract requiring `reading >= 1.0 && reading <= 3.0` across all nine places.

**Fact: an IEEE f64 ordered comparison guard (reading >= 1.5 && reading <= 2.5) establishes a finite, non-NaN sub-interval that satisfies a downstream contract requiring reading >= 1.0 && reading <= 3.0 across all nine places.**

Every place performs the same operation: establish or transport a bounded IEEE `f64` scalar `reading` within `[1.5, 2.5]`, pass it to a consuming contract `verify_window(reading: f64) -> i32 requires reading >= 1.0 && reading <= 3.0`, and return `0i32` to transition to `done()` and exit `0`. The positive oracle asserts exit `0` under both native and Terminal Psi execution. Only the transport and placement of `reading` changes across places.

## Places

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| Root | [`main.omg`](main.omg) | — | Direct float sub-interval delivery into window contract |
| Local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | The bounded reading is stored in a local variable before passing to `verify_window` |
| Field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | A record field carries the bounded reading into window verification |
| Parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | The bounded reading is passed as an explicit machine parameter |
| Call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | A machine call returns the bounded reading directly into window verification |
| Match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Both match arms produce the bounded reading joining into window verification |
| Const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Named constants define the interval bounds proving the downstream contract |
| Another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | A successor state receives the bounded reading and verifies the downstream window |
| Another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | A forwarding machine forwards the bounded reading to the verifying machine |
| Another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Each of four target bodies forwards the bounded reading to `verify_window` |

### Verification commands

Root `main.omg` and all nine place projects bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry `expected_native_exit.txt` (`0`). Run any positive directly:

```sh
omega run --both samples/cli/facts/ieee_float_ordered_comparison/main.omg
omega run --both samples/cli/facts/ieee_float_ordered_comparison/local/main.omg
omega run --both samples/cli/facts/ieee_float_ordered_comparison/field/main.omg
omega run --both samples/cli/facts/ieee_float_ordered_comparison/parameter/main.omg
omega run --both samples/cli/facts/ieee_float_ordered_comparison/call_result/main.omg
omega run --both samples/cli/facts/ieee_float_ordered_comparison/match_join/main.omg
omega run --both samples/cli/facts/ieee_float_ordered_comparison/const_name/main.omg
omega run --both samples/cli/facts/ieee_float_ordered_comparison/other_state/main.omg
omega run --both samples/cli/facts/ieee_float_ordered_comparison/other_machine/main.omg
omega run --both samples/cli/facts/ieee_float_ordered_comparison/target_body/main.omg
```

Each place has an `invalid/` twin that widens the upper bound to `3.5` (violating `reading <= 3.0`), causing entry into `verify_window` to fail closed at compile time. The control must reject with `cannot prove requires contract for call verify_window`. Check each twin with:

```sh
omega --check samples/cli/facts/ieee_float_ordered_comparison/<place>/invalid/main.omg
```

## Requirements and Evidence

Owning requirement: `wiki/spec/language/numeric_values.md#bounds-as-propositions`, `wiki/spec/language/numeric_values.md#ieee-comparisons`, and `wiki/spec/terminal-psi/integer_certificates.md#canonical-scalar-goals`.
Parent task: **CONSERVATION-CONTRACT** (`TASKS.md`).
This row establishes that an IEEE `f64` ordered comparison guard establishing `reading >= 1.5 && reading <= 2.5` refines a finite, non-NaN sub-interval that strictly satisfies a downstream contract requiring `reading >= 1.0 && reading <= 3.0` across all nine places.

- **Establishment:** Constructing a bounded `Reading` (`data Reading where value >= 1.5 && value <= 2.5`) or declaring a contract `requires reading >= 1.5 && reading <= 2.5` establishes a closed, finite IEEE `f64` interval. Because `1.5 >= 1.0` and `2.5 <= 3.0`, this interval strictly entails the downstream precondition `reading >= 1.0 && reading <= 3.0` at `verify_window(reading)`. No post-hoc equality guard is required.
- **Identity and lifetime:** The IEEE `f64` scalar value preserves its exact 64-bit IEEE 754 double-precision bit pattern (sign bit, 11-bit exponent, 52-bit fraction) across register passing and memory storage. It retains non-NaN status, sub-interval validity, and value identity across local bindings, struct fields (`Holder`), parameter passing, machine calls, match arms, and target-scoped bodies without precision drift or mutation.
- **Composition:** The narrow float interval `[1.5, 2.5]` composes directly through all nine program places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`), satisfying the consuming contract `[1.0, 3.0]` and returning exit code 0. In each place, the positive and reject twin differ solely in the upper bound (`2.5` vs. `3.5`), keeping all other operations identical.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time: widening the upper bound to `3.5` violates the precondition `reading <= 3.0`, causing the compiler to reject strictly with `error: cannot prove requires contract for call verify_window ... (needs fact \`reading >= 1.0 && reading <= 3.0\`)`. Rejecting unproved wider float intervals prevents out-of-range or NaN values from bypassing downstream contracts.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Float interval bounds and contract propositions lower into target-neutral Terminal Psi (`PSITERM\0`), maintaining ordered IEEE comparison semantics (`OrderedFloatLessThanOrEqual`, `OrderedFloatGreaterThanOrEqual`) and erasing proof-only obligation tags. The Terminal Psi interpreter evaluates the float parameter passing and verification call in exact agreement with native execution (`omega run --both`, exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings join `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Standard calling conventions (`CallPlan`) assign 64-bit floating-point registers: `XMM0` on `windows_x86_64` and `linux_x86_64`, `D0` (`V0`) on `macos_arm64` and `linux_arm64`. Value layout geometry for primitive `f64` (`size_of = 8`, `align_of = 8`) and `Reading` validates.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** The backend selects IEEE 754 ordered comparison instructions: on `x86_64`, `ucomisd` with parity flag check (`jp`/`jbe`) ensuring unordered/NaN results branch to rejection, and scalar move instructions (`movsd`); on `arm64`, `fcmp` setting condition codes (checked via `b.ls`/`b.hi` respecting unordered/NaN) and `fmov`. Register homes in floating-point register groups (`xmm`/`v`) and stack frames with appropriate alignment are assigned.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Relocation fixups resolve symbolic displacements and float literal byte-pool references, basic blocks and branch offsets reflow across entry and return epilogues, transitive machine-state footprint evidence is validated (`actual_transitive_footprint ⊆ permitted_transitive_use`), and native executable binaries (PE/ELF/Mach-O) are emitted, passing native execution and Terminal Psi agreement (`omega run --both`, exit 0) across all hosted targets.

The row is verified by `samples/cli/facts/ieee_float_ordered_comparison` where every positive records native exit 0 and Terminal Psi execution agreement (`omega run --both`), and every invalid twin rejects with `cannot prove requires contract for call verify_window`.
What remains open: Dynamic NaN-tag payload preservation across foreign FFI boundary crossings and non-standard denormalized subnormal arithmetic flush-to-zero hardware modes.
