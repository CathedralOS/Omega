# range_fact_delivery

Declared-range delivery proven at every site: an established range premise on
an exact subject discharges a wider `requires` interval at the call that names
that subject. The fact and the operation are one sentence — *for u64 `index`,
`index < 4` proves `index <= 4` at `count(index)`* — and every place copies it;
only the transport of the premise changes.

Owning requirement: **ONE-INTEGER-BOUNDS-ENGINE** (TASKS.md) — call-argument
domain membership and parameter-versus-literal `requires` entailment through
the integer interval domain — under the declared-range/dependent-value
delivery contract of
[dependent values](../../../../wiki/spec/language/dependent_values.md):
facts are transported only through exact parameter/result substitution and
checked contracts, joins retain only facts justified on every incoming edge,
and state-arrival requirements are proved on every incoming edge. The root
program's f64 row is the same delivery requirement spelled as same-format
bounds propositions under
[numeric values](../../../../wiki/spec/language/numeric_values.md); float
intervals stay a separate engine by the task's own scope.

## Root witness

`main.omg` admits `value >= 0.0 && value <= 0.5` at `admit`, delivers the
narrower declared premise `value <= 0.25` of `forward` into `admit`, nests the
discharging call `outer(admit(bounded))` inside an expression, and discharges
`count(index)`'s `value <= 4` from the guard fact `index < 4`. Every delivery
is judged directly at the call; no caller equality guard re-establishes the
premise after the fact.

## Integer delivery row

