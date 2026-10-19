# Stack frame slot isolation

Fact: three co-resident stack-backed record homes mutated independently through exclusive borrows preserve disjoint field values summing to 100u64 when subtracting 100u64.

Owning outcome: `CONSERVATION-CONTRACT`
Ticket: `65fd83b08dbb466891e09aaa82e9fea1`

## Requirement and Evidence

Under `wiki/spec/resources/storage.md#compiler-owned-stack-accesses`, every compiler-owned stack slot in an activation frame occupies a disjoint byte interval within the validated frame extent. Mutating one stack-backed home through an exclusive borrow (`&mut`) must never overwrite or alias an adjacent stack-backed home. Furthermore, under `wiki/spec/terminal-psi/structural_access.md#referent-identity`, borrowed arguments pass the original referent's storage address (`ValueClass::BorrowedReference`), forcing the referent local into an addressable stack frame home.

This fact row exercises three co-resident local `SlotPair` records (`slot_a`, `slot_b`, `slot_c`) allocated within the same activation frame. Mutating `slot_b` exclusively via `&mut slot_b` must preserve `slot_a` and `slot_c` uncorrupted. The combined sum of fields `slot_a.left + slot_a.right + slot_b.left + slot_b.right + slot_c.left + slot_c.right` equals 100u64 and is transported through each of the nine canonical places:

1. `local`: [local/main.omg](local/main.omg)
2. `field`: [field/main.omg](field/main.omg)
3. `parameter`: [parameter/main.omg](parameter/main.omg)
4. `call_result`: [call_result/main.omg](call_result/main.omg)
5. `match_join`: [match_join/main.omg](match_join/main.omg)
6. `const_name`: [const_name/main.omg](const_name/main.omg)
7. `other_state`: [other_state/main.omg](other_state/main.omg)
8. `other_machine`: [other_machine/main.omg](other_machine/main.omg)
9. `target_body`: [target_body/main.omg](target_body/main.omg)

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

- **Establishment.** `write_slot` carries
  `ensures target.left == next_left && target.right == next_right`, so the
  post-state of the borrowed referent is established by the call rather than
  re-derived by a caller equality guard. Each place's `evaluate` proves
  `requires isolated == 100` from the transported sum and establishes
  `ensures result == 0` on `isolated - 100u64`; the consuming operation is the
  subtraction itself.
- **Identity and lifetime.** The fact names three distinct storage versions
  (`slot_a`, `slot_b`, `slot_c`) that are live across the same call. The write
  through `&mut slot_b` changes only `slot_b`'s version; the `slot_a` and
  `slot_c` facts survive it, and the discrimination proof above observes each
  version independently while all three are live.
- **Composition.** The exact actuals are substituted into `evaluate`, each join
  retains only facts justified on the incoming path, and each
  `<place>/main.omg` differs from its `<place>/invalid/main.omg` only in the
  construct's own domain (`evaluate`'s `requires` bound and the mutated write
  value, producing a 99u64 sum) — never in a caller guard.
- **Soundness boundaries.** Ownership and access authority are preserved: the
  exclusive borrow of `slot_b` is the only mutable access, `slot_a` and `slot_c`
  are read through copies, and the twins reject rather than silently changing
  the arithmetic domain.
- **Terminal Psi and carrier lowering.** Each place lowers the borrowed
  argument and the record homes without loss into Terminal Psi; `local`,
  `field`, `parameter`, `call_result`, `match_join`, `const_name`,
  `other_state`, `other_machine` and `target_body` exercise the construct in a
  different neutral position, and every one executes identically in Terminal
  Psi and natively.
- **Native realization, frame/layout encoding and image emission.** The three
  co-resident homes are assigned disjoint frame slots and the exclusive-borrow
  write is realized to the correct address; the native leg of `omega run --both`
  and the recorded `built ... exit:0` rows show the emitted Mach-O image
  executing the authored result.

Open scope: this row establishes disjoint homes for three co-resident records in
one activation frame on the exercised places. Wider frame geometries (nested
borrows under spilling, callee-saved pressure across the same call) remain
unexercised and belong to the same requirement when a witness reaches them.
