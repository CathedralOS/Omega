# Stack frame slot isolation

Fact: three co-resident stack-backed record homes mutated independently through exclusive borrows preserve disjoint field values summing to 100u64 when subtracting 100u64.

Owning outcome: `CONSERVATION-CONTRACT`
Ticket: `65fd83b08dbb466891e09aaa82e9fea1`

## Requirement and Evidence

Under `wiki/spec/resources/storage.md#compiler-owned-stack-accesses`, every compiler-owned stack slot in an activation frame occupies a disjoint byte interval within the validated frame extent. Mutating one stack-backed home through an exclusive borrow (`&mut`) must never overwrite or alias an adjacent stack-backed home. Furthermore, under `wiki/spec/terminal-psi/structural_access.md#referent-identity`, borrowed arguments pass the original referent's storage address (`ValueClass::BorrowedReference`), forcing the referent local into an addressable stack frame home.

This fact row exercises three co-resident local `SlotPair` records (`slot_a`, `slot_b`, `slot_c`) allocated within the same activation frame. Mutating `slot_b` exclusively via `&mut slot_b` must preserve `slot_a` and `slot_c` uncorrupted. The combined sum of fields `slot_a.left + slot_a.right + slot_b.left + slot_b.right + slot_c.left + slot_c.right` equals 100u64 and is transported through each of the nine canonical places:

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| root (anchor) | [source](main.omg) | - | Root reference row exercising slot isolation across three co-resident records. |
| local | [source](local/main.omg) | [source](local/invalid/main.omg) | The isolated slot sum is bound to a local before exact subtraction. |
| field | [source](field/main.omg) | [source](field/invalid/main.omg) | A record field carries the isolated slot sum before exact subtraction. |
| parameter | [source](parameter/main.omg) | [source](parameter/invalid/main.omg) | The reader receives the isolated slot sum as a parameter. |
| call_result | [source](call_result/main.omg) | [source](call_result/invalid/main.omg) | The isolated slot sum is returned from a helper call with an ensures contract. |
| match_join | [source](match_join/main.omg) | [source](match_join/invalid/main.omg) | Both match arms produce the isolated slot sum. |
| const_name | [source](const_name/main.omg) | [source](const_name/invalid/main.omg) | Named constants preserve the isolated slot sum computation. |
| other_state | [source](other_state/main.omg) | [source](other_state/invalid/main.omg) | A transition forwards the isolated slot sum to a successor machine. |
| other_machine | [source](other_machine/main.omg) | [source](other_machine/invalid/main.omg) | A separate machine verifies and performs the exact subtraction. |
| target_body | [source](target_body/main.omg) | [source](target_body/invalid/main.omg) | Each hosted target body executes the exact subtraction. |

Each place establishes that `isolated - 100u64` performs exact subtraction without underflow and yields 0.

Reject twins in each place weaken the mutated write or target bound (producing a 99u64 sum), causing exact subtraction `isolated - 100u64` to reject at compile time with `exact subtraction underflow` (`exact arithmetic may overflow u64`).

Pinned fixtures:
- Pass: `tests/omega/pass/facts/stack_frame_slot_isolation_through_<place>_exit/`
- Fail: `tests/omega/fail/facts/stack_frame_slot_isolation_unproved_<place>/`

## Run record

Producer revision `952bc99fb970`, rebased onto incoming main `43c5fdff6670`
(a Psi contract-refinement change, so the compiler was rebuilt and every
observation below was retaken rather than reused). Host `macos_arm64`, compiler
`./target/debug/omega` with `OMEGA_LIBRARY_ROOT=<worktree>/source/library` and
reported build profile `{"opt_level":"1"}`.

