# In-place view argument

Owner: `RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY` in [TASKS.md](../../../../TASKS.md),
the requirement to lower the call argument `Input { bytes: &main.packet }` that
[`traits/runtime_dyn_stored_byte_input`](../../../../tests/omega/pass/traits/runtime_dyn_stored_byte_input/main.omg)
builds in place.

Fact: a shared byte view formed inside a constructed call argument borrows the
caller's exact backing array for that invocation, so the callee proves its
extent requirement from the formation and reads the authored bytes.

Rules: [structural access](../../../../wiki/spec/terminal-psi/structural_access.md),
[loans](../../../../wiki/spec/terminal-psi/loans.md) and
[byte views](../../../../wiki/spec/terminal-psi/byte_views.md).

| Place | Positive | Reject twin (3-byte backing) |
| --- | --- | --- |
| Local | [source](local/main.omg) | [twin](local/invalid/main.omg) |
| Field | [source](field/main.omg) | [twin](field/invalid/main.omg) |
| Parameter | [source](parameter/main.omg) | [twin](parameter/invalid/main.omg) |
| Call result | [source](call_result/main.omg) | [twin](call_result/invalid/main.omg) |
| Match join | [source](match_join/main.omg) | [twin](match_join/invalid/main.omg) |
| Const name | [source](const_name/main.omg) | [twin](const_name/invalid/main.omg) |
| Nested machine | [source](other_state/main.omg) | [twin](other_state/invalid/main.omg) |
| Other machine | [source](other_machine/main.omg) | [twin](other_machine/invalid/main.omg) |
| Selected implementation | [source](target_body/main.omg) | [twin](target_body/invalid/main.omg) |

Every place passes `Input { tag, bytes: &packet, tail }` straight into the call.
The backing array is a local, a field of the caller's borrowed record (between
canaries 3 and 4), an owned parameter, a call result, the array each match arm
selects (29 or 31 at index 2), an array sized and filled by named constants,
storage owned by a nested machine, storage owned by another top-level machine,
and an argument to `X86Reader`/`ArmReader` selected by target-keyed Build rows.
The callee returns byte 2 only when the record's `tag` and `tail` canaries are
5 and 9; otherwise it returns 0. Each `main` calls it with tag 5 and with tag 6,
so both arms run, and traps unless it observes 29 (31 for the second match arm)
and 0. A wrong byte, a dropped canary or a swapped arm traps in both engines;
changing the expected byte to 12 in the local shape traps natively (SIGILL) and
in Terminal Psi (`Trap`).

Each twin changes only the backing array's extent from 4 to 3 bytes. The formed
view then carries length 3 and the callee's `requires packet.bytes.len >= 4`
fails with ``needs fact `packet.bytes.len >= 4` here``.

## Baseline and repair

On base `ca41410e2`, the checker already rejected all nine twins, and all nine
positives refused lowering: `call operation: structural arguments: retained
temporary` (`boundary argument parameter` for the selected implementation).
The formation `&packet` has no statement loan, because no local owns it. Borrow
admission records it only in the call's ordered access roster. Value planning
(`scalar_computations/reference_captures.rs`) looked for statement loans,
returned loans and call-loan resources; those resources are installed after
value planning, so the construction never received a structural value and the
call could not schedule it. The repair reads the call's access row for the
formation occurrence itself, when that access is held through the invocation.
A scalar reference field already worked because its capture reads the referent
place directly. Binding the record to a local first also worked because the
local owns a statement loan.

## Requirements

- **Establishment:** the formation `&packet` establishes the view's length from
  the array's extent. No caller guard restates it; the twins remove it by
  shrinking the array.
- **Identity and lifetime:** the view names the caller's exact array for the
  invocation. Mutating that array through another argument of the same call
  still rejects (`receives packet as both mutable and read-only`).
- **Composition:** the requirement is proved at the call from the constructed
  argument's field; match arms each establish their own backing.
- **Psi 05–07 / Omega 00–09:** each positive executes through Terminal Psi and
  the Linux x86-64 native image with exit 0 (`omega run --both`).

All four hosted `ProgramEntry` rows are authored; executions were observed on
Linux x86-64 only. A mutable `&mut [u8]` field formed in place
(`fill(Output { bytes: &mut packet })`) and a view nested two records deep were
checked by probes but are not pinned rows. Copies and oracles are pinned under
`tests/omega/{pass,fail}/facts/in_place_view_argument_through_<place>_{exit,rejected}`.
