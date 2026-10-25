# Returned float equality

A returned f64 equals the live field it came from.

`read_kept` records `result == item.kept && result == 7.0` on the returned float. `agree` reads that record as `saved == item.kept && saved == 7.0`. Every place copies that sentence and performs the same operation: one completed call returns the `f64` field of a live `&Ends`, and the caller passes that result and the same live field to `agree(observed, current)`, whose `requires observed == current && observed == 7.0` is discharged from the callee's `ensures`. Exit 0 means the recorded equality survived the transport; any mismatch reaches `machine bad() { crash Trap; }`. Only the transport of the returned value changes.

## Owning requirement

Parent outcome: **`STATE-LOCAL-VALUE-FRONTIER`** (`TASKS.md`), subhead *Saved observations and arithmetic*: "A saved read of one array element stays equal to that live element and to 7 across a write to a different element … Float probes cover local, field, parameter, match join, const name and a nested machine, with actual overlapping-write twins. Float call result, other machine and target body still fail that equality with no loan; those transports and remaining case/generic/reference composition stay open."

The requirement this row establishes: a completed call's result contract `result == item.kept` is instantiated at the call site with the actual returned value, and that instantiated equality between the returned `f64` and the live field of the caller's borrowed argument reaches the consuming call's `requires` unchanged. Establishment, identity and invalidation, composition, and the checked-to-native lowering of that fact are demonstrated; the *other* carriers the subhead still names (case, generic, reference composition, and the no-loan result transports) remain open under the same item.

### Measured on base `3b7e1df0ff1`

The folder carried `local`, `call_result`, `other_machine` and `target_body`, each with a reject twin, and had no root `main.omg`. The five missing places are authored here, and each one refuses nothing: the compiler already transports the recorded equality through a record field, an explicit parameter, a match join, a const-named operand and a successor machine. This row is therefore coverage for those five places, not a compiler repair.

### Not a variant

This row is not a repeat of a neighbouring float or call-result row. `call_result_bound_discharge` and `independent_call_result_bound` both discharge a `requires` from a *constant* result bound (`opaque() -> 4`, `ensures result == 7`) at a subtraction or comparison, which the bounds engine decides structurally. `saved_observation_snapshot` and `exclusive_observation_exposure` carry a saved *array-element* read and are invalidated by an overlapping write. `float_delivery`, `float_range_delivery`, `ieee_float_ordered_comparison` and `float_comparison_route` lower float comparison, interval and format selection. None of them instantiates a relational result contract between a returned scalar and a caller-side borrowed field: the fact proved here is `saved == item.kept` — provenance, not a constant and not an interval — and it is dropped by the twins when the callee returns the other field.

## Places

| Place | Positive | Reject twin | Transport of the returned value |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) with [`build.omg`](build.omg) | — | canonical root witness: `read_kept` copies the live field into a local and returns it |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the returned value is copied into a local inside the callee and bound to a local in the caller |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | a `Holder` field carries the record, and the returned value crosses the call boundary against its field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | a `consume(item)` machine re-reads the field and passes both the returned value and the live field to `agree` |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the completed call's result is the consumed value |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | the callee returns a `match` over a runtime `flag` whose arms both yield the live field, so the join carries it |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant is the recorded value the returned result is compared against |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine `check(saved, current)` receives the returned value and the live field and consumes them |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `accept(&item, saved)` publishes `saved == item.kept && saved == 7.0` on its own `requires` and forwards both |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies reads the record and consumes the returned value |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares `Expected exit: 0`. Each place's `build.omg` is the same four `Build::bind` rows; only the application name changes.

```sh
omega run --both samples/cli/facts/float_result_equality/main.omg
omega run --both samples/cli/facts/float_result_equality/local/main.omg
omega run --both samples/cli/facts/float_result_equality/field/main.omg
omega run --both samples/cli/facts/float_result_equality/parameter/main.omg
omega run --both samples/cli/facts/float_result_equality/call_result/main.omg
omega run --both samples/cli/facts/float_result_equality/match_join/main.omg
omega run --both samples/cli/facts/float_result_equality/const_name/main.omg
omega run --both samples/cli/facts/float_result_equality/other_state/main.omg
omega run --both samples/cli/facts/float_result_equality/other_machine/main.omg
omega run --both samples/cli/facts/float_result_equality/target_body/main.omg
python3 tools/sample_native_gate.py --facts --filter cli/facts/float_result_equality
```

Measured with a fresh `opt-level 1` compiler built from this tree at `cdc409f3384a` (`python3 tools/build_slot.py -- mbx build -p omega`). This host is Intel macOS `x86_64`, and its catalogued `macos_x86_64` target is not one of the four authored bindings, so bare `omega run --both` stops at `selected target macos_x86_64 has no bound required root slot macos_x86_64::ProgramEntry`. That is host pinning, not a semantic refusal, and no fifth binding is added to the fact sources. The executing form on this host is a scratch copy whose `build.omg` additionally binds `macos_x86_64::ProgramEntry` (and, for `target_body`, whose source additionally declares an identical `macos_x86_64 machine observe(...)` body):

