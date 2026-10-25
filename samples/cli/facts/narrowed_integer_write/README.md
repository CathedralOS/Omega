# Narrowed integer writes

A completed write of 70 through a signed eight-bit field retains its i64 meaning
and leaves the adjacent byte unchanged.

Owner: **STATE-LOCAL-VALUE-FRONTIER** in [TASKS.md](../../../../TASKS.md).
The requirement is ordinary storage and call composition with exact physical
write-fit obligations. [IntegerAt](../../../../wiki/spec/layouts/plans.md)
keeps the semantic carrier: an `i64` loan does not license truncating its writes.
The programs use the bundled layout vocabulary, including its open generic
placement templates. Those templates must not acquire a concrete width merely
because this program also declares a narrowed field.

Every place writes 70 through the same signed eight-bit encoding, observes 70 in
its original byte, and verifies that the adjacent byte remains 85. The consuming
store must prove fit before it happens; the final comparison is only a runtime
oracle. Every twin changes only the supplied value from 70 to 200. It must
reject the physical write obligation, rather than report unknown layout reach
in an unrelated generic library declaration.

| Place | Transport | Reject twin |
| --- | --- | --- |
| [local](local/main.omg) | Local scalar [invalid](local/invalid/main.omg) |
| [field](field/main.omg) | Owned record field [invalid](field/invalid/main.omg) |
| [parameter](parameter/main.omg) | Forwarded call parameter [invalid](parameter/invalid/main.omg) |
| [call result](call_result/main.omg) | Completed identity result [invalid](call_result/invalid/main.omg) |
| [match join](match_join/main.omg) | Both normal-return arms, including an inline result [invalid](match_join/invalid/main.omg) |
| [const name](const_name/main.omg) | Named static value [invalid](const_name/invalid/main.omg) |
| [other state](other_state/main.omg) | Nested machine arrival [invalid](other_state/invalid/main.omg) |
| [other machine](other_machine/main.omg) | Generic forwarding with a separate type binder [invalid](other_machine/invalid/main.omg) |
| [target body](target_body/main.omg) | Checked body for each hosted target [invalid](target_body/invalid/main.omg) |

Each place has an `invalid/main.omg` twin and a corresponding pin under
`tests/omega/{pass,fail}/facts/narrowed_integer_write_through_<place>_*`.
`layouts/unused_generic_concrete_integer_at_requires_fit` additionally rejects
a concrete out-of-range write inside an unused generic template. Template
checking continues; only a symbolic type parameter's physical layout awaits
specialization.

The [completed-result control](controls/completed_result/main.omg) also copies
its scalar through an inline call before a later operand writes 70 to the old
source. Its twin starts that source at 200: the later 70 must not justify the
earlier result's physical store. Matching fixtures pin this snapshot boundary.
The [non-host-body control](controls/nonhost_body/invalid/main.omg) writes 200
only in the Linux x86-64 member while the other three members forward 70.
Psi must reject it even when macOS ARM64 is the realization target. The portable
interface's empty body cannot stand in for its members' performed writes.

## Requirements and evidence

- Establishment: validated signed eight-bit geometry and the completed scalar
  establish the performed-write fit requirement. The invalid value is 200.
- Identity and lifetime: field loans retain original byte storage and end
  before the byte/sentinel observations. No new access authority is introduced.
- Composition: every place transports the same value into the same store;
  result guarantees belong to exact completed invocations and both match arms.
- Soundness: source checks retain narrow-width fitting, unknown concrete-origin
  refusals, and concrete obligations inside open generic bodies.
- Psi 05–07: the checked store preserves semantic i64 meaning through Terminal
  lowering and execution, independently of its eight-bit physical encoding.
- Omega 00–02: the validated layout and ordinary call plans retain original
  storage geometry through the scalar reference.
- Omega 03–06: target instruction and frame realization write the selected byte,
  with no overwrite of its neighbor.
- Omega 07–09 and emission: native execution must agree with Terminal execution
  and the authored exit-zero/writeback oracle.

Run the row with `python3 tools/sample_native_gate.py --filter
cli/facts/narrowed_integer_write --jobs 2`. All four hosted targets have entry
bindings; native execution on other hosts remains separate qualification.

On macOS ARM64, producer built from base `6bbc0c024e2` plus this repair runs
all nine places and the completed-result control with native/Terminal exit 0.
All eleven invalid controls reject, including the non-host-body control.
Raw commands, producer hashes, intermediate refusals and focused corpus results
are retained with this lane's qualification evidence. Other-host execution and
the parent task's remaining cyclic, opaque and dependent composition stay open.
