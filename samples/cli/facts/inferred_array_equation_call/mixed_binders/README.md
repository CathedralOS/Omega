# Mixed selections through an array equation

Owning task: **STRUCTURAL-GENERIC-MATCHING** in [TASKS.md](../../../../../TASKS.md).
This production establishes closed structural completion with interleaved explicit
machine and value binders plus the separately stored named-conformance suffix.
The public contract remains [generic application and structural inference](../../../../../wiki/spec/language/generics.md).

Fact: an array equation infers Capacity=2 while exact explicit callback, value,
and evidence selections retain their tuple positions and each observes 7.
Every place copies that sentence. A consuming call reads Capacity and the three
selected observations from its returned record. The rejection twin changes only
the call's explicit Capacity to 3; its array, other selections and operation stay
unchanged. No caller guard supplies the inferred capacity.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [positive](local/main.omg) | [conflicting capacity](local/invalid/main.omg) |
| field | [positive](field/main.omg) | [conflicting capacity](field/invalid/main.omg) |
| parameter | [positive](parameter/main.omg) | [conflicting capacity](parameter/invalid/main.omg) |
| call_result | [positive](call_result/main.omg) | [conflicting capacity](call_result/invalid/main.omg) |
| match_join | [positive](match_join/main.omg) | [conflicting capacity](match_join/invalid/main.omg) |
| const_name | [positive](const_name/main.omg) | [conflicting capacity](const_name/invalid/main.omg) |
| other_state | [positive](other_state/main.omg) | [conflicting capacity](other_state/invalid/main.omg) |
| other_machine | [positive](other_machine/main.omg) | [conflicting capacity](other_machine/invalid/main.omg) |
| target_body | [positive](target_body/main.omg) | [conflicting capacity](target_body/invalid/main.omg) |

The [root](main.omg) and its [twin](invalid/main.omg) repeat the same operation.
Each positive binds ProgramEntry for Windows x86-64, Linux x86-64, macOS ARM64
and Linux ARM64 and declares expected exit 0. The nine places are also pinned as
`inferred_array_equation_call_mixed_binders_through_<place>_{exit,rejected}`
under `tests/omega/{pass,fail}/facts/`.

## Additional controls of the same substitution

[Value-index composition](value_index/main.omg) supplies Count=7 while a global
Count=99 exists. Its formal and returned `Selection<Count>` types must use the
selected binder; an outer consumer receives `Selection<7>`. Its
[Count=8 twin](value_index/invalid/main.omg) refuses the conflicting complete
application tuple. The current diagnostic names inability to derive the complete
type/const/machine/conformance tuple; it does not identify the conflicting index.

The callback [formal](callback_formal_shadow/main.omg),
[result](callback_result_shadow/main.omg),
[extent expression](callback_expression_shadow/main.omg) and
[static-call argument](callback_static_argument_shadow/main.omg) controls prevent
a selected machine binder from becoming a same-named global type or integer.
The evidence [formal](evidence_formal_shadow/main.omg),
[equation element](evidence_equation_shadow/main.omg),
[bare equation](evidence_name_shadow/main.omg), and
[domain](evidence_domain_shadow/main.omg) controls retain the separate evidence
binder's lexical identity. A [value-as-type](value_type_shadow/main.omg) control
retains its kind instead of selecting a global data declaration.

Normal admission still refuses a [wrong value carrier](value_carrier_mismatch/main.omg),
[wrong callback signature](callback_contract_mismatch/main.omg), and
[wrong conformance application](evidence_application_mismatch/main.omg).
Their exact diagnostic assertions are in sample.json and matching pinned fixtures.

## Requirements and evidence

- **Establishment:** the explicit array type and its structural equation determine
  Capacity=2. Callback, Count and Evidence remain authored selections; normal
  complete-tuple admission checks their contracts, carrier and trait application.
- **Identity and lifetime:** completion retains declaration-order slots and
  original supplied operands. Static value indices remain in formal/result type
  substitution. Excluded machine/evidence names cannot become globals. Copyable
  arrays and records introduce no new mutable-loan or cleanup authority.
- **Composition:** locals, fields, parameters, returned arrays, joins, constants,
  lexical nested machines, other machines and target bodies feed the same call.
  The dependent-result control composes its closed result with another consumer.
- **Soundness boundaries:** explicit Capacity=3 conflicts with the fixed array;
  carrier, callback, evidence and lexical-kind failures retain distinct refusals.
  No package review or other external authority is granted by these programs.
- **Psi 05-07:** complete selected arguments pass through ordinary checking and
  lowering. Actual Terminal Psi execution observes the same four values as native
  execution; no alternate source evaluator substitutes for that observation.
- **Omega 00-02:** ordinary entry binding, array/record layout and aggregate-call
  plans carry the selected values through the returned Observation record.
- **Omega 03-06:** ordinary argument/return homes, activation storage and selected
  instructions preserve the values consumed by the runtime oracle.
- **Omega 07-09 and emission:** Windows PE products execute with exit 0 and agree
  with Terminal Psi. Four target bindings and checked target bodies do not
  establish execution on unavailable native hosts.

The recovered source initially refused on exact current-main producer
`7df8b4d92070d83d357849fb2eb77e05c61f36a4` with
`machine equation completion requires type and const binders` in the root and
all nine places. A preliminary Windows candidate produced 11 exploratory
native/Terminal agreements, but the bare evidence-name control then exposed a
wrong acceptance in that candidate. Its cached equation classifier had not
recognized the separately stored lexical evidence binder. The final guard
rechecks that name within the selected declaration's complete binder roster.

Run records in the owning claim, qualification and submitted change retain exact
optimized producer revisions/digests, witness hashes, Windows target and both
engine observations. Earlier syntax-authoring failures and stale diagnostic
fragments are excluded from acceptance. These records do not establish execution
on unavailable native hosts or close the parent task.

## Remaining acceptance

The whole task remains open: open schemas, retained extensions, late-selected
receivers, operator/conformance supply, named-lifetime identity, finite type-equality
alternatives, and remaining selected computed calls/structured const composition.
This row covers closed mixed selections, not arbitrary callback/evidence-dependent
computed type expressions or runtime value-index identity. Named Std-backed
customers retain their own package review and execution requirements. Linux,
macOS and Linux ARM64 native observations for this production remain unmeasured.