| Place | Native | Terminal Psi | Agreement |
| --- | --- | --- | --- |
| root | `native exit 0` | `interp exit: 0` | yes |
| local | `native exit 0` | `interp exit: 0` | yes |
| field | `native exit 0` | `interp exit: 0` | yes |
| parameter | `native exit 0` | `interp exit: 0` | yes |
| call_result | `native exit 0` | `interp exit: 0` | yes |
| match_join | `native exit 0` | `interp exit: 0` | yes |
| const_name | `native exit 0` | `interp exit: 0` | yes |
| other_state | `native exit 0` | `interp exit: 0` | yes |
| other_machine | `native exit 0` | `interp exit: 0` | yes |
| target_body | `native exit 0` | `interp exit: 0` | yes |

`omega run --target <name>` and `omega run --both --target <name>` are compile publication only (`ExecutionOutcome::TargetOnly`); they never execute and are not runtime evidence, so none of the rows above uses them.

### The oracle discriminates

Exit 0 is strict. Corrupting the consuming comparison in the `local` place from `transition agree(saved, item.kept) == 7.0` to `== 9.0` leaves the equality proved and changes the observed value, so the machine leaves `done()` and reaches `machine bad() { crash Trap; }`: `native signal 4` with `interp crash: Trap (host presentation agrees)`, CLI exit 255 against the authored 0. Both engines refuse the wrong observation.

## Reject twins

Every place has an `invalid/` twin whose `read_kept` returns `item.other` instead of `item.kept` (in the `field` place, `holder.item.other`). Nothing else changes: the same record, the same call site, the same consuming `agree`, the same control flow. The losing machine then no longer meets its own published `ensures`, and proof checking fails closed at exit 1 with the diagnostic naming the recorded equality:

```text
error: cannot prove ensures contract for exit from read_kept at statement 0: result == item.kept && result == 7.0
```

The statement index and the field path follow the place: `local` reports statement 1 (it copies the field into a local before returning) and `field` reports `result == holder.item.kept && result == 7.0`; `const_name` reports `result == item.kept && result == 7.0` because its constant stays `7.0` and only the returned field changes. Each twin's only edit is the field the losing machine returns, so every one rejects for the recorded equality rather than incidentally, and each sample twin carries `cannot prove ensures contract` as its authored `sample.json` `rejection_contains` (mirrored as `expected.txt` in the pinned fixtures).

## Pinned fixtures

The row is pinned by `tests/omega/{pass,fail}/facts/float_result_equality_through_*`:

- Positives: `float_result_equality_through_{local,field,parameter,call_result,match_join,const_name,other_state,other_machine,target_body}_exit`, each carrying `expected_native_exit.txt` = 0 and `expected_interpreter_exit.txt` = 0.
- Twins: `float_result_equality_through_{local,field,parameter,call_result,match_join,const_name,other_state,other_machine,target_body}_rejected`, each carrying `expected.txt` with the `cannot prove ensures contract` fragment.

Every fixture is a byte-identical copy of its sample place or twin, so the sample and the fixture cannot drift apart. `python3 tools/corpus_gate.py --filter facts/float_result_equality` reports 18 fixtures (`checked` 9, `rejected` 9), 0 rejected without their expected fragment, 0 fail fixtures that did not reject.

## Requirements and Evidence

- **Parent outcome:** `STATE-LOCAL-VALUE-FRONTIER` (`TASKS.md`), subhead *Saved observations and arithmetic*.

1. **Establishment:** The equality is established where it originates: `read_kept` is called on a live `&Ends`, and its published `ensures result == item.kept && result == 7.0` is proved at the callee's return from its own `requires item.kept == 7.0` and body. The caller's `agree(saved, item.kept)` then discharges `requires observed == current && observed == 7.0` from that completed call, not from a post-hoc caller guard. `transition agree(saved, item.kept) == 7.0` is the consuming operation's own observation, not a re-establishment of the equality.
2. **Identity and lifetime:** The fact names one exact storage version — the `kept` field of the caller's live `&Ends` at the instant of the call. The twins show the identity is load-bearing: returning `item.other` invalidates the equality even though the record, the call site and the consuming call are unchanged. No write overlaps `item.kept` between establishment and consumption in any place.
3. **Composition:** The same fact composes through all nine places into the consuming call: local copy, record field store and re-read, explicit machine parameter, direct call result, match join over both arms, const-named operand, successor-state argument (with the borrowed record carried alongside), a forwarding machine that republishes the record's contract, and the four target bodies. `<place>/main.omg` and `<place>/invalid/main.omg` differ only in the construct's own output — the field the losing machine returns (in `const_name`, the named constant that value is compared against) — never in the call site or control flow.
4. **Soundness boundaries:** The twins fail closed during compile-time proof checking instead of admitting an invalid contract downstream; the runtime corruption control shows the surviving positive observation is real (`native signal 4` and `interp crash: Trap`). Borrowed access to `item` is preserved across every transport; no place acquires ownership of the record to carry the fact.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The `f64` field read, the completed call, the parameter and state arguments, the match join and the `ensures`/`requires` obligations lower without loss into canonical target-neutral Terminal Psi (`PSITERM\0`) with the proof-only bindings erased, and the Terminal interpreter executes root and all nine places in agreement with native execution (`interp exit: 0` against `native exit 0`).
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles for the four hosted targets through the ordinary Build companion; the `Ends` record's `f64` field layout (`size_of`/`align_of`, field offsets) and the `Holder` record that carries it in the `field` place evaluate target geometry; `CallPlan`/`StatePlan` place the `f64` result and the borrowed record across register and stack lanes, including the `other_state` argument list and the `target_body` bodies.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects the target's scalar move and call instructions for the `f64` result, assigns register homes and disjoint stack-frame homes for the record and the local copies, keeps the borrowed record live across the calls, and inserts the entry prologue and return epilogue within the `StatePlan` ceiling.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Final encoding resolves frame displacements, lays out the basic blocks containing the join and the `done()`/`bad()` branch, validates machine-state footprint evidence, and emits a Mach-O image that executes natively at exit 0 and agrees with Terminal Psi.

