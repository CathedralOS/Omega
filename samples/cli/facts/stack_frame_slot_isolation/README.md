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
