# Case-where equality

Case `Point(lo, hi) where lo == hi` makes `hi - lo` exact in `u64`, without
a guard or Wrapping. The fact-place requirement keeps this fact and operation
unchanged across all nine places. The nine source-check places include a named
constant selected through a static binder, whose generic body needs the case predicate.
Each qualifying twin removes only that predicate and reaches subtraction rejection.

The root project and all nine place projects bind `ProgramEntry` for host
targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), initialize
`main.width = 99` and an interval payload where `lo == hi` (such as `17, 17` or `4, 4`),
and declare `Expected exit: 0`. The exact `hi - lo = 0` subtraction result writes
`main.width = 0`, and the transition on `main.width == 0` verifies execution reaches
`pass()` (exit 0) rather than `fail() { crash Trap; }`. Both native and Terminal Psi
execution are verified (`omega run --both`).

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) with [`build.omg`](build.omg) | — | canonical root witness with `Interval::Point(17, 17)` |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | copied local interval |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | borrowed context field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit interval parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | identity machine call result |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | two distinct interval fields joined by a match |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named `Point { lo: 4, hi: 4 }` constant through static `const INTERVAL: Interval` binder |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | interval forwarded to a nested machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | interval forwarded to another machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | four hosted target bodies |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares `Expected exit: 0`. `omega run --both` reports Terminal Psi interpreter and native execution agreement (`interp exit: 0` and `native exit 0`):

```sh
omega run --both samples/cli/facts/case_where_equality/main.omg
omega run --both samples/cli/facts/case_where_equality/local/main.omg
omega run --both samples/cli/facts/case_where_equality/field/main.omg
omega run --both samples/cli/facts/case_where_equality/parameter/main.omg
omega run --both samples/cli/facts/case_where_equality/call_result/main.omg
omega run --both samples/cli/facts/case_where_equality/match_join/main.omg
omega run --both samples/cli/facts/case_where_equality/const_name/main.omg
omega run --both samples/cli/facts/case_where_equality/other_state/main.omg
omega run --both samples/cli/facts/case_where_equality/other_machine/main.omg
omega run --both samples/cli/facts/case_where_equality/target_body/main.omg
python tools/sample_native_gate.py --facts --filter case_where_equality
```

### The oracle discriminates

The exit code `0` is strictly discriminative. Exact subtraction `hi - lo = 0` sets `main.width = 0`. The machine transitions on `main.width == 0` to `pass()` (exit 0); any unexpected width branches to `fail()` which executes `crash Trap;`. Both native and Terminal Psi engines abort non-zero on failure.

## Reject Twins

Each place has an `invalid/` reject twin with authored `sample.json` (`"role": "intended-invalid"` and `"rejection_contains": ["exact arithmetic", "may overflow `u64`"]`) and `expected.txt`. All nine twins remove only `where lo == hi`, retaining the same `u64` carriers, transition, state parameters, and exact `hi - lo` assignment. In the const twin, the named `4`/`4` constructor remains valid, but it cannot strengthen the generic body's declared contract. Proof checking fails closed during compile-time verification:

| Place | Twin | Pinned Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `exact arithmetic ... may overflow \`u64\`` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `exact arithmetic ... may overflow \`u64\`` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `exact arithmetic ... may overflow \`u64\`` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `exact arithmetic ... may overflow \`u64\`` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `exact arithmetic ... may overflow \`u64\`` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `exact arithmetic ... may overflow \`u64\`` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `exact arithmetic ... may overflow \`u64\`` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `exact arithmetic ... may overflow \`u64\`` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `exact arithmetic ... may overflow \`u64\`` |

## Pinned Fixtures

The sample folder is named `case_where_equality`; its fixture families are
`case_equality_through_<place>_compile` (check-only siblings) and
`case_where_equality_through_<place>_exit` (the runtime witnesses). Both were
listed here before either was measured; only the `_compile` family had golden
rows. The nine `_exit` rows below are now recorded.

- Positive passes: `tests/omega/pass/facts/case_equality_through_<place>_compile` and `tests/omega/pass/facts/case_where_equality_through_<place>_exit` (all 9 places pinned, each recording native exit 0).
- Negative controls: `tests/omega/fail/facts/case_equality_through_<place>_compile` and `tests/omega/fail/facts/case_where_equality_through_<place>_rejected` (all 9 places pinned, rejecting with exact arithmetic overflow).

### Recorded measurements

All nine `_exit` fixtures record `built ... exit:0` in
`tests/omega/corpus_native_macos_arm64.txt`, and the same nine record
`interpreter-exit:0 interpreter:agrees` in
`tests/omega/corpus_both_macos_arm64.txt`, which is the pinned Terminal
Psi/native agreement. The nine reject twins record `rejected ... expected` in
`tests/omega/corpus_outcomes.txt`. Root plus nine places each print
`native exit 0` / `interp exit: 0` under `omega run --both`;
`tools/sample_native_gate.py --filter cli/facts/case_where_equality` reports
10/10 pass, 11/11 intended-invalid controls matched, 2 check-only probes.

