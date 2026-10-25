# Lexical generic contract bounds

A shadowed const binder carries the seven-unit upper bound before exact 7 - value.

Parent: **FILESYSTEM-PLATFORM-SERVICES**, [TASKS.md](../../../../TASKS.md).
The Windows UTF-16 converter exposes a general contract-resolution defect:
an unrelated `data Units {}` redirects the requirement member's own const
`Units` operand, making identical public/implementation contracts disagree.
This row exercises the same resolution and refinement judgment with an ordinary
inline implementation, so it can execute without a Windows foreign provider.
It does not establish Windows API behavior or close the parent capability.

`Bounded::value` accepts input at most its invocation-local const `Units` and
guarantees that same upper bound on its result. `Selected` has its own exact
invocation binder and returns the input. Every place selects seven, carries the
result's upper bound, and consumes it in exact `7u64 - value`. The authored
result is zero; no caller equality guard establishes the subtraction premise.
The unrelated data declaration stays present in every place. The const-name
place spells the same seven through a typed named constant.

The witnesses are [local](local/main.omg), [field](field/main.omg),
[parameter](parameter/main.omg), [call result](call_result/main.omg),
[match join](match_join/main.omg), [const name](const_name/main.omg),
[a nested machine](other_state/main.omg), [another machine](other_machine/main.omg),
and [another target body](target_body/main.omg). Each has `invalid/main.omg`
beside it. Every twin changes only the public requirement's precondition from
`input <= Units` to `true`, so it no longer implies the implementation's
unchanged private bound. The selected input seven still satisfies that private
bound: rejection must judge the complete public contract, not just this call.
The bound, operation, public guarantee and global homonym stay unchanged. The
intended diagnostic names the unestablished `input <= Units` precondition;
every corresponding positive must accept to distinguish this from the old
homonym mismatch.

Every positive declares `Expected exit: 0` and all four hosted `ProgramEntry`
bindings. Byte-identical source/Build mirrors live under
`tests/omega/pass/facts/lexical_generic_bound_through_<place>_exit` and
`tests/omega/fail/facts/lexical_generic_bound_through_<place>_rejected`.

## Requirements and evidence

- **Establishment:** the checked implementation and exact const-seven
  application establish the public upper bound; a data type is not a numeric
  premise. Domain-bearing field/parameter/state destinations independently owe
  their installed upper bound.
- **Identity and lifetime:** requirement-local and implementation-local const
  symbols match by their invocation positions, never by spelling or the global
  data declaration. Immutable scalar snapshots keep their own static facts.
- **Composition:** call substitution, field storage, parameters, joins and state
  arrivals preserve the same bound. Subtraction consumes it directly. A stronger
  private precondition cannot replace the public contract.
- **Soundness boundaries:** local-first resolution retains exact owner and
  parameter identity; it grants no ownership, effects or new proof premise.
  Every generic argument and declared contract still undergoes ordinary checking.
- **Psi 05–07:** the ordinary selected member and scalar transports must lower
  to Terminal Psi with their static contracts erased only after checking and
  execute through the Terminal interpreter.
- **Omega 00–02:** the Build entry, selected target body and scalar call/result
  placements use the ordinary provider and calling-plan route.
- **Omega 03–06:** emitted calls, fields, joins and state transfers must preserve
  the observed scalar in normal register/frame homes.
- **Omega 07–09 and emission:** actual `omega run --both` must return native zero
  and Terminal zero at each place; declared target entries are not cross-host
  execution evidence.

The frozen pre-repair local witness refuses both contract directions:
`Selected::value` does not guarantee `result <= Units` and its matching
`input <= Units` is not implied. The minimal foreign converter probe independently
shows the same refusal after adding only the unrelated data declaration.
Run identities, producer profiles, streams and comparison evidence belong in
the qualification/claim records. All nine places must retain native zero and
Terminal zero on the measured host, with their exact reject controls. Windows
native execution requires a Windows runner, unavailable on this host. Nested
structural-signature captures and proposition-call target lookup have separate
owners and are not established by this const-binder repair. An unused malformed
foreign contract that names global data `Units` without any const binder still
accepts on both the baseline and repaired compiler: numeric-operand formation
is not established by this lexical-identity repair.
