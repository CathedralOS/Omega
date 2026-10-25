# Signed 8-bit Wrapping Subtraction Bound Fact Row

Fact: 8-bit signed wrapping subtraction lhs -% rhs with bounded interval 40i8 <= diff <= 90i8 establishes 40i8 <= diff <= 90i8 across all nine places so exact subtractions 90i8 - diff and diff - 40i8 are safe.

Every place computes or transports an 8-bit signed wrapping difference `diff = (lhs as i8 in Wrapping) - (rhs as i8 in Wrapping)` (`compute_diff(80i8, 15i8) == 65i8`) from bounded inputs (`lhs in [60i8..100i8]`, `rhs in [10i8..20i8]`), statically establishing the interval `diff in [40i8..90i8]`. This static bound proves that the consuming exact 8-bit signed subtractions `let headroom: i8 = 90i8 - diff;` (`25i8`) and `let margin: i8 = diff - 40i8;` (`25i8`) are overflow- and underflow-free across all nine language places without runtime arithmetic overflow checks or traps.

Only the transport mechanism of the bounded wrapping difference changes across the nine places.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | bounded wrapping difference is computed and held in a local variable before subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the bounded wrapping difference is stored in a struct field before subtraction |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the bounded wrapping difference is passed to a machine parameter with contract |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a helper machine returns the bounded wrapping difference with guaranteed contract |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both branches of pattern matching evaluate the bounded wrapping difference and join |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply bounded inputs for wrapping subtraction |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the bounded wrapping difference to a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a separate forwarding machine receives and verifies the wrapping difference |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | dedicated target-scoped machine bodies evaluate the bounded wrapping difference on each hosted target |

## Execution

Each positive place is an application that exits 0, and every positive place (and the root `main.omg`) carries an authored `expected_native_exit.txt` declaring `0`. `omega run --both` verifies agreement between Terminal Psi interpreter and native compilation:

```sh
omega run --both samples/cli/facts/signed_8bit_wrapping_subtraction_bound/main.omg
omega run --both samples/cli/facts/signed_8bit_wrapping_subtraction_bound/local/main.omg
omega run --both samples/cli/facts/signed_8bit_wrapping_subtraction_bound/field/main.omg
omega run --both samples/cli/facts/signed_8bit_wrapping_subtraction_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_8bit_wrapping_subtraction_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_8bit_wrapping_subtraction_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_8bit_wrapping_subtraction_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_8bit_wrapping_subtraction_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_8bit_wrapping_subtraction_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_8bit_wrapping_subtraction_bound/target_body/main.omg
```

All ten positive places produce `native exit 0` and `interp exit: 0`.

### The Oracle Discriminates

The authored exit code `0` is not an unconstrained default. If the expectation in `verify_diff()` is modified to require a non-zero exit or if an unexpected computation result occurs (`headroom != 25i8 || margin != 25i8`), execution transitions to `machine bad() { crash Trap; }` with `native signal 5` and `interp crash: Trap`, yielding exit code 255. The oracle therefore strictly discriminates between correct evaluation and faulty execution.

### Reject Twins

Each place has an `invalid/` twin whose `lhs` upper bound is widened to `105i8` (`lhs = 105i8, rhs = 10i8`), allowing the difference to reach `95i8 > 90i8`. When passed to `verify_diff` (or forwarding contracts), the required contract `diff <= 90i8` fails closed at compile time with `cannot prove requires contract`. None reaches native execution.

Check each twin with:
```sh
omega --check samples/cli/facts/signed_8bit_wrapping_subtraction_bound/<place>/invalid/main.omg
```

| Place | Reject Twin | Authored Verbatim Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `cannot prove requires contract for call verify_diff from probe: diff >= 40 && diff <= 90 ... in state `entry`` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `cannot prove requires contract for call verify_diff from probe: held.diff >= 40 && held.diff <= 90 ... in state `entry`` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `cannot prove requires contract for call check_param from probe: diff >= 40 && diff <= 90 ... in state `entry`` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `cannot prove requires contract for call verify_diff from probe: diff >= 40 && diff <= 90 ... in state `entry`` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `cannot prove requires contract for call verify_diff from probe: diff >= 40 && diff <= 90 ... in state `entry`` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `cannot prove requires contract for call verify_diff from probe: diff >= 40 && diff <= 90 ... in state `entry`` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `cannot prove requires contract for call step from probe: diff >= 40 && diff <= 90 ... in state `entry`` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `cannot prove requires contract for call forward from probe: diff >= 40 && diff <= 90 ... in state `entry`` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `cannot prove requires contract for call verify_diff from evaluate_diff: diff >= 40 && diff <= 90 ... in state `entry`` (4 diagnostics, one per target-scoped body) |