For u64 `index`, `index < 4` proves `index <= 4` at `count(index)`. Each
positive place carries the same `probe` premise `input < 4` into the same
`count` call through a different transport, exercises subjects 0 and 3, and
exits 0 only when both return unchanged. Each place binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64` and `linux_arm64`, declares
`Expected exit: 0`, and passes `omega run --both` — native and Terminal Psi
legs agree.

| Place | Positive | Fact-sensitive reject twins |
| --- | --- | --- |
| local | `local/main.omg` | `local/invalid/main.omg`, `local/controls/missing_bound/main.omg` |
| field | `field/main.omg` | `field/invalid/main.omg`, `field/controls/missing_bound/main.omg` |
| parameter | `parameter/main.omg` | `parameter/invalid/main.omg`, `parameter/controls/missing_bound/main.omg` |
| call_result | `call_result/main.omg` | `call_result/invalid/main.omg`, `call_result/controls/missing_bound/main.omg` |
| match_join | `match_join/main.omg` | `match_join/invalid/main.omg`, `match_join/controls/missing_bound/main.omg` |
| const_name | `const_name/main.omg` | `const_name/invalid/main.omg`, `const_name/controls/missing_bound/main.omg` |
| other_state | `other_state/main.omg` | `other_state/invalid/main.omg`, `other_state/controls/missing_bound/main.omg` |
| other_machine | `other_machine/main.omg` | `other_machine/invalid/main.omg`, `other_machine/controls/missing_bound/main.omg` |
| target_body | `target_body/main.omg` | `target_body/invalid/main.omg`, `target_body/controls/missing_bound/main.omg` |

Each twin weakens only the transported premise to `< 6`, keeping `count`'s
`<= 4` requirement and the runtime oracle unchanged, and rejects at the same
`count` call with `cannot prove requires contract for call count ... index <= 4`.
The match twin weakens only its right arm, testing that every predecessor must
establish the joined bound. The const-name place names the source ceiling; it
introduces no new scalar type or proof authority. The target-body twin checks
all four authored target bodies. Positive and twin differ only in the
construct's own bound — never in a caller guard.

Run `omega run --both samples/cli/facts/range_fact_delivery/<place>/main.omg`
with finite stdin. Check a twin with `omega --check <place>/invalid/main.omg` or
`omega --check <place>/controls/missing_bound/main.omg`.

## Requirements and Evidence

- **Establishment:** `probe`'s `requires input < 4` contract (and the root's
  `forward` premise `value <= 0.25` and transition guard `index < 4`)
  establishes the narrower range premise directly at the call edge without any
  post-hoc caller equality guard (`transition x == 100`). The premise introduces
  the static fact on the exact subject, which is verified before admitting the
  operation.
- **Identity and lifetime:** The range fact stays bound to the exact scalar
  subject across local copy (`local`), record field `reading.index` (`field`),
  parameter passing (`parameter`), completed call result (`call_result`),
  match join (`match_join`), named const ceiling (`const_name`), arriving state
  parameter (`other_state`), forwarding machine (`other_machine`), and target
  body (`target_body`). No mutation, alias clobbering, or storage invalidation
  intervenes between fact establishment and consumption.
- **Composition:** The strict `< 4` interval entails `<= 4` at `count(index)`
  across all nine places; calls substitute exact actuals, match joins retain the
  bound justified on every incoming path, and state transitions prove arrival
  obligations on incoming edges. Each `<place>/main.omg` vs.
  `<place>/invalid/main.omg` differs only in the construct's own premise ceiling
  (`4` vs `6`), never in a caller guard.
- **Soundness boundaries:** All nine `invalid/` and `controls/missing_bound/`
  twins reject with `cannot prove requires contract for call count ... index <= 4`
  when any incoming path weakens the premise bound to `< 6`. The checker strictly
  enforces proof obligations rather than passing on unproved assumptions, and
  distinguishes erased static contracts from executable runtime transitions.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Checked `requires` and
  `ensures` range obligations erase cleanly after verification, while the
  executable scalar operations, control flow, and calls lower without loss into
  canonical target-neutral Terminal Psi (`PSITERM\0`). The resulting programs
  execute identically in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):**
  Companion build rows bind `ProgramEntry` and cross-package dependency
  `integer_range_delivery` across all four hosted targets (`windows_x86_64`,
  `linux_x86_64`, `macos_arm64`, `linux_arm64`). Concrete layout geometry for the
  `Reading` record resolves byte alignment and field offsets, and `CallPlan` and
  `StatePlan` place `u64` and `f64` scalar arguments and returns in target ABI
  registers.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):**
  Backend lowering selects target ISA instructions for 64-bit integer and IEEE `f64`
  comparisons and scalar moves, allocates register homes, assigns disjoint
  stack-frame spill slots across nested calls (`outer(admit(bounded))`,
  `count(index)`), and inserts entry prologues and callee-saved epilogues conforming
  to target calling conventions.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):**
  Final encoding resolves symbolic stack-frame displacements, relaxes short/near
  conditional branch offsets with instruction reflow, binds IEEE float constants
  and symbol relocations, validates machine-state footprint evidence, and emits
  executable PE/ELF/Mach-O images whose execution outcomes agree with Terminal Psi
  (`omega run --both` exit 0) across `windows_x86_64` and `macos_arm64`.

## What remains open

Foreign hosted bodies (`windows_x86_64`, `linux_x86_64`, `linux_arm64`) are
source checked here; their native executions record on their own hosts'
goldens. The float admit row covers only same-format f64 delivery; the
separate float engine's mixed-format and non-orderable cases stay under
numeric-values scope. **ONE-INTEGER-BOUNDS-ENGINE**'s unification of the
private interval engines, and wider premise-chain shapes beyond the strict
narrowing shown here, remain open under the task.

## Run record

Measured on `windows_x86_64` with prebuilt `omega-latest.exe`:
- 10/10 samples meet their documented oracle on `windows_x86_64` with `omega run --both` exit 0.
- 18/18 intended-invalid controls (all 9 `<place>/invalid/` twins and 9 `<place>/controls/missing_bound/` controls) match their authored rejection (`cannot prove requires contract for call count ... index <= 4`).

Baseline defect (qualification receipt `df135d5d`, base `ff8be1e0d47`):
five places — `const_name`, `field`, `parameter`, `call_result`,
`other_machine` — carried host-local `omega.lock` files whose recorded
`compiler_identity` predated the current producer, so `omega run --both`
exited 200 with
"accepted package `integer_range_delivery` compiler identity changed; run
`omega update` for fresh review before compiling". That is stale
accepted-review state, not a compiler defect (tools/testing.md). Fresh
`omega update` re-recorded the package review in this checkout for all nine
places; the unchanged programs then passed `--both` with exit 0. No compiler
repair was needed.

Measured on `macos_arm64` @ `ff8be1e0d47`: root plus all nine places exit 0
under `omega run --both`; all nine `controls/missing_bound` twins reject exit 1
with `cannot prove requires contract for call count ... index <= 4`;
`sample_gate.py` reports 19/19 checked with 9 intended rejections matched;
`sample_native_gate.py` reports 10/10 oracles plus 9/9 controls;
`corpus_gate.py` re-pinned the 27 fail rows to record the actual emitted
diagnostics (previously recorded without diagnostic text).

The row is pinned under two selectors: `tests/omega/{pass,fail}/facts/
range_fact_delivery_through_<place>_{exit,rejected}` and
`tests/omega/{pass,fail}/facts/integer_range_delivery_through_<place>_{exit,
missing_bound}` — the integer `count` row pinned twice under both names.
Every positive records native exit 0, every twin records its emitted
rejection diagnostic. (The `float_range_delivery_through_*` family pins the
sibling `samples/cli/facts/float_range_delivery/` row, not this folder.)

```sh
omega run --both samples/cli/facts/range_fact_delivery/local/main.omg
omega --check samples/cli/facts/range_fact_delivery/local/controls/missing_bound/main.omg
python3 tools/sample_gate.py --filter cli/facts/range_fact_delivery --cli <omega>
python3 tools/sample_native_gate.py --filter cli/facts/range_fact_delivery --cli <omega>
python3 tools/corpus_gate.py --filter range_delivery,range_fact_delivery
```
