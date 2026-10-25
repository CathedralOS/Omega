# data_where_standing

Counter.value <= 60 makes exact i32 70 + value representable without a per-call bound.

The root `main.omg` and nine child projects are exit-0 native witnesses of the
same operation, with `// Expected exit: 0` and `crashes Trap` on failure.
Each `invalid/` twin removes only the data `where value <= 60` clause;
construction, transport, call, arithmetic carrier, and operation stay identical.
The unbounded `i32` can exceed `2147483577`, so its addition cannot be proved
representable. The lower carrier endpoint remains safe after adding 70.

## Places

| Place | Positive | Reject twin | Evidence transported |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root entry reading field from struct-level invariant |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | scalar local copied from declared field |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | nested field projection |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | owned Counter parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Counter returned by an ordinary machine |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | scalar join from two bounded fields |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named Counter supplied to a const binder |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Counter passed to successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Counter forwarded to another machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | four separately checked target bodies |

The generic bodies must justify their additions independently of the entry's
literal value. In particular, the const-name twin must reject before concrete
specialization can conceal a missing standing premise.

Canonical authority is the declared default-domain and delivery contract in
[`dependent_values.md`](../../../../wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization)
and the per-operation representability rule in
[`numeric_values.md`](../../../../wiki/spec/language/numeric_values.md#arithmetic-and-comparisons).
Neither a safe final value nor the caller's literal supplies an unproved
intermediate operation with evidence.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/data_where_standing/main.omg
omega run --both samples/cli/facts/data_where_standing/local/main.omg
omega run --both samples/cli/facts/data_where_standing/field/main.omg
omega run --both samples/cli/facts/data_where_standing/parameter/main.omg
omega run --both samples/cli/facts/data_where_standing/call_result/main.omg
omega run --both samples/cli/facts/data_where_standing/match_join/main.omg
omega run --both samples/cli/facts/data_where_standing/const_name/main.omg
omega run --both samples/cli/facts/data_where_standing/other_state/main.omg
omega run --both samples/cli/facts/data_where_standing/other_machine/main.omg
omega run --both samples/cli/facts/data_where_standing/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The authored exit 0 is not vacuously assumed:
- In root `main.omg`, inverting the guard `transition total <= 130` to `total > 130` routes execution to `fail(main)` which invokes `crash Trap;`, terminating with native signal 5 and interpreter crash `Trap`.
- In child places, failing the outcome condition (e.g. `compute(&counter) == 130 && compute(&low) == -2147483578`) branches to `machine bad() { crash Trap; }`, trapping with `rc=255` under both engines.
- In each `invalid/` twin, removing the standing invariant `where value <= 60` from `data Counter` causes proof checking to fail closed with `exact arithmetic in machine ... may overflow `i32``, preventing execution entirely.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/data_where_standing/<place>/invalid/main.omg
```

Every twin is rejected during checked compilation with `may overflow `i32``.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/data_where_standing --jobs 4
```

`sample_native_gate.py` confirms that 10/10 samples meet their documented oracle on `windows_x86_64` (and `macos_arm64`), and 9/9 intended-invalid controls match their authored rejection.

### Historical Diagnostic Context

The first semantic refusal on base `55352539edae` was in `const_name`:
`data Counter has default-domain facts whose index-site proof is not implemented`.
All other places checked. The root application and all nine places pass natively. All nine unbounded
twins rejected their exact addition, including the const generic's body before
specialization. The repair separates structural eligibility from proving each
completed index value's required predicates; it does not infer membership from
canonical bytes or waive data validity.

## Pinned Fixtures

The row is pinned by `tests/omega/pass/facts/data_where_standing_through_<place>_exit` and `tests/omega/fail/facts/data_where_standing_through_<place>_rejected` fixtures.
The nine positive places are pinned under `tests/omega/pass/facts/`, each binding `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and recording native exit 0.
The nine reject twins are pinned under `tests/omega/fail/facts/`, each rejecting with the exact arithmetic overflow diagnostic.

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`).
- **Requirement established:** End-to-end standing invariant (`data ... where`) preservation and integer arithmetic representability across all nine language transport places. Specifically, a declared standing invariant on a nominal record (`data Counter where value <= 60`) attaches intrinsically to the record type, travels intact across every transport place without decay or loss, and discharges proof obligations for exact integer arithmetic (`70 + value`) without requiring post-hoc caller equality guards or per-call bounds, with exact agreement between Terminal Psi and native execution across all four hosted targets.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Constructing or receiving a `data ... where` record (via literal construction `Counter { value: 60 }`, `Counter { value: -2147483648 }`, or default-domain zero-initialization `main.counter`) directly establishes the standing invariant `value <= 60` on the value at the point of construction. Consuming machines and states rely directly on the type-level standing invariant; no post-hoc caller equality guard (`transition x == 100`) or caller-side condition is required to establish or introduce the fact.
  - **Identity and lifetime:** The `data ... where` standing invariant remains attached to the exact record value and its projected fields (`counter.value`) across moves, copies (`[copy]`), and borrows (`&Counter`). Because `Counter` is a copyable value type, copying or borrowing preserves the invariant across storage homes. Unrelated mutations to adjacent or enclosing storage (such as modifying `main.hits = 0`) leave the standing invariant on `counter.value` unaffected.
  - **Composition:** The standing `where` clause fact travels through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) to discharge the consuming exact arithmetic operation (`70 + value`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the presence or absence of the standing invariant `where value <= 60,` on `data Counter`, while keeping all construction, transport, arithmetic carriers, and callee contracts identical.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during proof checking when the standing invariant `where value <= 60` is removed. An unconstrained `i32` can exceed `2147483577`, so `70 + value` may overflow `i32` in exact arithmetic; checking rejects at compile time with `exact arithmetic in machine ... state ... terminal expression may overflow \`i32\`` (decision 17). Values violating or lacking standing invariants cannot be formed or consumed in exact operations.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The standing `where` clause proof obligation erases completely after static verification, incurring zero runtime proof overhead. Structural record fields, exact arithmetic operations, match joins, state transitions, and target-scoped bodies lower into canonical target-neutral Terminal Psi (`PSITERM\0`). All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native execution exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations. The `data Counter` record layout plan validates with `size_of == 4` and `align_of == 4` with field `value: i32` at offset 0, confirming zero physical memory overhead for the standing `where` clause. `CallPlan` and `StatePlan` lower the record across target ABI register and stack lanes according to target calling conventions.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 32-bit signed addition instructions (e.g. `add` on `x86_64`, `add` on `arm64`) and comparison instructions directly without inserting redundant runtime overflow checks. The backend assigns register homes and disjoint activation storage for `Counter` instances and locals, composes worst-case stack usage (WCSU), and inserts standard frame prologues and epilogues honoring the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve for record field loads, basic blocks lay out with short/near branch relaxation and offset reflow, machine-state footprint evidence validates, and direct PE, ELF, and Mach-O executables are emitted. Terminal Psi and native execution agree with exit 0 (`omega run --both`) across hosted targets including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Relational standing invariants linking multiple record fields (`where low <= high`), mutable borrow updates to fields governed by standing invariants (`&mut counter.value`) with restoration windows, and generic data types with standing predicates parameterized over type variables.
