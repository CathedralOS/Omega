# Indexed write-only reborrows

Fact: indexed write-only reborrows update the original selected cells to -43 and 9 while preserving padding 11 and 23.

Parent: **STRUCTURAL-BORROW-IDENTITY** in [TASKS.md](../../../../TASKS.md).
This row establishes original caller storage through named write-only descriptor
chains and indexed call preparation. It does not close the parent's remaining
generic fixed-array views, reference-recast qualification, or hosted matrix.

Each program starts with two `Cell` values, with padding 11/23 and contents
85/29. A parent write-only loan and its child denote the same original array.
The program selects element 1 twice, stores 7 then 9, changes the selector to 0,
and stores -43. Its oracle reads both original elements, both padding fields,
and the selector after the loans finish. A copied referent, a stale selector,
a missing store, or an incorrect field stride fails that oracle.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Root customer | [main](main.omg) | [invalid](invalid/main.omg) |
| Local | [main](local/main.omg) | [invalid](local/invalid/main.omg) |
| Field | [main](field/main.omg) | [invalid](field/invalid/main.omg) |
| Parameter | [main](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| Call result | [main](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| Match join | [main](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| Const name | [main](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| Nested machine | [main](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| Other machine | [main](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| Target body | [main](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

The [selector-expression production](selector_expression/main.omg), with its
[reject twin](selector_expression/invalid/main.omg), retains the same fact and
operation with `position + 0` selecting each element. This exercises the saved
inputs of the selector computation in addition to the direct reference read.
The [cast selector](cast_selector/main.omg) and [unary selector](unary_selector/main.omg)
retain those inputs through `position as u64` and `(~(~position)) % 2`.
Their respective [cast reject](cast_selector/invalid/main.omg) and
[unary reject](unary_selector/invalid/main.omg) enforce the same access obligation.
The remainder supplies the unary expression's ordinary bound of two; it leaves
both authored selected positions unchanged.

Every reject inserts a read of `cell.value` through the callee's existing
`&write Cell` parameter. It must reject specifically because write-only access
permits replacement without observation; a lowering refusal is not a passing
reject. The nine places and the selector production are pinned under
`tests/omega/{pass,fail}/facts/write_only_indexed_reborrow_*`.

## Requirements and evidence

- **Establishment:** all referents are valid initialized storage. Exact checked
  reference formation retains the immediate parent, access, and physical
  projection. The write-only child does not require readable referent contents.
- **Identity and lifetime:** each descriptor retains its original home and
  parent chain. Call preparation consumes those homes and the captured selector
  inputs. The final caller read occurs after the last use of the descriptors.
- **Composition:** the same two-element geometry, values, stores, and oracle
  survive each place. Parameter entry requires selector 1; subsequent assignment
  establishes selector 0 before the final call. No caller equality guard grants
  missing authority or justifies forming an index.
- **Soundness:** the existing access-supply relation still rejects widening a
  write-only reference into readable or mutable access. Each reject tests the
  same no-observation obligation at the consuming store machine.
- **Checked to Terminal Psi:** checked reference captures must publish each
  descriptor rather than erase the child as an alias. Completed selector and
  strict computation operands retain their exact occurrence graph for borrow
  checking; consumers do not scan unrelated expression occurrences. Casts and
  unary operations retain their completed child as the same occurrence.
  Borrowed block arrivals retain their exact parent permission chain in native
  custody replay. Suspended ancestors on that chain are authority for the
  access; independent loans and descendants still conflict, and captured
  lifetime debts remain enforced separately.
- **Provider, layout and ABI:** each positive binds the ordinary program entry
  for Windows x86-64, Linux x86-64, macOS ARM64, and Linux ARM64. Calls carry the
  original array/record addresses and preserve the two-field element stride.
- **Instruction and frame realization:** stores target the selected caller
  element and field, leaving adjacent padding and the other element intact.
- **Encoding and execution:** acceptance requires `omega run --both` exit 0
  through the public package-review route, with the authored caller readback.
  Actual producer/target observations are retained in the claim and qualification
  records; target bindings alone do not establish matching-host execution.

At fixed base `37dabdc19193fe27968bb97d5922118db61b68db`, the original
customer refused with `computed borrow lost its established local`. Of the ten
root/place positives, only the call-result production already ran with native
and Terminal exit 0; the other nine refused. All ten sensitive controls rejected
for the intended write-only read. The parameter production additionally exposed
a missing retained selector operand, rather than an invalid source program.

Run the row with:

```text
python tools/sample_native_gate.py --cli <optimized-omega> --filter write_only_indexed_reborrow --jobs 8
python tools/corpus_gate.py --runner <optimized-runner> --both --filter write_only_indexed_reborrow
```

Candidate validation and remaining hosted coverage are recorded in the claim and
qualification ledger. A recovered pre-crash four-file candidate ran the original
root, nine places and binary-selector production on Linux and Windows, while its
corpus route exposed a native-custody replay refusal. That candidate is historical:
the complete repair also retains cast/unary inputs and exact ancestor permission
in borrowed arrivals. Its fresh validation is recorded separately; the earlier
observations are not evidence for the later producer.