Each `invalid/expected.txt` holds the twin's diagnostic verbatim as the source-check diagnostic record
carries it: the single message line, which the CLI prints under its `cannot check prepared project: ...`
header and with an `error: ` prefix. The corpus fragment check compares against that message text, so
each fail fixture pins the `expected` verdict rather than an `unexpected` rejection for another reason.
`...` above abbreviates only the shared `(needs fact `diff >= 40 && diff <= 90` here; establish it
before the call via a prior contract guarantee, domain membership, or guard)` suffix.

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/signed_8bit_wrapping_subtraction_bound_through_<place>_exit`
and `tests/omega/fail/facts/signed_8bit_wrapping_subtraction_bound_through_<place>_rejected` for the nine
places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`,
`other_machine`, `target_body`). Each pass fixture is a byte-identical copy of its sample place's
`main.omg`/`build.omg` plus `expected_native_exit.txt` = 0; each fail fixture is a byte-identical copy
of its place twin's `main.omg`/`build.omg`/`expected.txt` (the place's own verbatim
`cannot prove requires contract ...` diagnostic). Positive versus twin `git diff` changes only the
`lhs` upper bound (`lhs <= 100i8` to `lhs <= 105i8`, the `LHS` constant `80i8` to `105i8`) and the
`compute_diff(...)` operand pair (`80i8, 15i8` to `105i8, 10i8`), which is the fact's own domain.
All 54 fixture files (18 fixtures x 3 files) are byte-identical to the sample files they name,
verified pairwise by SHA-256.

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`): "common fields use data `where`, payloads case `where`, machine/state parameters `requires`, results `ensures`, locals the facts from initializers, guards and calls". This row is one customer group's nine-place transport and execution of that settled proposition-based scalar range contract; `ARITHMETIC-POLICY-REALIZATION` owns the underlying numeric-policy realization of the `Wrapping` operation itself.
- **Requirement established:** 8-bit signed wrapping subtraction (`i8 in Wrapping` subtraction, `lhs -% rhs`, `[numeric values](wiki/spec/language/numeric_values.md)`) of bounded inputs (`lhs in [60i8..100i8]`, `rhs in [10i8..20i8]`) statically establishes interval bounds `[40i8..90i8]`, proving exact 8-bit signed subtractions `90i8 - diff` and `diff - 40i8` overflow/underflow-free without runtime checks across all nine language places.

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** Bounded inputs (`lhs >= 60i8 && lhs <= 100i8` and `rhs >= 10i8 && rhs <= 20i8`) establish static interval bounds on `compute_diff`. Wrapping subtraction over `i8 in Wrapping` (`(lhs as i8 in Wrapping) - (rhs as i8 in Wrapping)`) computes `wrap_diff`, which when cast back to exact `i8` establishes `result >= 40i8 && result <= 90i8` by monotonic interval arithmetic (since no wrap occurs on the bounded domain). The bounds directly justify the consuming exact signed subtractions `90i8 - diff` (`25i8`) and `diff - 40i8` (`25i8`), proving absence of underflow without post-hoc caller equality guards.
- **Identity and lifetime:** The 8-bit signed difference value (`i8`) occupies a 1-byte scalar slot and preserves its exact value identity and interval bounds across local variables, record fields, machine parameters, return results, pattern match joins, named constants, state transitions, machine calls, and target-scoped bodies without truncation, sign distortion, or lifetime aliasing.
- **Composition:** The bounded wrapping difference composes through all 9 places into the consuming operations `90i8 - diff` and `diff - 40i8`. Positive places and reject twins differ only in the input operand values (`(80, 15)` vs `(105, 10)`) and callee bound contracts.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with static proof verification diagnostics (`cannot prove requires contract` / `diff <= 90`), proving that unproved upper bounds are soundly rejected before native emission.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Wrapping subtraction and exact signed subtractions lower into canonical target-neutral Terminal Psi (`PSITERM\0`). Mathematical proof certificates and range assertions are verified and erased, retaining 8-bit signed scalar arithmetic (`ScalarType::I8`), executing with exact parity in the Terminal Psi interpreter (`omega run --both` exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` binds across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). 8-bit signed integers are laid out with 1-byte size and 1-byte alignment (`size_of = 1`, `align_of = 1`). Parameter passing places `i8` values in 8-bit sub-registers (`cl, dl, r8b, r9b` on Windows x86_64; `dil, sil, dl, cl, r8b, r9b` on Linux x86_64; `w0..w7` on ARM64) according to target calling plans.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend code generation selects 8-bit signed subtract instructions (`sub r8, r8` on x86_64, `sub w, w, w` on ARM64) for both wrapping subtraction and exact subtraction. Register allocation assigns dedicated 8-bit homes and disjoint stack frame spill slots conforming to WCSU bounds.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** 8-bit arithmetic instructions encode into valid PE/ELF/Mach-O binaries that natively execute and agree with Terminal Psi (`omega run --both` exit 0) across all hosted targets.

