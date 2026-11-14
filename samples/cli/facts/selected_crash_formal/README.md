# Crash conditions through selected implementations

A call must obey the crash conditions its interface promises, even when the
chosen implementation happens to return normally. These examples check that
the compiler keeps that rule as the input passes through nine different places
in a program.

The `Reader` interface permits a `Trap` only when `value < Limit`. The caller
chooses an implementation of `Reader<Zero>` and requires a nonnegative input,
which rules out that crash. Each reject twin changes only `< Limit` to
`<= Limit`: zero is now an allowed input that may crash, so the compiler must
reject the caller's promise that it cannot crash. The chosen implementation
simply returns its input; that harmless body cannot weaken the interface's
published conditions. The final equality check observes the returned value;
it does not prove that the call is safe.

Owning tasks: **CRASH-CONTRACT**, checking the chosen interface's crash conditions
against the actual input; and **BUILD-OWNED-TARGET-SELECTION**, choosing a
platform implementation in Build configuration while preserving those conditions.

Fact: a generic selected Reader<Zero>::read call preserves the entry i32 >= 0
fact to discharge its authored value < Zero Trap route.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [local](local/main.omg) | [invalid](invalid_local/main.omg) |
| Field | [field](field/main.omg) | [invalid](invalid_field/main.omg) |
| Parameter | [parameter](parameter/main.omg) | [invalid](invalid_parameter/main.omg) |
| Call result | [call_result](call_result/main.omg) | [invalid](invalid_call_result/main.omg) |
| Match join | [match_join](match_join/main.omg) | [invalid](invalid_match_join/main.omg) |
| Const name | [const_name](const_name/main.omg) | [invalid](invalid_const_name/main.omg) |
| A nested machine | [other_state](other_state/main.omg) | [invalid](invalid_other_state/main.omg) |
| Another machine | [other_machine](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) |
| A selected target implementation | [target_body](target_body/main.omg) | [invalid](invalid_target_body/main.omg) |

The row is pinned under `tests/omega/{pass,fail}/facts/selected_crash_formal_through_*`.
All nine positives and the root bind all four hosted targets and expect exit 0.
Only the available Linux x86-64 host supplies native execution evidence here.

## Requirements established by this row

- **Establishment:** the caller's `value >= 0` requirement rules out `value < 0`.
  The compiler must use the checked integer comparison, not infer its meaning
  from an operator's spelling.
- **Identity and lifetime:** each crash condition uses the value actually passed
  at that call. [saved_actual](saved_actual/main.omg) copies the input before a
  write; its [twin](saved_actual/invalid/main.omg) passes the changed value and
  must reject. Reading a scalar reference means reading its actual backing
  storage.
- **Composition:** an identity call's `result == value` guarantee carries the
  input's nonnegative bound into a later call. Nested machines, match results,
  and forwarding helpers must preserve the same bound. The match example runs
  both branches with distinct returned values, so choosing the wrong branch
  makes its runtime check fail. A match keeps a range covering every arm's
  completed value. A copied argument keeps its bound when a later argument
  writes the source.
- **Soundness boundaries:** integer comparison rules apply to executable integer
  values; erased mathematical values and structural equality cannot stand in
  for them. A chosen implementation cannot narrow the public crash conditions.
- **Checked to Terminal Psi:** the compiler's portable representation retains
  the evaluated arguments, chosen declaration, and call contract. Its
  interpreter must produce the same result as the native program.
- **Provider selection, layout and calling convention:** package review matches
  `Zero` to Reader's declared `Limit` parameter. Ordinary integer and borrowed
  field arguments use their existing calling conventions.
- **Instructions and stack storage:** calls, saved scalar values, borrowed reads,
  and returns use the ordinary Linux x86-64 implementation.
- **Encoding and native execution:** the emitted executable must run and produce
  the expected result; successful checking or emission alone does not count.

[permitted_return](permitted_return/main.omg) allows a crash for its input, but
the chosen implementation returns normally. [permitted_crash](permitted_crash/main.omg)
uses the same public condition and actually crashes: the native
illegal-instruction signal and the interpreter's `Trap` must agree. Permission
to crash does not require a crash to happen.

