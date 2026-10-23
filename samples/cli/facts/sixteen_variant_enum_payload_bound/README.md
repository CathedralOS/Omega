# Sixteen-variant enum payload bound

Fact: matching a 16-variant tagged enum payload Packet { V1(u64), V2(u64), V3(u64), V4(u64), V5(u64), V6(u64), V7(u64), V8(u64), V9(u64), V10(u64), V11(u64), V12(u64), V13(u64), V14(u64), V15(u64), V16(u64) } where every arm satisfies 40u64 <= v <= 80u64 preserves 40u64 <= res <= 80u64 across all nine places so exact subtractions 80u64 - res and res - 40u64 are safe.

Owning Task: `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`): "common fields use data `where`, payloads case `where`". This row is that settled case-payload clause carried through all nine transport places; `ARITHMETIC-POLICY-REALIZATION` owns the underlying numeric-policy realization.

Every positive place and reject twin tests a 16-variant tagged enum (`data Packet [copy]`) with bounded 64-bit unsigned payloads across all variants (`V1(v: u64)` through `V16(v: u64)` where `v >= 40u64 && v <= 80u64`).
Matching or extracting the active variant payload yields an exact result `res: u64` statically guaranteed to lie in `[40, 80]`.
Casting or transporting `res` across places preserves `40u64 <= res <= 80u64`.
Subsequent exact subtractions `headroom = 80u64 - res` and `margin = res - 40u64` inside `verify_payload` are proved underflow-free and non-negative, evaluating to `20u64 + 20u64 = 40u64` for `v == 60u64` and terminating with exit 0.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness matching 16-variant enum payload and evaluating dual margin subtractions |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | extracted 64-bit variant payload is copied into a local binding before verification |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | extracted 64-bit variant payload is stored into and read from a record field before verification |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | extracted 64-bit variant payload is passed as an explicit machine parameter before verification |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | extracted 64-bit variant payload is returned from a dedicated helper machine |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms of a match expression unify the extracted 64-bit variant payload |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply upper and lower ceilings for variant payload bounds |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition carries the extracted 64-bit variant payload into a successor state |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an independent forwarding machine transports the extracted 64-bit variant payload |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target provides a target-scoped machine body performing payload extraction and verification |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg` and declares `Expected exit: 0`.
`omega run --both` verifies Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/sixteen_variant_enum_payload_bound/main.omg
omega run --both samples/cli/facts/sixteen_variant_enum_payload_bound/local/main.omg
omega run --both samples/cli/facts/sixteen_variant_enum_payload_bound/field/main.omg
omega run --both samples/cli/facts/sixteen_variant_enum_payload_bound/parameter/main.omg
omega run --both samples/cli/facts/sixteen_variant_enum_payload_bound/call_result/main.omg
omega run --both samples/cli/facts/sixteen_variant_enum_payload_bound/match_join/main.omg
omega run --both samples/cli/facts/sixteen_variant_enum_payload_bound/const_name/main.omg
omega run --both samples/cli/facts/sixteen_variant_enum_payload_bound/other_state/main.omg
omega run --both samples/cli/facts/sixteen_variant_enum_payload_bound/other_machine/main.omg
omega run --both samples/cli/facts/sixteen_variant_enum_payload_bound/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, evaluating `probe(Packet::V16 { v: 60u64 })` extracts `res = 60u64`.
Because all sixteen variant arms enforce `v >= 40u64 && v <= 80u64`, the exhaustive match statically establishes `res in [40, 80]`.
Subsequent exact subtractions in `verify_payload` (`headroom = 80u64 - res = 20u64` and `margin = res - 40u64 = 20u64`) evaluate to `20u64 + 20u64 = 40u64`.
The transition guard `combined == 40u64` verifies exact arithmetic evaluation, transitioning to `done()` with native exit 0.
If payload extraction or the verification arithmetic is corrupted, or the acceptance condition is not met, the guard fails and transitions to `bad() { crash Trap; }`, terminating with exit 255 (measured on the `local` place with the guard flipped: `native signal 5` and `interp crash: Trap`).

### Invalid controls and reject twins

Each place has an `invalid/` reject twin that widens one variant's bound (`V16(v: u64) where v >= 39u64 && v <= 80u64`, or `>= 39u64 && v <= MAX_RES` in the `const_name` place).
When `v == 39u64`, `res` can fall to `39u64 < 40u64`, which violates the contract precondition `value >= 40u64` required by `verify_payload` (or the ensures postcondition on helper extraction).
The compiler strictly fails closed at compile time; each twin's `expected.txt` holds its diagnostic
verbatim as the source-check diagnostic record carries it (the single message line; the CLI prints it
under a `cannot check prepared project: ...` header) and the fail fixture copies that file
byte-for-byte, so every fail row pins the corpus's `expected` verdict:

| Place | `omega run --both` | Reject twin diagnostic (verbatim `expected.txt` fragment) |
| --- | --- | --- |
| `local` | native 0 / interp 0 | `cannot prove requires contract for call verify_payload from probe: local_res >= 40 && local_res <= 80 (needs fact `value >= 40 && value <= 80` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry` |
| `field` | native 0 / interp 0 | `cannot prove requires contract for call verify_payload from probe: holder.res >= 40 && holder.res <= 80 (needs fact `value >= 40 && value <= 80` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry` |
| `parameter` | native 0 / interp 0 | `cannot prove requires contract for call consume from probe: res >= 40 && res <= 80 (needs fact `forwarded >= 40 && forwarded <= 80` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry` |
| `call_result` | native 0 / interp 0 | `cannot prove ensures contract for exit from extract_payload at statement 31: result >= 40 && result <= 80` |
| `match_join` | native 0 / interp 0 | `cannot prove requires contract for call choose from probe: res >= 40 && res <= 80 && 60 >= 40 && 60 <= 80 (needs fact `first >= 40 && first <= 80 && second >= 40 && second <= 80` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry` |
| `const_name` | native 0 / interp 0 | `cannot prove requires contract for call verify_payload from probe: res >= 40 && res <= 80 (needs fact `value >= 40 && value <= 80` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry` |
| `other_state` | native 0 / interp 0 | `cannot prove requires contract for call consume from probe: res >= 40 && res <= 80 (needs fact `forwarded >= 40 && forwarded <= 80` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry` |
| `other_machine` | native 0 / interp 0 | `cannot prove requires contract for call forward_payload from probe: res >= 40 && res <= 80 (needs fact `mid >= 40 && mid <= 80` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry` |
| `target_body` | native 0 / interp 0 | `cannot prove requires contract for call verify_payload from target_probe: res >= 40 && res <= 80 (needs fact `value >= 40 && value <= 80` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry` (4 diagnostics, one per target-scoped body) |

The same nine twins are also mirrored under `invalid/<place>/`, byte-identical to their per-place twin
(`main.omg`, `build.omg`, `sample.json`, `expected.txt`), so the invalid controls are discoverable by
both the per-place route and the batch route.

## Pinned Fixtures

The fact row is pinned under `tests/omega/` as 18 corpus fixtures:

- 9 positive pass fixtures: `tests/omega/pass/facts/sixteen_variant_enum_payload_bound_through_<place>_exit/` carrying the place's `main.omg`, `build.omg` (`ProgramEntry` bound for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and `expected_native_exit.txt` = `0`; every file is byte-identical to the sample place file it names.
- 9 reject fail fixtures: `tests/omega/fail/facts/sixteen_variant_enum_payload_bound_through_<place>_rejected/` carrying the place twin's `main.omg`, `build.omg` and `expected.txt` (the verbatim diagnostic above); every file is byte-identical to the twin file it names.

Positive versus twin `git diff` changes only the `V16` case bound (`v >= 40u64` to `v >= 39u64`, and in
the `const_name` place the `MIN_RES`-spelled lower bound to the literal `39u64`) plus the header
comment, which is the fact's own domain.

## Requirements and Evidence (AGENTS.md)

- **Establishment**: Variant declarations `V1(v: u64)` through `V16(v: u64)` where `v >= 40u64 && v <= 80u64` establish bounded intervals on active variant payloads. Exhaustive pattern matching across all sixteen tagged enum variants joins the incoming arm obligations, statically proving that the extracted payload satisfies `[40, 80]` without post-hoc caller equality guards.
- **Identity and lifetime**: The extracted payload interval fact `res in [40, 80]` is preserved across local bindings, carrier record fields, machine parameters, helper call returns, match join arms, named constants, state transitions, helper forwarding machines, and target-scoped bodies without semantic loss.
- **Composition**: In each place, the synthesized interval `res in [40, 80]` composes directly with `verify_payload` contract preconditions and subsequent exact subtractions `headroom = 80u64 - res` and `margin = res - 40u64` to statically discharge underflow safety and non-negativity without post-hoc caller equality guards.
- **Soundness boundaries**: In every reject twin, widening the payload bound on variant `V16` to `v >= 39u64` permits `res` to reach `39u64`, which fails closed under contract requires preconditions or ensures postconditions across boundary transmissions, verifying that out-of-interval variant payloads cannot satisfy safety bounds.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`)**: Tagged enum construction, variant discrimination, and payload access lower to target-neutral Terminal Psi tagged case representations (`PSITERM\0`), while proof-only where-clauses are erased. Both Terminal Psi interpreter and native execution agree with exit 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)**: Provider layout plans represent the 16-variant tagged enum `Packet` with its discriminant tag and 8-byte payload (`IntegerAt 8-byte` for `u64`), respecting layout alignment and calling conventions across all four hosted platforms.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)**: Backend lowering selects target ISA variant tag comparison and branch instructions alongside 64-bit integer moves and exact subtractions: `sub rax, ...` on `x86_64` and `sub x0, ...` on `arm64`, maintaining disjoint register homes and activation stack frames without spilling corruption.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)**: Direct binary emission encodes machine instructions, resolves symbolic stack-frame displacements and basic block branches, validates transitive machine-state footprints, and emits PE/ELF/Mach-O images that execute natively and agree with Terminal Psi on exit 0 across all four hosted platforms.
- **Remaining Open**: None for this row; all nine places and nine reject twins pass both native and
  interpreter gates with documented exit 0 and verified negative control rejection.

### Verification Evidence

Measured on macOS arm64 with the factsr1 shared generation producer `gen-9b001b8f6af` (revision
`9b001b8f6afa2cdac67fd580033c3560d906a5d5`, the wave base; omega sha256
`6d24393125b7847b2f46d2f25e856f18a644f0fa1cedd2ad9b54f316f8b7da74`, corpus_runner sha256
`cee00bbdce17fc300b548aca249c2e9e2a44e59c17e97186055da76c3947444e`):

- `omega run --both` over the root and the nine positive places: 10/10 report `native exit 0` and
  `interp exit: 0`, exit 0.
- `omega --check` over the nine twins: 9/9 refuse, exit 1, each with its own authored diagnostic (the
  `target_body` twin emits four diagnostics, one per target-scoped body); the `invalid/expected.txt`
  written for each twin and mirror is byte-identical to the measured output.
- Discrimination: an authored probe of the `local` place whose acceptance guard is flipped
  (`transition combined == 39u64`) stays accepted by `--check` (exit 0), then runs to `native signal 5` /
  `interp crash: Trap (host presentation agrees)` with exit 255 — the authored exit 0 is not an
  unconstrained default.
- The row's 18 corpus fixtures are byte-identical copies of the sample place and twin files (54/54 file
  pairs verified with `sha256sum`).
- The three corpus legs were recorded with the fixture and golden changes staged at published main
  `9b001b8f6afa2cdac67fd580033c3560d906a5d5`, so every row is stamped `@9b001b8f6afa+wt`. Counting this
  row and `signed_8bit_wrapping_subtraction_bound` together (18 fixtures each):
  `tests/omega/corpus_outcomes.txt` gained 36 rows (+60/-0 — 18 `checked ... check-target:macos_arm64`,
  18 `rejected ... expected` and 24 diagnostic continuation lines, 12 per row: eight single-diagnostic
  twins plus the four target-body diagnostics), `tests/omega/corpus_native_macos_arm64.txt` gained 18
  `built ... exit:0` rows (+18/-0) and `tests/omega/corpus_both_macos_arm64.txt` gained 18
  `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows (+18/-0). All three diff as pure
  additions, all three legs exited 0 with 0 `timeout` rows and 0 rows recording a lost success; the
  native leg's 2 `regressed-from` rows are its pre-existing memory-canary pins. The first record pinned
  the fail rows with the corpus `unexpected` verdict because their `expected.txt` carried the CLI's
  two-line presentation instead of the diagnostic message; after the 18 fragments were rewritten, a
  scoped re-record of those rows (pure addition, +60/-0 unchanged) pins `expected` on all 18.
