# Selected dynamic service reach

Parent: `RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY` in [TASKS.md](../../../../TASKS.md).
Requirement: complete the operational envelope refinement for dynamic
requirements beyond suspension and blocking. A coercion retains the selected
conformance's exact envelope ([dynamic dispatch](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md)).
This row covers the service-reach axis.

Fact: A dyn call reaches only its selected conformance's services, so a machine publishing an empty reach row may dispatch through a non-reaching selection and subtract 100u64 from its result.

`Source::get` publishes `reaches Journal`. `JournaledSource` calls the
`Journal` boundary and returns 101; `QuietSource` reaches nothing and returns
100. Every positive dispatches through a `QuietSource` selection inside
`observe`, which publishes an empty reach row (`reaches` with no services).
The caller subtracts 100u64 using the requirement's `result >= 100u64`
guarantee and traps unless the difference is exactly the selected
implementation's value. A dispatch through the wrong table entry traps.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [invalid](invalid_local/main.omg) |
| field | [source](field/main.omg) | [invalid](invalid_field/main.omg) |
| parameter | [source](parameter/main.omg) | [invalid](invalid_parameter/main.omg) |
| call result | [source](call_result/main.omg) | [invalid](invalid_call_result/main.omg) |
| match join | [source](match_join/main.omg) | [invalid](invalid_match_join/main.omg) |
| constant | [source](const_name/main.omg) | [invalid](invalid_const_name/main.omg) |
| nested machine | [source](other_state/main.omg) | [invalid](invalid_other_state/main.omg) |
| other machine | [source](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) |
| target implementation | [source](target_body/main.omg) | [invalid](invalid_target_body/main.omg) |

Each twin changes only the selection to `JournaledSource`. It rejects because
`observe` (or the selected `X86Observe` implementation) now reaches the
undeclared `Journal` service. The match join selects `QuietSource` or
`SteadySource` (result 102) and runs both arms; its twin replaces the second arm
with `JournaledSource`, so the join keeps the union of both selections. The
target implementation place selects `X86Observe` or `ArmObserve` per target
through Build.

Run each positive with `omega run --both <place>/main.omg` and each twin with
`omega --check <place>/invalid/main.omg`.

This row follows the ratified
[known dynamic contract narrowing](../../../../wiki/spec/language/effects.md#known-dynamic-selections):
check-time selections retain their available checked contracts; unknown inputs
and providers selected later retain their governing ceilings. A smaller private
body does not rewrite an explicitly published callable contract.

Nearest row: [dynamic suspension](../dynamic_suspension/README.md) narrows only
the suspension and blocking summary. Its twins change the requirement input and
reject a contract; these twins change the selection and reject a reach row.

## Requirements

- Establishment: the coercion `Impl as dyn Source` establishes the selection.
  An unknown selector, such as an unannotated public `dyn` parameter, keeps the
  requirement's published `reaches Journal` row.
- Identity and lifetime: the operational fixed point resolves the selector
  through locals, fields, constants, private call results, joins, nested
  machine arrivals and private machine parameters. Each known selector context
  is summarized as its own instance of the machine body; the declaration
  context still publishes the conservative row.
- Composition: a joined selection contributes the union of its alternatives'
  reach; a private call under a known selector contributes the reach of that
  context's summary.
- Soundness boundaries: the published ceiling of a public or annotated machine
  is unchanged; only the inferred body row narrows.
- Lowering and native execution: the selector stays in its own lane; the
  positives execute the selected table entry in both Terminal Psi and native
  execution on Linux x86-64.

Open: the other envelope axes the specification lists (direct synchronous
invocation, mutation, capability demands, failure, termination, resource
ceilings and guarded crash routes) still use the requirement row. Windows,
macOS and Linux arm64 execution of this row is unverified; this row ran on
Linux x86-64 only.