Every positive place and root `main.omg` binds `ProgramEntry` for all four
hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`)
in its `build.omg` and declares `Expected exit: 0` in its header. Measured with
`omega run --both` (`< /dev/null`), root and all nine places report
`native exit 0` together with `interp exit: 0` — Terminal Psi and native
execution agree at exit 0 in ten of ten positives. All nine
`<place>/invalid/main.omg` twins exit 200 with the pinned diagnostic
`exact arithmetic in machine ... may overflow u64` (`target_body` carries four
such diagnostics, the others one each).

Gate numbers on this revision:

- `python3 tools/sample_native_gate.py --facts --filter stack_frame_slot_isolation`
  → `10/10 samples meet their documented oracle on macos_arm64 (0 unscored)`;
  `18/18 intended-invalid controls match their authored rejection`.
- `python3 tools/corpus_gate.py --native --filter facts/stack_frame_slot_isolation`
  → nine `pass/facts/stack_frame_slot_isolation_through_<place>_exit` rows
  recorded as `built <ms> exit:0` in `tests/omega/corpus_native_macos_arm64.txt`.
- `python3 tools/corpus_gate.py --filter facts/stack_frame_slot_isolation`
  → 18 fixtures `{'checked': 9, 'rejected': 9}`, with `0 fail fixtures did not
  reject` and `0 rejected without their expected fragment`.

## Discrimination proof

The oracle is proven to discriminate on the values themselves rather than to
hold for any program of this shape. A positive probe (`all three homes intact`,
`write_slot(&mut slot_b, 15, 25)`) observes `native exit 0` and `interp exit: 0`.
Changing **only the construct's own write value**, `15` to `14`, and nothing
else — the caller's `transition` guards and the `run_check` `requires` clause
still expect `15` — flips the observation to `native signal 5` and
`interp crash: Trap (host presentation agrees)`. Independently moving
`slot_a`/`slot_c` from `{10,20}` to `{15,25}` while leaving `slot_b`'s write
untouched also traps both engines. So each home's value is separately observed
and none aliases another: had the frame slots aliased, corrupting one live value
while the others stayed live could not have changed the observed result.

## Requirements and Evidence

- **Establishment:** `write_slot` carries `ensures target.left == next_left && target.right == next_right`, so the post-state of the borrowed referent is established by the call rather than re-derived by a caller equality guard. Initializing disjoint local stack-backed bindings (`slot_a`, `slot_b`, `slot_c`) establishes independent value and range facts on each slot directly without any post-hoc caller equality guard; each place's `evaluate` proves `requires isolated == 100` from the transported sum and establishes `ensures result == 0` on `isolated - 100u64`.
- **Identity and lifetime:** The fact names three distinct storage versions (`slot_a`, `slot_b`, `slot_c`) that are live across the same call; mutating `slot_b` exclusively through `&mut slot_b` preserves the static facts and values of provably disjoint sibling locals throughout their lifetimes, as confirmed by discrimination observations.
- **Composition:** Isolated stack-frame slot facts compose through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operation (`isolated - 100u64`); `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in the target slot's bound or mutation (producing a 99u64 sum), never in a caller guard.
- **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time (`exact arithmetic in machine ... may overflow \`u64\``), strictly preventing unproved subtraction when a write to the consumed slot weakens its bound or causes an underflow.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Distinct local storage roots, exclusive borrows (`&mut`), and mutations lower into canonical target-neutral `PSITERM\0` while erasing proof-only obligations, executing with identical semantics in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), local carrier layout geometry (`SlotPair` size 16, align 8) evaluates and validates, and `CallPlan`/`StatePlan` lower carriers across target registers and stack slots.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering assigns non-overlapping byte intervals (`[offset, offset + size)`) in the activation frame for disjoint locals, composes worst-case stack usage (WCSU), and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Distinct symbolic stack-frame displacements resolve into non-aliasing `[rsp + disp]` / `[sp, #imm]` memory operands, basic blocks and branches relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across hosted targets including `windows_x86_64` and `macos_arm64`.

Open scope: this row establishes disjoint homes for three co-resident records in
one activation frame on the exercised places. Wider frame geometries (nested
borrows under spilling, callee-saved pressure across the same call) remain
unexercised and belong to the same requirement when a witness reaches them.
