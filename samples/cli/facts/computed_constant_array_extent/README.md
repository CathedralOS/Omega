# Computed constant array extent completion

MODULE-NAMESPACE-RESOLUTION: an array initializer and its alias use the exact
completed value of the selected computed extent. The unchanged root computes
`SIZE=1+0`, `SOURCE=[3+0]`, then observes `ALIAS[0]==3` in native and Terminal
execution. A wrong observation crashes, so exit zero asserts the value 3.

The frozen clean compiler at `53cd2938437a29356a48d6eaf29f14de07a4bb99`
refused the root at `array extent constant is not a completed integer literal`.
The repair schedules declared-type dependencies before canonical value encoding,
retains symbolic pending values privately, and rejoins exact declaration custody.
It does not use provisional aggregate values as dependency evidence.

`composition` exercises transitive extents, empty and nested arrays, record field
geometry, a closed array result and a match join. `module_owner` selects the
imported producer's extent despite a consumer shadow. `qualified_element` keeps
a qualified literal element behind pending geometry. All nine standard transport
places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`,
`other_state`, `other_machine`, `target_body`) and their `invalid/` twins carry
the computed constant array extent `SIZE = 1 + 0` and element `SOURCE[0] = 3`
end to end across all four hosted `ProgramEntry` contracts.

| Place / Program | Witness (`main.omg`) | Reject twin (`invalid/main.omg`) | Distinct mechanism exercised |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | - | Root package probe computing `SIZE = 1 + 0`, `SOURCE = [3 + 0]`, `ALIAS: [u64; 1] = SOURCE`, and observing `ALIAS[0] == 3`. |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | Computed-extent array copied into `let local_arr: [u64; SIZE] = ALIAS` before indexing `local_arr[0] == 3`. |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | Computed-extent array stored in nominal record field `Holder.items: [u64; SIZE]` and projected as `held.items[0] == 3`. |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Computed-extent array passed as `arr: &[u64; SIZE]` parameter to `check_param`. |
| call_result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Helper machine `make() -> [u64; SIZE]` returns computed-extent array into `CALLED`. |
| match_join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Computed-extent array unified across conditional `match` branches (`MATCHED: [u64; SIZE]`). |
| const_name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Transitive computed constants (`BASE`, `SIZE`, `SOURCE`, `ALIAS`, `SELECTED`) establish extent and element value. |
| other_state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | State transition passes `local_arr[0]` into successor `machine inspect(v: u64)`. |
| other_machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Cross-machine call `Inspector::check(&local_arr)` verifies `arr[0] == 3` on `&[u64; SIZE]`. |
| target_body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Target-scoped `select_val()` bodies across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` index `ALIAS[0]`. |
| composition | [`composition/main.omg`](composition/main.omg) | - | Transitive extents, empty and nested arrays, record field geometry, closed array result and match join. |
| module_owner | [`module_owner/main.omg`](module_owner/main.omg) | - | Selects imported producer's extent despite a consumer shadow `const SIZE: u64 = 2 + 0`. |
| qualified_element | [`qualified_element/main.omg`](qualified_element/main.omg) | [`controls/qualified_element/main.omg`](controls/qualified_element/main.omg) | Qualified literal element behind pending array geometry. |

Perturbing a single token in the root oracle (`transition ALIAS[0] == 3` -> `transition ALIAS[0] == 4`) causes both engines to fail closed with `crash Trap` (`interp crash: Trap`, `native exit -1073741795`).
Each standard `<place>/invalid/main.omg` twin changes only the index from `0` to `1` and rejects with `cannot prove index`. Additional controls change one obligation: `controls/mismatch` adds an initializer element,
`controls/index` changes the index to 1, and `controls/qualified_element` changes
the qualified element value from 3 to 4. They must reject cardinality, bounds and
the original default-domain predicate respectively.

## Pinned fixtures

- Standard 9-place positive fixtures (9):
  - `tests/omega/pass/facts/computed_constant_array_extent_through_local_exit`
  - `tests/omega/pass/facts/computed_constant_array_extent_through_field_exit`
  - `tests/omega/pass/facts/computed_constant_array_extent_through_parameter_exit`
  - `tests/omega/pass/facts/computed_constant_array_extent_through_call_result_exit`
  - `tests/omega/pass/facts/computed_constant_array_extent_through_match_join_exit`
  - `tests/omega/pass/facts/computed_constant_array_extent_through_const_name_exit`
  - `tests/omega/pass/facts/computed_constant_array_extent_through_other_state_exit`
  - `tests/omega/pass/facts/computed_constant_array_extent_through_other_machine_exit`
  - `tests/omega/pass/facts/computed_constant_array_extent_through_target_body_exit`
