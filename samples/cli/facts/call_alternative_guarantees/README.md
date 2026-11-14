# Retained alternatives through completed calls

The admitted Choice disjunction justifies select's complete entry contract, whose returned value <= 60 permits Counter construction and exact value + 195.

Parent: **BUMP-ALLOCATOR-CANARY**. Checked extent recomposition needs the same
rule: an empty extent or an adjacent nonempty extent is acceptable, without
knowing which alternative holds in a generic caller. Losing that complete
requirement also loses the returned extent's length guarantee and prevents
constructing the allocator's reset state.

Nearest row: [coupled_requires_construction](../coupled_requires_construction/README.md).
That row transports an arithmetic equation into construction. This row proves a
different requirement: retain an admitted **disjunction**, use it to admit a
call, and retain that call's normal guarantees. Neither individual alternative
is an available premise. The invalid input admits `left = 61, right = 200`,
which cannot satisfy the selected call's contract; construction cannot borrow
the resulting unestablished guarantee.

## Witnesses

| Place | Positive | Invalid control |
| --- | --- | --- |
| Local | [source](local/main.omg) | [source](invalid_local/main.omg) |
| Field | [source](field/main.omg) | [source](invalid_field/main.omg) |
| Parameter | [source](parameter/main.omg) | [source](invalid_parameter/main.omg) |
| Call result | [source](call_result/main.omg) | [source](invalid_call_result/main.omg) |
| Match join | [source](match_join/main.omg) | [source](invalid_match_join/main.omg) |
| Const name | [source](const_name/main.omg) | [source](invalid_const_name/main.omg) |
| Nested machine | [source](other_state/main.omg) | [source](invalid_other_state/main.omg) |
| Another machine | [source](other_machine/main.omg) | [source](invalid_other_machine/main.omg) |
| Selected implementation | [source](target_body/main.omg) | [source](invalid_target_body/main.omg) |

Every twin changes only Choice's left bound from 60 to 61. The selected callee
still requires `left <= 60 || right <= 60`. The expected refusal names either
that unsatisfied call or Counter construction, never parsing or a missing
provider. Byte-identical pins live under `tests/omega/{pass,fail}/facts/` with
the `call_alternative_guarantees_<place>_{exit,rejected}` names.

## Requirements and evidence

- **Establishment:** Choice's independently checked default domain supplies
  the whole disjunction. Select must establish its own `result <= 60` on both
  branches before a completed call can supply that fact to construction.
- **Identity and lifetime:** each fact names the exact input record or
  completed call result. No destination annotation supplies its own premise.
  These copy-only witnesses have no mutable alias or cleanup obligation.
- **Composition:** local, field, argument, return, join, constant, lexical
  nested machine, separate machine, and Build-selected implementation transport
  the same fact. The additional Counter bounds are not restated as caller
  guards or inserted as trust assumptions.
- **Soundness:** neither disjunct is inferred alone. Every weakened-domain
  control must reject. Integer formation and selected callable meaning remain
  independently checked.
- **Checked-to-Terminal:** both selector branches execute and return distinct
  values, 17 and 43. Bounds erase; the selected value does not.
- **Provider/ABI:** all four hosted ProgramEntry bindings are authored. The
  target place selects an ordinary named implementation. No foreign provider
  or interpreter-only shortcut implements the selector.
- **Native selection, frames, encoding:** `omega run --both` must observe
  native and Terminal exits 0/0 for every positive. The operation checks
  totals 212 and 238, so reversed or omitted selection traps. The field place
  also checks four nonzero neighboring canaries.

The Windows x86-64 host observed exits 0/0 at all nine positive places and the
intended refusal at all nine controls with the optimized allocator repair
producer (`535f6e00367e67f1d0e0fb755f4601a72e33eb69fa67cf6aa80381f665476c18`).
The corpus pins retain the same nine execution agreements and nine refusals.
The other three targets are declared, not executed on this host. The real
allocator customer also executes mutation, retirement, reset, and backing reuse;
genuine Vec growth remains the parent outcome.
