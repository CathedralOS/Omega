# Qualified payloads through selected owned replacement

An owned payload in PositiveSmall preserves every active case payload when
replacing one of 4096 frames.

Parent: **FILESYSTEM-PLATFORM-SERVICES**, `TASKS.md`. This row exercises the
general source-to-execution prerequisite exposed by the Windows removal stack:
a captured single-element replacement must preserve the conditional field
contracts of an otherwise valid fixed array. It does not establish Windows API
behavior or close the parent capability.

Each place carries the same `0 < payload < 8` qualification into the same
runtime-indexed `Frame::Ready` store, forwards the inventory to another state,
and subtracts one from the selected payload. The authored result is 2; an empty
or wrong replacement fails the runtime oracle. The array extent, domain and
operation do not change between places. Only the payload's transport changes.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [program](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [program](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [program](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call result | [program](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match join | [program](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const name | [program](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| another state | [program](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| another machine | [program](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| another target body | [program](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Every positive binds all four hosted `ProgramEntry` targets and declares exit
0. Match-join execution exercises both arms. Each twin changes only its source
payload from 3 to 0 (one match arm in the join), breaking the same qualification
without adding or removing a caller guard. The field control's early diagnostic
reports the equivalent declared interval `1..=7`; the other controls name
`PositiveSmall` directly.

The programs are mirrored under
`tests/omega/{pass,fail}/facts/indexed_owned_case_replacement_through_*`;
positive fixtures declare native exit 0. Run a place with:

```sh
omega run --both samples/cli/facts/indexed_owned_case_replacement/local/main.omg
python3 tools/sample_native_gate.py --facts --filter cli/facts/indexed_owned_case_replacement
```

## Requirements and evidence

- Establishment: literals, constructor fields, qualified parameters/results and
  constant admission establish `PositiveSmall` before the store. Zero array
  formation selects `Empty`, not an invented payload qualification.
- Identity and lifetime: the store uses the captured destination and completed
  RHS. Preserved memberships describe exact candidate leaves, not a fanout of
  the replacement's scalar value, tag, issuer or equality.
- Composition: the successor's full array-field contract must follow from
  independently justified unchanged elements and the installed value. The
  consuming subtraction uses the carried payload bound directly.
- Soundness: invalid payload formation rejects. Qualification preservation
  grants neither a loan nor copy/disposal authority. Routed issuer, reference
  retention and mutation controls supplement this predicate-only row.
- Psi 05–07: case construction, selected store, state arrival, payload selection
  and exact subtraction must survive Terminal Psi lowering and interpretation.
- Omega 00–02: the case/array layout and explicit calls/state parameters must
  realize through the ordinary provider, geometry and calling-plan route.
- Omega 03–06: the 4096-element activation and dynamic address calculation must
  use disjoint homes and valid target instructions, not a source-shape shortcut.
- Omega 07–09: the emitted native image must execute and agree with Terminal
  Psi. Target bindings alone are not evidence of execution on another OS.

Baseline producer `3067bce0724a` rejects all nine positives with 4096 lost
conditional field obligations each. Producer `55de0870d9bc` executes every
place with native exit 0 and Terminal exit 0 on macOS arm64; all nine twins
reject for their intended qualification. The nine pinned positives also agree
in the corpus's shared-artifact native/Terminal route. Run records retain
producer, source, oracle and artifact identities.

The supplemental routed-copy, guard-loan, qualified readonly-array and
consumed-linear witnesses also execute with exit 0 in both engines. Their
controls retain issuer, coverage, alias, handback and consumption obligations.
This establishes whole-value replacement and those compositions, not arbitrary
temporarily-invalid primitive stores into qualified storage. That separate
lowering remains fail-closed. Windows native filesystem execution remains a
separate environment dependency; neither this row nor foreign publication
establishes it.