### The oracle discriminates, and the corruption refuses statically

Exit `0` separates the exact subtraction from every other outcome: `hi - lo = 0`
selects `pass()`, and any other width selects `fail() { crash Trap; }`. Forcing
that fail path (adding `+7u64` after the proved subtraction, so a non-zero width
reaches the transition) makes both engines trap:

```text
native signal 5
interp crash: Trap (host presentation agrees)   exit 255
```

The baseline keeps `native exit 0` / `interp exit: 0`, so the oracle is
discriminative in both directions.

The case predicate itself cannot be corrupted into a runtime trap. Every
corruption of the construct's own input is refused during source checking,
before native production:

| Corruption | Observed source result |
| --- | --- |
| `Point { lo: 17, hi: 3 }` against its own `where lo == hi` | static refusal: `a where fact evaluates FALSE at this construction (ch12: construction is the gate)` |
| `Point { lo, hi }` from runtime parameters bounded only by `requires lo <= 100u64 && hi <= 100u64` | static refusal: `cannot prove the case constraint: a where fact is not established by the values observed at this construction` |
| remove `where lo == hi`, keep the `4`/`4` constructor | **checks and runs** (`native exit 0`) -- closed scalar projection proves `4 - 4` independently, as the materialized-literal audit below already recorded |

So the fact is enforced at the construction gate, not at runtime: only a
corruption applied *after* the proved subtraction reaches the trap path above.
That distinction is why this row's discrimination needs the post-proof width
corruption rather than an unequal payload.

`Interval [copy]` explicitly permits the ordinary copies used by the place
programs. Unannotated owned data is affine even when all its fields are scalar;
the initial borrowed-storage transfer refusals were required ownership checks,
not arithmetic defects. The original root source remains unchanged.

On base `2ef2ab353498f0830f6023d8a023b63d21664cb9` (containing
`73ecbccba3`), eight place programs checked after that declaration. The const
place first rejected a complete `Point`/`Empty` dispatch as non-exhaustive, then
rejected exact subtraction after coverage was repaired. The compiler repair
compares complete aggregate dispatch subjects, binds substituted payload fields
to their exact case, and extends closed scalar projection through the complete
common/active constructor roster. It retains case predicates, ownership,
closed-sibling and Exact-policy obligations.

The focused corpus also checks constant-array dispatch, distinct aggregate axes,
common fields with same-named sibling payloads, and unsafe sibling arithmetic.
An additional maximum-`u64` construction probe still fails on the exact base
before this repair: its case predicate is not established. That existing wide
construction gap is outside this authored constant-4 witness and is not claimed
fixed by the source checks here.

## Materialized-literal audit controls

The audit at `e0ddeec45d6b36ad90d1ace436b47d6b1741f67a` removes only the
case's `where lo == hi`, keeping the named constant, `4`/`4` payloads,
transition, state parameters, and exact `hi - lo` assignment. It still checks.
The existing const-name rejection therefore did not establish that the
transported predicate was needed. The earlier 9/9 completion claim for that witness was incorrect; the audit
correction correctly reopened acceptance before the static-binder repair below.

| Case predicate | Named constant payloads | Observed source result |
| --- | --- | --- |
| `lo == hi` | `lo: 4, hi: 4` | checks (`controls/literal_equal_with_predicate`) |
| absent | `lo: 4, hi: 4` | checks (`controls/literal_equal_without_predicate`) |
| absent | `lo: 4, hi: 3` | rejects `state take` exact `u64` subtraction (`controls/invalid_unequal_without_predicate`) |
| `lo == hi` | `lo: 4, hi: 3` | rejects constant case predicate (`controls/invalid_unequal_with_predicate`) |

The two added controls differ only in `hi: 4` versus `hi: 3`. They establish
closed-scalar subtraction checking, not case-predicate transport. Corpus mirrors
are `pass/facts/case_const_equal_literals_without_predicate` and
`fail/facts/case_const_unequal_literals_subtraction_rejected`.

