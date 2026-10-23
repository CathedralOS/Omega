# Generic selected invocation crash contracts

Owning requirement: **CRASH-CONTRACT**, generic selected-use joins and exact
invocation-entry operands through public package review and execution.

Fact: a generic selected Reader<Zero>::read call preserves the entry i32 >= 0
fact to discharge its authored value < Zero Trap route.

The requirement owns `crashes Trap value < Limit`. The public consumer selects
`Implementation satisfies Reader<Zero>` and requires a nonnegative input. The
provider's harmless implementation cannot remove this public obligation. Every
reject twin changes only `< Limit` to `<= Limit`; zero then leaves an uncovered
route under the same consumer precondition. The runtime equality checks observe
the authored result; they do not establish the fact used to accept the call.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [local](local/main.omg) | [invalid](local/invalid/main.omg) |
| Field | [field](field/main.omg) | [invalid](field/invalid/main.omg) |
| Parameter | [parameter](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| Call result | [call_result](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| Match join | [match_join](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| Const name | [const_name](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| Another state | [other_state](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| Another machine | [other_machine](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| Another target body | [target_body](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

The row is pinned under `tests/omega/{pass,fail}/facts/selected_crash_formal_through_*`.
All nine positives and the root bind all four hosted targets and expect exit 0.
Only the available Linux x86-64 host supplies native execution evidence here.

## Requirements established by this row

- **Establishment:** the caller's independent entry requirement justifies the
  comparison complement. Fixed-integer comparison meaning comes from the exact
  checked declaration; token spelling supplies no authority.
- **Identity and lifetime:** selected requirement formals substitute authenticated
  invocation operands. [saved_actual](saved_actual/main.omg) additionally copies
  the entry value before a write; its [twin](saved_actual/invalid/main.omg) passes
  changed storage and must reject. A bare readable scalar reference observes
  the referent through its structural root, never a fictional scalar slot.
- **Composition:** the completed identity call's `result == value` guarantee
  transports the exact normal result into the enclosing selected call. Named
  states, common match origins and forwarded calls retain the same operation.
- **Soundness boundaries:** the checked executable predicate algebra excludes
  erased mathematics and structural proof equality from integer complementation.
  No provider body supplies a public guarantee or narrows a requirement ceiling.
- **Checked to Terminal Psi:** ordinary checked call correspondence and lowering
  retain evaluated arguments, the selected declaration and contract commitment.
  Terminal execution must agree with the authored native result.
- **Provider settlement, layout and ABI:** public review projects `Reader<Zero>`
  by the trait's exact const telescope slot using checked canonical values. The
  existing scalar and borrowed-field calling plans supply the exercised ABI.
- **Instruction selection and frames:** the selected calls, scalar snapshots,
  borrowed reads and normal-return values use the ordinary Linux x86-64 path.
- **Encoding and native execution:** the emitted image executes the oracle;
  merely checking or emitting it does not establish this layer.

[permitted_return](permitted_return/main.omg) retains a true permitted route
while the selected provider returns normally. [permitted_crash](permitted_crash/main.omg)
uses the same public guard and actually crashes: the native illegal-instruction
presentation and Terminal `Trap` must agree. A permitted guard is not an event.

## Baseline and remaining scope

At `d6cf554fd1cbf20551983d29bae26f5cf3b81664`, the local, nested call-result
and saved-actual positives wrongly reject with an uncovered Trap call route.
The covered-route variant checks, but public review rejects its named const
argument as a residual const declaration in structural type identity.

This row does not close **CRASH-CONTRACT**. Provider-backed structural selection,
other intrinsic spellings, opaque/content provenance, compound ordered completion
and the task's remaining executed-outcome obligations remain open. A separate
exploratory generic provider body with an open const guard still rejects its
own crash site; it is not claimed as completed by this selected-consumer row.
Run identities, commands, source hashes and actual engine outcomes are retained
in qualification evidence rather than inferred from folder presence.
