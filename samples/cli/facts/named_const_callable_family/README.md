# Named const callable family

A named constant supplies the open-Const length of a callable family, and the selected callback measures that exact length.

Owning task: **OPERATOR-MACHINE-SUPPLY** (`TASKS.md`). The exact requirement this row establishes is the item's former bullets "Preserve exact cross-module const-alias selection in callable/evidence arguments" (its callable half) and "Admit open-Const callable families through their declaration-owned scopes". The family `apply<const Count: u64, machine Callback>` requires `Callback(value: &[u8; Count]) -> u64`; the call `apply<FOUR, measure>(&four)` names a constant declaration, never a literal, and the callback `measure<const Length: u64>` returns the `Length` it was specialized at. The row closes those two bullets for integer-valued const declarations. It does not close the evidence-argument half of the first bullet or the rest of OPERATOR-MACHINE-SUPPLY ([What remains open](#what-remains-open)).

Every place copies that sentence and the same operation. `FOUR` (4) and `TWO` (2) select two instances of one callback, so `first == 4` and `second == 2` hold only when each constant's own value reaches its callback. `hold(first, second)` carries `requires first == 4` and `requires second == 2`: the caller reads the result the family records, and `main` reaches `done()` (exit 0) only through it. A runtime conjunct (`first == 4 && second == 2`) follows `hold` and cannot establish its premises, which `hold` consumes first. The reject twin changes only the selected declaration's value (`const FOUR: u64 = 5`, or the same change in the foreign module), so the explicit `Count` and the operand's length disagree. Only the transport of the two array operands changes.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both arrays are local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | both arrays are fields of one record |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | each array arrives as an explicit parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | each array is a call result |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | each array is chosen by a `match` whose arms both yield an array |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | each array is copied from a named constant |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives both arrays |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine supplies both arrays |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Build selects one implementation per hosted target; each performs both calls |

Three more productions of the same layer, each with the same sentence, operation and oracle:

| Production | Positive | Reject twin | What changes |
| --- | --- | --- | --- |
| cross-module alias chain | [`cross_module_alias/main.omg`](cross_module_alias/main.omg) | [`cross_module_alias/invalid/main.omg`](cross_module_alias/invalid/main.omg) | `AGAIN` → `ALIAS` → `foreign::SELECTED`; the twin edits only [`foreign.omg`](cross_module_alias/invalid/foreign.omg) (`SELECTED = 5`) |
| forwarded family | [`forwarded_family/main.omg`](forwarded_family/main.omg) | [`forwarded_family/invalid/main.omg`](forwarded_family/invalid/main.omg) | a second family `apply_again<const Total, machine Inner>` forwards its open Const and callback to the first |
| implementation scope | [`implementation_scope/main.omg`](implementation_scope/main.omg) | [`implementation_scope/invalid/main.omg`](implementation_scope/invalid/main.omg) | the family is applied inside a member of a named implementation |

The row has no root `main.omg`; `local` is its canonical witness.

## Defect and repair

At base `949694bc033` every positive above was refused, while the same programs with the literals `4` and `2` in place of `FOUR` and `TWO` ran. The first diagnostic in all twelve:

```
machine `measure$specialized$…$0` state `entry` parameter `value` uses unresolved fixed-array length `Length`; a symbolic array length must name an in-scope `const` parameter
```

A callback generic over a type (`accepts<'borrow, Kind>`) failed the same way with ``unresolved fixed-array length `FOUR` `` and ``argument `value` … expects `&[u8; FOUR]` ``. `forwarded_family` also reported ``machine argument … does not refine `Callback`: parameter 0 expects `&[u8; 4]`, got `&[u8; FOUR]` ``. The cause did not depend on aliases or modules: a same-module constant, an alias chain and a path into another module failed alike, and a plain generic (`takes<FOUR>(&values)`) or a callback with a fixed length (`measure_four(value: &[u8; 4])`) ran.

`CallableApplication` (`typed-trees`, `callable_applications/payloads.rs`) substitutes a callable's binders into the signature of a required callback. An array length bound to a const declaration kept the declaration's name as `FixedArrayLength::ConstParameter { symbol }`, so the applied signature named a binder that no scope owns. The callback's own binder, inferred from that signature, could never close. The repair reads the declaration's evaluated integer through `integer_const_declaration_extent`, the decimal value `static_const_argument_spelling` already derives for the same call's tuple, and records `Literal(value)` for the length. The argument's constant origins stay on the applied array reference, so the declaration selection remains attached. A pending, structured or non-integer declaration, and a binder forwarded from a caller, take the previous path unchanged.

## Execution

