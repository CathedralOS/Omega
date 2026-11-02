# Closed record results in implementation arguments

Owner: **STATIC-MACHINE-MIGRATION** in [TASKS.md](../../../../../../TASKS.md).

Selected<Configuration { minimum: Base + 4 }> at Base = 3 supplies at least seven before exact value - 7.

This production stays in the [existing record-const fact](../../README.md).
An ordinary checked `configuration()` call produces the same `Configuration`
value as the direct constructor. Both the required trait argument and the
selected named implementation consume that complete closed result. An evaluated
field list cannot replace the result's nominal carrier or original call custody.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [source](main.omg) | [source](invalid/main.omg) |
| field | [source](../../field/closed_call/main.omg) | [source](../../field/closed_call/invalid/main.omg) |
| parameter | [source](../../parameter/closed_call/main.omg) | [source](../../parameter/closed_call/invalid/main.omg) |
| call result | [source](../../call_result/closed_call/main.omg) | [source](../../call_result/closed_call/invalid/main.omg) |
| match join | [source](../../match_join/closed_call/main.omg) | [source](../../match_join/closed_call/invalid/main.omg) |
| const name | [source](../../const_name/closed_call/main.omg) | [source](../../const_name/closed_call/invalid/main.omg) |
| nested machine | [source](../../other_state/closed_call/main.omg) | [source](../../other_state/closed_call/invalid/main.omg) |
| other machine | [source](../../other_machine/closed_call/main.omg) | [source](../../other_machine/closed_call/invalid/main.omg) |
| target body | [source](../../target_body/closed_call/main.omg) | [source](../../target_body/closed_call/invalid/main.omg) |

Each twin changes only the selected implementation's argument to the record
with minimum `3 + 3`. The requirement still receives the call's minimum seven,
and the consuming operation stays `value - 7`. The intended refusal names the
incompatible complete `Reading` implementation. The field place checks canaries
641 and 919 around the value. The match place executes both arms and checks
distinct tags 17 and 29. The target place retains Build-selected ordinary named
implementations and its architecture tags.

At fixed base `e9f492c5ccc6f84731ce0e65eb9a9d4f56fdb067`, optimized producer
SHA-256 `2304d302c3cf40ec53cff6d8f6cf76e964651bdeb43a33c2aa70b70ecdfab3f6`
checks the existing open-expression local and a direct closed constructor.
The frozen local call source instead rejects with “application probe destination
requires an unconstrained exact builtin integer or Boolean carrier.” Its source
SHA-256 is `85f7d8c17c6877d3e46511462326ac46d131f5a2f28a36f5df018bdfb6705a0a`.
The ordinary outer command is `omega run --both main.omg` at each positive.

The accepted [generic application rules](../../../../../../wiki/spec/language/generics.md#closed-applications-and-canonical-identity)
and [canonical static identities](../../../../../../wiki/spec/language/evaluation.md#canonical-static-identities)
allow records, arrays and cases while excluding float, reference, slice, dynamic
and opaque indices. The repair uses the checked initializer invocation and its
independent declaration-backed result encoding, then replays that encoding
against the exact receiving const carrier before publishing the argument.

- Establishment: the helper's checked construction and completed invocation
  produce the canonical value; the requirement supplies no formation premise.
- Identity and lifetime: exact nominal result and original lexical selections
  survive evaluation. These copyable, borrow-free indices carry no storage loan.
- Composition: the same seven and exact subtraction cross all nine places.
  The selected implementation must satisfy the complete public trait argument.
- Soundness: every authored call retains its admission, concrete premises and
  failure discharge. Canonical carrier replay independently checks eligibility
  and data predicates; equal bytes supply no nominal authority.
- Psi 05–07: checked specialization erases the record const argument while
  retaining the selected member and its public result guarantee in Terminal Psi.
- Omega 00–02: ordinary scalar/record geometry and calling plans realize each
  transport, including neighboring field slots and a returned match record.
- Omega 03–06: the selected result, subtraction, canaries and branch tags expose
  wrong instructions, register homes, field offsets or call results.
- Omega 07–09: acceptance requires actual native and Terminal exit-zero agreement
  for every positive, with the intended static refusal for every twin.

On Linux x86-64, the focused sample gate reports 12/12 positive programs with
native and Terminal exit-zero agreement and 14/14 intended rejections. This
includes all nine places, the [nested return](controls/nested_return/main.omg),
and the [data argument](controls/data_argument/main.omg). The latter's
[reject twin](controls/data_argument/invalid/main.omg) returns `Envelope` with
minimum six into the expected minimum-seven carrier; equal layouts do not erase
the complete argument identity. Additional controls refuse an
[unsatisfied call premise](controls/missing_requirement/main.omg), a
[same-shaped foreign carrier](controls/wrong_carrier/main.omg), and a
[noncanonical float field](controls/noncanonical_field/main.omg).

The repair removes the scalar-only result gate from static-call evaluation.
Checked initializer admission and interpretation produce aggregate results;
materialized values retain a separate, nonexecuting edge to their original
computation through copying and both source and typed custody. Ready initializer
layers close their static prerequisites before specialization. Their exact
constant declaration supplies the lexical owner. Generated data instances replay
completed canonical arguments under their independently selected nominal carriers,
rather than comparing copied expression handles.

The reproducible command is:

```sh
python3 tools/sample_native_gate.py --cli target/debug/omega --facts \
  --filter closed_call --jobs 2 --timeout 90
```

The corresponding `open_record_const_closed_call_*` corpus fixtures retain each
program and its authored oracle. All four hosted ProgramEntry bindings are
authored. Other-host execution, provider-dependent deferred aggregate calls,
broader argument custody, the Omega-written producer and the remaining parent
migration remain open acceptance.

The [equivalent literal transport](controls/data_argument/equivalent_literal/main.omg)
also passes the function-produced `Envelope` to a parameter indexed by the direct
`Configuration { minimum: 7 }` literal. Both forms publish the same resolved
carrier identity after exact const-argument validation. The data conflict check
uses the complete semantic application identity, including unused static indices,
instead of comparing generated declaration symbols or diagnostic spellings.

The focused official corpus run records 12 checked positives and 14 intended
rejections, with no wrong-reason results or oracle failures. Its Linux x86-64
execution leg records all 12 positives with native/Terminal agreement. Eight
existing scalar and record controls retain their expected semantic outcomes
(five checked, three rejected). Their older diagnostic and timing goldens report
movement; the diagnostic changes also reproduce with the fixed base producer.
Only the new fixtures receive new corpus rows, stamped with the actual fixed
base plus working-tree provenance.