### Verification Evidence

Measured on macOS arm64 with the factsr1 shared generation producer `gen-9b001b8f6af` (revision
`9b001b8f6afa2cdac67fd580033c3560d906a5d5`, the wave base; omega sha256
`6d24393125b7847b2f46d2f25e856f18a644f0fa1cedd2ad9b54f316f8b7da74`, corpus_runner sha256
`cee00bbdce17fc300b548aca249c2e9e2a44e59c17e97186055da76c3947444e`):

- `omega run --both` over the root and the nine positive places: 10/10 report `native exit 0` and
  `interp exit: 0` (Terminal Psi and native agree), exit 0.
- `omega --check` over the nine twins: 9/9 refuse, exit 1, with the twin's own authored fragment; each
  `invalid/expected.txt` is byte-identical to the measured diagnostic and the fail fixture copies it.
- Discrimination: two authored probes of the same place program were run through the same producer.
  Flipping the oracle's expected value (`transition headroom == 24i8 && margin == 25i8`) and corrupting
  the computation (`compute_diff(80i8, 14i8)`, giving `diff = 66i8`) both stay accepted by `--check`
  (exit 0), then run to `native signal 5` / `interp crash: Trap (host presentation agrees)` with exit
  255 — the authored exit 0 is not an unconstrained default.
- The three corpus legs were recorded with the fixture and golden changes staged at published main
  `9b001b8f6afa2cdac67fd580033c3560d906a5d5`, so every row is stamped `@9b001b8f6afa+wt`. Counting this
  row and `sixteen_variant_enum_payload_bound` together (18 fixtures each): `tests/omega/corpus_outcomes.txt`
  gained 36 rows (+60/-0 — 18 `checked ... check-target:macos_arm64`, 18 `rejected ... expected` and 24
  diagnostic continuation lines, 12 per row: eight single-diagnostic twins plus the four target-body
  diagnostics), `tests/omega/corpus_native_macos_arm64.txt` gained 18 `built ... exit:0` rows (+18/-0) and
  `tests/omega/corpus_both_macos_arm64.txt` gained 18 `built ... exit:0 interpreter-exit:0
  interpreter:agrees` rows (+18/-0). All three diff as pure additions, all three legs exited 0 with 0
  `timeout` rows and 0 rows recording a lost success; the native leg's 2 `regressed-from` rows are its
  pre-existing memory-canary pins. The first record pinned the fail rows with the corpus `unexpected`
  verdict because their `expected.txt` carried the CLI's two-line presentation instead of the diagnostic
  message; after the 18 fragments were rewritten, a scoped re-record of those rows (pure addition,
  +60/-0 unchanged) pins `expected` on all 18.
- What remains open: the other `CANONICALIZE-SCALAR-RANGE-CONTRACTS` customer groups and
  `ARITHMETIC-POLICY-REALIZATION`'s remaining numeric-policy routes. Sibling width/operator pairs
  (16-, 32-, 64-bit signed and unsigned wrapping subtraction, wrapping addition/multiplication,
  saturating and exact subtraction) exist as their own rows; this row's nine places and nine twins are
  closed.