Every positive binds `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg` and declares `Expected exit: 0`. `omega run --both` reports Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/named_const_callable_family/<place>/main.omg < /dev/null
omega run --both samples/cli/facts/named_const_callable_family/<place>/invalid/main.omg < /dev/null
python tools/sample_native_gate.py --cli <omega> --facts --filter cli/facts/named_const_callable_family
python tools/corpus_gate.py --both --filter named_const_callable_family
```

Measured on Windows x86-64 (32 logical CPUs) with the optimized producer built from `949694bc033` plus this change (`omega --build-profile` reports `opt_level: 1`):

- All twelve positives report `native exit 0` and `interp exit: 0` through `omega run --both`.
- All twelve twins exit at compile time with ``generic machine `apply` (`apply_again` in `forwarded_family`) has a static selection, but its complete … specialization tuple cannot be derived …: Const slot `Count` (`Total`) conflicts: 5 … versus 4``, from `main` (`main::check` for the nested machine, `observe` for another machine, `X86Observe::observe` for the target body, `Observer::run` for the implementation scope, `observe_four` for the parameter place).
- `tools/sample_native_gate.py --facts --filter cli/facts/named_const_callable_family` reports 12/12 samples meeting their documented oracle on `windows_x86_64` and 12/12 intended-invalid controls matching their authored rejection.
- The unmodified base producer reproduces the defect on all twelve positives; its twelve twins reject with the same diagnostic as the repaired producer (see [Reject twins](#reject-twins)).

### The oracle discriminates

The program reaches `done()` only through `hold(first, second)`, whose two `requires` clauses are discharged from the results the two calls record, and then through `first == 4 && second == 2`. One callback instance is specialized twice, at `Length = 4` and at `Length = 2`, so a lowering that keyed specialization by the constant's name or reused one instance would return the wrong length and take `bad()` (`crash Trap`). No caller guard establishes the premises before `hold`.

## Reject twins

Each place and production has an `invalid/` twin whose `sample.json` carries `"role": "intended-invalid"` and `rejection_contains`: `["Const slot", "named(integer-const(5))", "named(integer-const(4))"]`. In each twin one line differs from its positive (verified by `diff`): `const FOUR: u64 = 4;` becomes `const FOUR: u64 = 5;`, or, for `cross_module_alias`, `foreign.omg`'s `public const SELECTED: u64 = 4;` becomes `5`. The constant's value is the only input that changes, so the explicit `Count` (5) and the length inferred from the operand (4) disagree and the call has no specialization tuple.

The base producer rejects these twins with the same diagnostic: the explicit constant is evaluated by the tuple derivation both before and after the repair. They pin that the constant's value, not its name, selects the family's instance; the repaired layer is pinned by the twelve positives, which were red at base.

## Pinned fixtures

The row is pinned by 24 fixtures: `tests/omega/pass/facts/named_const_callable_family_through_<place>_exit` and `tests/omega/fail/facts/named_const_callable_family_through_<place>_rejected` for the nine places, and `tests/omega/{pass,fail}/facts/named_const_callable_family_{cross_module_alias,forwarded_family,implementation_scope}_{exit,rejected}` for the three productions. Each pass fixture is a byte-identical copy of its positive (plus `foreign.omg` for the alias chain), a `build.omg` binding the four hosted targets and `expected_native_exit.txt` of `0`. Each fail fixture is a byte-identical copy of its twin with an `expected.txt` carrying the three fragments above.

The corpus gate over the 24 fixtures (`--filter named_const_callable_family`, jobs at the runner default) reports: check leg 12 `checked` and 12 `rejected`, 0 rejected without their expected fragment, 0 fail fixtures that did not reject; native leg 12 `built` with exit 0; both leg 12 `built`, executed on both engines with exit 0.

## Requirements and Evidence

1. **Establishment:** the constant declaration establishes the evaluated integer (4 or 2); the family's applied requirement records that exact length, and the callback's `Length` closes at it. `hold` consumes the recorded results; nothing re-establishes them after the calls.
2. **Identity and lifetime:** `FOUR`, `TWO` and the foreign declaration are exact declarations selected through the alias chain; the applied array type keeps the argument's constant origins. The two instances `measure[Length = 4]` and `measure[Length = 2]` stay distinct. Neither array is written between its call and the read.
3. **Composition:** the recorded results compose into `hold(first, second)` at all nine places and three productions. Each positive and its twin differ in one line, the selected constant's own value; no twin adds a caller guard.
4. **Soundness boundaries:** a constant whose value disagrees with the operand's length yields no specialization tuple, so no program with a wrong constant compiles. A pending, structured or negative declaration never becomes an extent.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** each callback application lowers to an ordinary call of a specialized instance; the lengths are erased to scalar results. The Terminal Psi interpreter executes all twelve positives and agrees with native execution.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** every place and production binds `ProgramEntry` for the four hosted targets; the corpus check leg realizes the host target from those bindings. `target_body` selects distinct implementations per target through Build rows. The arrays are passed as references and the results as `u64` registers.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** the executed `windows_x86_64` images assign register homes and disjoint frame slots for two arrays and two scalar results around four calls; a wrong length or instance takes the `bad()` trap.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** the emitted PE images execute and agree with Terminal Psi (`omega run --both`); the twins reject before emission.

## What remains open

- The evidence-argument half of the first bullet is pinned outside this row by [`cross_module_const_alias_callable_arguments_exit`](../../../tests/omega/pass/generics/cross_module_const_alias_callable_arguments_exit/main.omg): a cross-module alias supplies `Selected<ALIAS>` to `selected<ALIAS, Selected<ALIAS>>`, and its twin [`cross_module_const_alias_shadowed_evidence`](../../../tests/omega/fail/generics/cross_module_const_alias_shadowed_evidence/main.omg) changes only that evidence argument to the same-spelled local constant, so the binding `Implementation satisfies Reading<Minimum>` rejects without reading any dispatched result. A caller still cannot read the result of an evidence-dispatched call directly (`hold(picked)` with `requires picked == 7` is refused even for `choose<Seven>(1, 2)` where `Seven::pick` returns 7).
- An array length that is an expression of the open Const (`[u8; Count + 1]`) fails with an unrelated refusal for the generic callback, with a literal or a named argument alike, and is outside this row.
- Native execution is measured on `windows_x86_64`. `linux_x86_64`, `macos_arm64` and `linux_arm64` are bound in every `build.omg` and realized by binding; they were not executed here.
- The remaining OPERATOR-MACHINE-SUPPLY bullets (`OpenIndexOperationSelection` laws and the `operator` introducer migration) are untouched.
