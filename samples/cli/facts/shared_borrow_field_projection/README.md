# Shared Borrow Field Projection

Projecting a bounded field through a shared borrow preserves its exact static upper bound without moving the backing record, discharging exact subtraction `90u64 - used` across all nine places.

Every place performs the same operation: construct a record `window: Window = Window { used: 40 }` bounded by `where used <= 90`, take a shared borrow `&window`, transport the reference or its projected field through the place, project `view.used` to observe `used <= 90`, perform exact subtraction `90u64 - used` (yielding `50u64`), and verify in `main` that `diff == 50` exiting 0. Only the transport mechanism of the shared reference or its projected field changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root shared borrow field projection |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the shared borrow is held in a local binding, copied to a second local, and projected there |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the shared borrow is stored in a struct field and projected through the member access |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the shared borrow is passed as a machine parameter and projected in the callee |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the shared borrow arrives as the return result of a helper machine and is projected by the caller |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms yield the shared borrow so the joined reference projects the bounded field safely |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant `LIMIT: u64 = 90` provides the exact minuend for the projected field subtraction |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a transition forwards the shared borrow to a successor machine that projects the field |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the shared borrow to an intermediate machine for projection |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the shared borrow field projection and exact subtraction |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/shared_borrow_field_projection/main.omg
omega run --both samples/cli/facts/shared_borrow_field_projection/local/main.omg
omega run --both samples/cli/facts/shared_borrow_field_projection/field/main.omg
omega run --both samples/cli/facts/shared_borrow_field_projection/parameter/main.omg
omega run --both samples/cli/facts/shared_borrow_field_projection/call_result/main.omg
omega run --both samples/cli/facts/shared_borrow_field_projection/match_join/main.omg
omega run --both samples/cli/facts/shared_borrow_field_projection/const_name/main.omg
omega run --both samples/cli/facts/shared_borrow_field_projection/other_state/main.omg
omega run --both samples/cli/facts/shared_borrow_field_projection/other_machine/main.omg
omega run --both samples/cli/facts/shared_borrow_field_projection/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating across both the observable runtime value and the static proof obligation:
- **Observable value discrimination (1-token consumer perturbation):** In every positive place, `Window` is bounded with `where used <= 90` and initialized with `used: 40`. Shared borrow field projection yields `view.used == 40u64`, and exact subtraction `90u64 - used` produces `50u64`. In `main`, `transition diff == 50` transitions to `machine done() {}`, terminating with `native exit 0` and `interp exit: 0`. Perturbing the demanded value by 1 token (`transition diff == 50` -> `transition diff == 51` in `local/main.omg`) traps both engines (`native exit -1073741795`, `interp crash: Trap (host presentation agrees)`), proving the runtime oracle reads the projected field and executes the subtraction rather than succeeding vacuously.
- **Obligation class and soundness discrimination (1-token bound perturbation):** This fact belongs to the **value/arithmetic bound** obligation class: the record invariant `where used <= 90` is preserved through shared-borrow field projection (`view.used`) to discharge the compile-time exact subtraction proof obligation on `90u64 - used`. Each invalid twin widens only the construct's field bound by 1 token (`where used <= 90` -> `where used <= 91`) with no caller guard changes. Because `90u64 - used` could underflow if `used == 91`, compile-time proof checking fails closed with `exact arithmetic in machine ... may overflow \`u64\`` before any runtime realization.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/shared_borrow_field_projection/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\cargo-target\debug\omega-latest.exe --filter cli/facts/shared_borrow_field_projection --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 18/18 intended-invalid controls match their authored rejection (`may overflow \`u64\``).

## Pinned Fixtures