This limitation follows the current witness's representation. The
[constant contract](../../../../wiki/spec/language/constants.md) names an
immutable compile-time value and requests semantic evaluation; symbolic
mathematical definitions are a separate construct. The existing closed
constructor projection in
`omega-rust/psi/compilation/checking/src/value_custody/literals/record_projection.rs`
retains the landed payload scalars. Those known `4`/`4` values independently
prove exact subtraction. Under the
[case contract](../../../../wiki/spec/language/data_and_literals.md#case-constraints),
an omitted clause contributes true, and
[Exact arithmetic](../../../../wiki/spec/language/numeric_values.md#arithmetic-and-comparisons)
requires representability, not dependence on a particular proof premise.
Rejecting safe `4 - 4` merely because the optional clause is absent would change
those rules. Disabling scalar projection would hide valid evidence rather than
repair the compiler.

No new language restriction or impossibility theorem follows from those
controls. The materialized-literal route cannot distinguish predicate necessity.

## Named constant through a static binder

`const_name/main.omg` retains the named `Point { lo: 4, hi: 4 }` value as
`NAMED_INTERVAL`. `Main::selected` explicitly selects it into
`Main::main<const INTERVAL: Interval>`. That body directly dispatches the const
name, forwards the same payload bindings into `take`, and assigns `hi - lo`.
It introduces no runtime parameter for the interval and adds no arithmetic guard.

The [generic contract](../../../../wiki/spec/language/generics.md#value-binders-and-const-requirements)
makes this binder statically supplied within its specialization. Generic bodies
check against their declared requirements: a concrete selection cannot silently
strengthen the API. Matching the binder's established `Point` case therefore
supplies the equality needed by the generic body. Removing only the predicate
rejects subtraction there even though the concrete `4`/`4` constructor is valid.
This demonstrates the fact through a const name without pretending literal
arithmetic depends on the predicate.

On base `866a955e52f8abcdce6d7b80e18fbbb772f51205`, the full named selection
rejected exact arithmetic in `Main::main$specialized$…` state `take`. The shared
declared-receiver lookup omitted static and runtime-capable value binders. Binding
their members through the authored carrier preserves exact payload identities
into specialization. Existing case/field owner checks and scalar projection stay
in place; no predicate or arithmetic obligation is relaxed. Forwarded const
selection and a runtime-capable value binder pin the same operation's composition.

An unused, uninstantiated binder-only scratch probe still reports its pre-existing
unresolved authored member selection. It does not supply the assigned named
constant/application journey and is not claimed repaired. The root program and
nine assigned routes execute natively and under Terminal Psi with `Expected exit: 0`;
all predicate-erasure twins and materialized controls reject with their authored diagnostics.

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`).

### Specification and Lowering Layers

1. **Establishment:** The case payload refinement `Point(lo: u64, hi: u64) where lo == hi` in enum/data type `Interval` establishes static equality between the payload fields `lo` and `hi` upon construction and matching. When projected into an exact arithmetic subtraction expression `hi - lo`, the compile-time proof engine proves non-negative representability in unsigned `u64` (`hi >= lo` identically discharged from `lo == hi`), evaluating to 0 without requiring runtime guards, wrapping, or post-hoc caller equality checks (`transition x == 100`).
2. **Identity and lifetime:** The established equality `lo == hi` attaches strictly to the unpacked case payload bindings of the specific `Point` variant instance. Moving or copying the interval value (`Interval [copy]`) preserves the standing equality invariant across value transfers, while invalid construction (e.g. attempting to construct `Point(4, 3)` with an unsatisfied `lo == hi` predicate) or mismatched variant extraction fails closed at compile time before invalid storage can be observed.
3. **Composition:** The case equality invariant composes across all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) to prove exact `hi - lo = 0` subtraction in `main.width = hi - lo`. Each place's positive `<place>/main.omg` vs. its reject twin `<place>/invalid/main.omg` differs only in the case predicate clause (`where lo == hi` present vs removed), cleanly isolating the transport and consumption of the exact equality fact across all places.
4. **Soundness boundaries:** All 9 reject twins fail closed during source proof checking when `where lo == hi` is omitted. Without the equality premise, exact subtraction `hi - lo` on unconstrained `u64` operands is not provably in range, rejecting with compile-time diagnostic `error: exact arithmetic in machine ... state ... assignment may overflow `u64`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation)`. This ensures that unverified arithmetic cannot compile, bypass proof gates, or reach code generation.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs—including tagged variant layout, pattern match dispatch, case payload projection, exact integer subtraction, and state transitions—lower without information loss into canonical target-neutral Terminal Psi (`PSITERM\0`). Mathematical proof obligations and erased predicate annotations are checked and erased, while preserving explicit affine cleanup and state transition semantics. The Terminal Psi interpreter executes every positive place in complete agreement with native execution (`omega run --both`).
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`, `ProviderPlan`, `CompilerBuiltinExecution`, `BoundaryExecutionBinding`) across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Tagged case variant layout plans (`size_of`, `align_of`, discriminant tags, and payload offsets for `Point` and `Empty`) are evaluated and validated. `CallPlan` and `StatePlan` assign machine state parameters, calling arguments, and return homes to register and stack lanes according to target platform ABIs.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend instruction selection emits target ISA instructions (`x86_64` and `arm64`) for variant tag comparison, payload field extraction, and integer subtraction (`sub`). Register allocator selects register homes and disjoint activation frame spill slots, composing worst-case stack usage (WCSU) bounds and emitting platform-standard prologues and epilogues conforming to `StatePlan` ceilings.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Final encoding resolves symbolic branch displacements and stack frame offsets, relaxes function-relative jumps, validates transitive machine-state footprints, and emits direct PE/ELF/Mach-O executable placements. Native binaries execute natively with exit code 0 and match Terminal Psi interpretation across all hosted platforms.