[distinct_limits](probes/distinct_limits/main.omg) uses both `Reader<Zero>` and
`Reader<One>` in one program. Each call must keep its own limit even though
both name the same interface member. Its [invalid twin](probes/distinct_limits/invalid/main.omg)
weakens only the second caller's input requirement; allowing zero is insufficient
to rule out a crash below one. This is an additional check of call separation,
not another fact row.

[private_forwarding](probes/private_forwarding/main.omg) checks the same public
condition through a private helper after its implementation is chosen. Its
[invalid twin](probes/private_forwarding/invalid/main.omg) removes the caller's
nonnegative-input requirement. The harmless implementation must not make that
broader promise pass.

[argument_snapshot](probes/argument_snapshot/main.omg) copies the first argument
before evaluating a second argument that clears its source. The original value
and its bound must survive. Its [twin](probes/argument_snapshot/invalid/main.omg)
widens `safe`'s input contract to permit -1; clearing the source later cannot
justify the already copied argument. [match_record](probes/match_record/main.omg) checks the same
nonnegative bound on a field selected by two match arms. Its
[twin](probes/match_record/invalid/main.omg) permits a crash at zero.

## Baseline and remaining scope

Fresh measurements at `feef96dc297dbd12c2185973f1a216be927c060f` pass all nine
places and their reject twins on Linux x86-64. The focused corpus run checks
41 programs: 18 valid programs pass and 23 controls reach their intended
rejections. All 18 native/Terminal observations match their expected results: 16 exit
0 and two deliberately trap. The ordinary sample path passes all 32 projects
(18 executions and 14 intended rejections). The optimized compiler and
raw results are retained in
[run 41542440751](https://github.com/CathedralOS/Omega/actions/runs/41542440751).
These are Linux execution results; the other hosted targets remain unexecuted.

At `d6cf554fd1cbf20551983d29bae26f5cf3b81664`, the local, nested call-result
and saved-actual positives wrongly reject with an uncovered Trap call route.
The version that permits the crash passes source checking, but package review
rejects the named constant argument instead of treating it as its checked value.

This row does not close **CRASH-CONTRACT**. The task still includes selected
implementations with record arguments, other compiler-provided operations,
opaque calls and content ownership, and sequences of calls with different
possible outcomes. An exploratory implementation whose crash condition depends
on an unknown constant still rejects its own crash site; this row does not
claim to fix it. Run records retain the actual commands, source hashes, and
engine outcomes. Folder presence alone is not execution evidence.

## Build-owned target-body migration

The `target_body` place calls the public `SelectedForward::forward` interface.
The caller still chooses the Reader implementation with `safe<Provider<Zero>>`.
Four explicit Build rows choose the forwarding implementation for each hosted
platform. Every body passes the same input to the caller's chosen Reader.
The interface keeps `crashes Trap value < Zero` and declares that it uses no
services. The public helper names the interface it may call with
`invokes SelectedForward`. Changing only Reader's `< Limit` to `<= Limit` must
still reject, even though the chosen implementation returns normally.

This uses the generic interface support already landed with
`selected_value_application`. The compiler now reads named integer limits from
their completed, checked values and keeps the full selected signature for each
call. Private forwarding helpers use that public signature too. When lowering
the call, the retained call record connects the public requirement to the
implementation that will execute. These repairs enforce the existing crash
rule; they do not change which crashes an interface permits.
[Provider selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection),
[explicit evidence selection](../../../../wiki/spec/language/conformances.md#declaration-and-selection),
and [boundary crash contracts](../../../../wiki/spec/terminal-psi/boundary_calls.md#boundary-crash-contracts)
govern the preserved selection and call obligations.
[Generic requirement evidence](../../../../wiki/spec/language/generics.md#generic-machine-requirements)
permits the member's caller-supplied implementation binder; Build selects its
platform implementation without filling that binder. Other target-prefix
migration and the remaining CRASH-CONTRACT acceptance remain open.