- Standard 9-place reject fixtures (9):
  - `tests/omega/fail/facts/computed_constant_array_extent_through_local_rejected`
  - `tests/omega/fail/facts/computed_constant_array_extent_through_field_rejected`
  - `tests/omega/fail/facts/computed_constant_array_extent_through_parameter_rejected`
  - `tests/omega/fail/facts/computed_constant_array_extent_through_call_result_rejected`
  - `tests/omega/fail/facts/computed_constant_array_extent_through_match_join_rejected`
  - `tests/omega/fail/facts/computed_constant_array_extent_through_const_name_rejected`
  - `tests/omega/fail/facts/computed_constant_array_extent_through_other_state_rejected`
  - `tests/omega/fail/facts/computed_constant_array_extent_through_other_machine_rejected`
  - `tests/omega/fail/facts/computed_constant_array_extent_through_target_body_rejected`
- Root & control fixtures:
  - `tests/omega/pass/facts/computed_constant_array_extent_exit`
  - `tests/omega/fail/facts/computed_constant_array_extent_{mismatch,index,qualified_element}_rejected`

| Obligation Class | How This Fact Row Establishes It |
|---|---|
| **Establishment** | `const SIZE: u64 = 1 + 0; const SOURCE: [u64; SIZE] = [3 + 0]; const ALIAS: [u64; 1] = SOURCE;` evaluates declared-type extent dependencies before canonical value encoding so `[u64; SIZE]` and `[u64; 1]` unify and prove index `0 < 1` ([constants](../../../../wiki/spec/language/constants.md), [evaluation](../../../../wiki/spec/language/evaluation.md)). |
| **Identity and lifetime** | The fixed-size array carrier `[u64; SIZE]` and borrowed `&[u64; SIZE]` retain exact extent `1` and element value `3` across local copies, `Holder` fields, parameters, and constant bindings ([counts and addresses](../../../../wiki/spec/language/counts_and_addresses.md), [modules](../../../../wiki/spec/language/modules.md)). |
| **Composition** | Across all nine standard transport places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`), `[u64; SIZE]` preserves its completed extent `1` and element `3`. |
| **Soundness boundaries** | Each `<place>/invalid/main.omg` changes the index from `0` to `1` and rejects with `cannot prove index`; `controls/mismatch` and `controls/qualified_element` reject cardinality and domain mismatches. |
| **Checked-to-Terminal Psi lowering (`Psi 05–07`)** | Completed array constants and indexed reads lower into canonical `PSITERM\0` operations and execute in the Terminal Psi interpreter (`omega run --both`). |
| **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)** | `ProgramEntry` settles across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`; 8-byte `[u64; 1]` and `Holder` layouts and calling plans validate without reopening source. |
| **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)** | Backend lowering realizes fixed-size array stack homes, borrowed slice/array pointer projections, and 64-bit equality comparisons within bounded activation frames. |
| **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)** | Native PE/ELF/Mach-O images emit and execute on `windows_x86_64` and `linux_x86_64` with exit code `0`, agreeing with Terminal Psi (`omega run --both`). | Static
formation and exact module identity feed ordinary checking and checked-to-Terminal
lowering; actual both-engine executions exercise provider settlement, concrete
array layout, calling/activation realization and native ELF emission. No new
provider, ABI, loan or cleanup mechanism is introduced. Untouched target bindings
do not establish other-host execution.

Governing contracts: [constants](../../../../wiki/spec/language/constants.md),
[evaluation](../../../../wiki/spec/language/evaluation.md),
[counts and addresses](../../../../wiki/spec/language/counts_and_addresses.md),
[modules](../../../../wiki/spec/language/modules.md). Broader module evaluation
remains open, including specialized provider/target applications, abstract/open
carriers and other structured proof subjects. Literal-array argument evaluation,
array-indexed record predicates and array `.len` observations have separate
baseline limitations; these witnesses do not claim those mechanisms.

Constant evaluation of generic constructors through named const arguments remains
unsupported; explicit literal generic construction is preserved. This is outside
the completed declaration-array readiness mechanism.