Acceptance demonstrated: the same returned-value equality with the live field survives all nine transport places, with nine fact-sensitive reject twins that reject for the recorded equality and a runtime control that traps in both engines. What remains open on `STATE-LOCAL-VALUE-FRONTIER` is the rest of the *Saved observations and arithmetic* frontier — the no-loan result transports it names, case/generic/reference carriers, and its other composition obligations — plus the recorded defect below, which this row does not repair.

## Recorded compiler defect (not repaired here)

**A `const` compared only against itself is never folded, so an identity chain discharges vacuously.** Measured with fresh `opt-level 1` producers built from `3b7e1df0ff1` and `bd34303bacb`; accepted at both, exit 0 with `compiled 29 source file(s)`:

```omg
const KEPT: f64 = 9.0;
data Ends { kept: f64; other: f64; }
machine read_kept(item: &Ends) -> f64
requires item.kept == KEPT
ensures result == item.kept && result == KEPT
{ item.kept }
machine agree(observed: f64, current: f64) -> f64
requires observed == current && observed == KEPT
{ observed }
machine main() crashes Trap {
    let item: Ends = Ends { kept: KEPT, other: 1.0 };
    let saved: f64 = read_kept(&item);
    transition agree(saved, item.kept) == KEPT { true -> done() _ -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
```

The `const` must appear in all three sites — the record initializer, the callee's `requires`/`ensures` on both sides, and the consumer's `requires`. Then every obligation is the syntactic identity `KEPT == KEPT`, so none is ever evaluated: `item.kept == 7.0` never has to be proved from `item.kept == KEPT`, and `saved == 7.0` never has to be proved from `saved == item.kept`. Break the chain and it refuses, which is what makes this a finding rather than a curiosity: initializing the field from a second constant (`OTHER = 3.0`) refuses with two diagnostics, and requiring `observed == OTHER` in the consumer refuses with one. Any literal anywhere on one side of one obligation also forces the fold and refuses.

The `const_name` place is authored so the row does not depend on this: its field is the literal `7.0` and the named constant is the recorded value the returned result is compared against, so `requires item.kept == 7.0` must actually be proved from the field's value. Its twin then moves the constant to `9.0` and rejects with `result == item.kept && result == 9.0`.

### Selective match weighing is keyed to the subject's known value, and that is sound

An earlier reading of this row's authoring probes suggested a wrong acceptance when a match's scrutinee is statically known (`match true { true -> saved, false -> 9 }` is accepted while `match true { true -> 9, false -> saved }` is refused). Three probes settle it as correct behavior rather than a defect, all measured at `bd34303bacb`:

| Probe | Subject | Verdict |
| --- | --- | --- |
| local proven `true` (`let subject: bool = true == true`) | known value, one reachable arm | accepted (correct) |
| `choose(subject: bool, saved)` — the subject is an unconstrained parameter | unknown, both arms reachable | refused (correct) |
| local `true` then `subject = other()` before the match | the initializer's value is retired by the write | refused (correct) |

So the discarded arm is dropped only when the subject's value is known at the match, and a write retires that value — which is what makes the selectable case have exactly one reachable arm. The obligation holds at runtime in every accepted case (`saved == item == 7`), and rejecting them would be a false rejection of valid source.

### Artifact ingress

The emitted Terminal Psi artifacts for this row re-ingest cleanly: `omega run --both --keep` on the `field` and `other_state` places exits 0 in both engines, and `omega inspect-terminal --artifact program.oterm` then reports the module and exits 0 with no malformed-foundation error on either. A sibling row whose scalar field carries a declared domain does hit `Semantic(MalformedStructuralFoundation("scalar field write lacks its required domain identity"))` at `omega-rust/psi/compilation/representations/terminal-psi/codec/src/sections/semantic_module/module_foundation_validation/scalar_fields.rs`, which matters because both `lower_artifact` and `TerminalProgram::load` decode through that validator. This row's `f64` fields carry no such domain, so the limitation does not reach it.
