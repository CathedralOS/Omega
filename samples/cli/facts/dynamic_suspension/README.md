# Suspension-capable dynamic calls

Parent: `RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY` in [TASKS.md](../../../../TASKS.md).
Requirement: support suspending requirements in dynamic call planning while
preserving the separate selector, ordinary operand lanes, public result contract,
and checked suspension frontier.

Fact: a suspension-capable dyn call preserves its selector, explicit operand, and public result bound when subtracting 100u64.

`DeferredValue::get` requires input 100 and publishes result 100. Its selected
member calls a suspension-capable boundary requirement whose selected provider
completes inline. This exercises a possible suspension crossing and continuation;
it does not demonstrate actual scheduler parking. The consuming subtraction uses
the public guarantee directly. Every executable binds all four hosted targets.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [source](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call result | [source](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match join | [source](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| constant | [source](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| nested machine | [source](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| other machine | [source](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| target body | [source](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Twins change only the input to the dynamic requirement from 100 to 99. They
reject for the unproved requirement, independently of suspension acknowledgement.
The match join executes two different implementation tables and both branches;
the first result remains live across the second suspension-capable call.

Run each positive with `omega run --both <place>/main.omg` and each twin with
`omega --check <place>/invalid/main.omg`.

## Evidence and remaining acceptance

Fresh Windows baseline producer `cbee4617745b4191afa242f784aa8c7a72a9674d`,
opt-level 1, incorrectly reports that the dynamic call has neither suspension nor
blocking while its selected member calls a possibly-suspending requirement.
The original result and exact producer/source hashes are retained under ignored
`build/slot4-evidence/suspension-baseline-row-root.json`.

The requirement establishes the bound; source values and captured operands own
its identity. Calls and joins must preserve that guarantee, and invalid inputs
must still reject. Lowering must retain the source occurrence and carry demands
on `CallDynamic`, with the selector separate from data arguments. Provider
settlement and ABI planning must preserve those lanes; instruction selection,
frame realization, image relocations, and native execution agree with Terminal
Psi on the available Windows x86-64 host. The original dynamic suspension
refusal paths are removed. Ordinary call planning retains the same source
occurrence and frontier for scalar and Unit calls; the Terminal target names the
public interface/slot and captures its selector separately from the data lane.
Wire tag 5 records that target; retired tags 3 and 4 still reject.

The focused corpus records 15 source positives and 18 intended rejects: nine
places and twins, six composition positives, and nine additional controls.
The 15 positives execute with native and Terminal exit 0. Five suspension-codec
tests pass, including target roundtrip and rejection of retired target tags.
These are local observations; full parent acceptance remains open.

The additional [Unit call](unit_call/main.omg) exercises a statement call's
data-only contract telescope, with its own [invalid input](unit_call/invalid/main.omg).
[Retained selection](retained_selector/main.omg) uses the same selector for two
possibly-suspending calls. Missing acknowledgement and nested-suspension controls
are pinned under `tests/omega/fail/traits/dyn_suspending_*`.

[Live record](retained_record/main.omg) keeps a structural local across the
computed call. [Retained envelope](retained_envelope/main.omg) copies a pure
selector into a field, rebinds the original to a waiting implementation, and
uses both exact envelopes. [Parameter envelope](parameter_envelope/main.omg)
forwards each selection through two private machines, including a nested machine.
[Returned envelope](returned_envelope/main.omg) transports each selection through
a private identity result. Their marker controls reject both a missing waiting
acknowledgement and a redundant pure acknowledgement. The source-value graph
uses captured storage versions and call operands; an unknown or public/opaque
result keeps the public requirement bound. Dynamic selection narrows only the
suspension/blocking summary here; other envelope categories remain parent work.

Scheduler parking, borrowed-parameter preservation across an inner suspension,
complete operational-envelope refinement, crash and recursion contracts,
package qualification metadata, product-source parity, and native execution on
the other hosts remain parent acceptance. A borrowed Unit-call experiment and
its refusal are retained under ignored `build/slot4-evidence/`; the compiler's
retained-loan checks remain enforced.