Each of the nine places is pinned twice in the compiler corpus:
- **Positive native-exit fixtures:** `tests/omega/pass/facts/shared_borrow_field_projection_through_<place>_exit/` (`build.omg`, `main.omg`, `expected_native_exit.txt`), binding `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and recorded in `tests/omega/corpus_native_windows_x86_64.txt` (`built ... exit:0`), `tests/omega/corpus_native_macos_arm64.txt` (`built ... exit:0`), and `tests/omega/corpus_outcomes.txt` (`checked`).
- **Reject twin fixtures:** `tests/omega/fail/facts/shared_borrow_field_projection_through_<place>_rejected/` (`build.omg`, `main.omg`, `expected.txt`), recorded in `tests/omega/corpus_outcomes.txt` (`rejected`).

Both host-pinned native goldens are the measurement:

```sh
python3 tools/corpus_gate.py --native --filter facts/shared_borrow_field_projection
# clean — 9 fixtures ({'built': 9}) ... executed 9 (exit codes {'0': 9})
python3 tools/corpus_gate.py --filter facts/shared_borrow_field_projection
# clean — 18 fixtures ({'checked': 9, 'rejected': 9}), 0 rejected without their expected fragment
```

### The projection carries the bound (discrimination)

Engine agreement (`omega run --both` exit 0) shows the program runs; it does not
by itself show that the *projection* carries the bound rather than the constant
surviving folding. The halves below separate those.

- **Static half (licensing).** Breaking the construct's own contract — widening
  the record's declared domain to `data Window where used <= 91` while leaving
  the shared borrow, the projection, and the consumer untouched — is refused
  *before native production*:

  ```
  error: exact arithmetic in machine `evaluate` state `entry` local `diff`
  may overflow `u64`: the operands are not provably in range
  (decision 17 -- exact arithmetic is a proof obligation)
  ```

  That is the pinned twin's diagnostic, so the twin measures the obligation
  rather than an unrelated refusal. The obligation's kind decides the class: it
  is a `where`-clause construction fact, so **no runtime trap is reachable**.
  Seeding the field from an opaque helper (`Window { used: seed() }`) is refused
  at the *construction* gate — `a `where` fact is not established by the values
  observed at this construction` — and an out-of-domain literal (`used: 91`) is
  refused at the same gate. `construction_bounds.rs` folds the `where` fact over
  field-value intervals, so seeding cannot defeat it. The refusal is STATIC.

- **Shared borrow refuses a write (access mode is enforced).** Mutating through
  the shared projection is refused statically:

  ```
  error: machine `evaluate` state `entry` assignment requires mutable source
  access through every reference crossed by its destination
  ```

  The control shows this is about access mode rather than a general write
  defect: the identical write through `&mut Window` is accepted and runs green
  in both engines, and making that mutable write observable (`transition diff ==
  49`) while asserting the un-mutated value (`diff == 50`) traps at runtime in
  both engines — `native signal 5` / `interp crash: Trap (host presentation
  agrees)`. The mutable write therefore genuinely lands, while the shared one
  cannot be formed at all. This refusal is the read-only counterpart to the
  `mutable_borrow_field_writeback` row and matches that lane's independent
  observation word for word.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for shared borrow `&` structural field projection and read-only loan authority. Specifically, projecting a bounded field (`view.used <= 90u64`) through a shared borrow (`&Window`) preserves its exact static upper bound without moving or consuming the backing record, discharging exact subtraction `90u64 - used` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded record definition (`data Window where used <= 90, { used: u64; }`) establishes the invariant on valid instances. Creating a shared borrow `&window` introduces a shared reference whose projected field access `view.used` directly carries the field invariant bound (`used <= 90`) without requiring any post-hoc caller equality guard (`transition x == 50`) to justify forming the exact subtraction `90u64 - used`.
  - **Identity and lifetime:** The backing record `window` retains its identity and storage throughout the loan window. The shared borrow `&Window` provides non-exclusive, read-only loan authority without consuming, moving, or invalidating the backing record or its other fields.
  - **Composition:** The projected field bound composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction (`90u64 - used`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the field bound on the record (`used <= 90` vs `used <= 91`), leaving exact subtraction unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `used <= 91` is specified: exact subtraction `90u64 - used` underflows below zero if `used == 91`, strictly rejecting with `exact arithmetic in machine ... may overflow \`u64\``. No unproved underflow or invalid loan reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Shared borrow creation (`BorrowShared`), structural field projection (`ProjectField`), and integer subtraction lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving reference addressing and field dereferencing. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The record layout plan (`Window`: single `u64` field at offset 0, alignment 8; `ViewHolder`: reference pointer field at offset 0, alignment 8) validates, and target `CallPlan`/`StatePlan` assign reference pointers and return values across target registers and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA address loading and memory dereference instructions (`mov rax, [rbp - disp]` and `mov rdx, [rax + offset]` on `x86_64`; `ldr` on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA pointer dereference and subtraction instructions encode into machine bytes, basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Mutable field projection reborrowing windows, multi-level nested struct borrow projections with cross-field dependent invariants, and dynamic slice sub-element loans.
