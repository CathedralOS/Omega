# Fixed-size 256-element array indexing bound preservation

Fact: a static index bound idx < 256u64 carried through the place proves safe fixed-size array indexing arr[idx] on [u64; 256].

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the fixed-size 256-element array indexing bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | The 256-element array and bounded index are verified directly in main |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | The array and bounded index are copied into locals before indexing |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | A record field carries the bounded index alongside the fixed-size array |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | The reading machine receives the bounded index and array directly as parameters |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | The index arrives from a helper call with an ensures result < 256 contract |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | Both branches of a match expression produce in-bounds indices |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | A named constant bounds the index contract for the fixed-size array |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | A transition to a successor machine forwards the bounded index and array |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | A forwarding machine transports the bounded index and array to the reader |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | Each of four target bodies reads the array element using the bounded index |

Run each place with `omega run <place>/main.omg`.

## Pinned fixtures

Each place is pinned under [`tests/omega`](../../../../tests/omega): the positive program as
`pass/facts/fixed_array_twofiftysix_elem_index_bound_through_<place>_exit` (byte-identical
`main.omg` and `build.omg`, `expected_native_exit.txt` = 0) and its twin as
`fail/facts/fixed_array_twofiftysix_elem_index_bound_through_<place>_rejected`
(byte-identical `main.omg`, `expected.txt` = `cannot prove index`).

Evidence with the shared generation producer (`gen-9d9ba4fb625`, revision `9d9ba4fb625`):

- root plus all nine positive places: `omega run --both` — native exit 0 and interp exit: 0;
- all nine twins: `omega --check` — rejected with `cannot prove index ... is within length 256`;
- golden rows for the 18 fixtures recorded in the outcome, `--native` and `--both` legs
  (`python3 tools/corpus_gate.py --runner <gen>/corpus_runner --filter facts/fixed_array_twofiftysix_elem_index_bound --record`).

## Requirements and Evidence

- **Parent outcome:** `LAYOUT-PLANS` / `COUNTS-AND-ADDRESSES` / `STRUCTURAL-ACCESS` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** Fixed-size 256-element array `[u64; 256]` value carrier and in-bounds indexing bounds proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that a static index bound `idx < 256u64` proves safe fixed-size array indexing `arr[idx]` on `[u64; 256]` across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** Fixed-size 256-element array `[u64; 256]` literal construction and static entry precondition `requires idx < 256` (or record constraint `where idx < 256` / named constant `LIMIT = 256`) establish proven in-bounds access before indexing `arr[idx]`. `omega run --both` reports native exit 0 and interpreter exit 0 agreeing for all nine places.
- **Identity and lifetime:** The independent element values are conserved across all value transfers and transitions. Accessing element `arr[142]` yields `142u64`, verified by transition guard `elem == 142` transitioning to `done()` with exit code 0.
- **Composition:** Machine calls, pattern match joins, and state transitions preserve exact preconditions and actual values without narrowing or corruption. Each `invalid/main.omg` differs from its positive twin in only the bound carrying the fact and its explanatory comment:
  - `local`: `arr` and `idx` are copied into local variables before indexing.
  - `field`: `arr` and `idx` are packaged into `Container where idx < 256` and projected.
  - `parameter`: `arr` and `idx` are passed as machine arguments under `requires idx < 256`.
  - `call_result`: `supply_index()` returns `idx` under `ensures result < 256`.
  - `match_join`: `idx` converges across boolean match arms into a safe in-bounds index.
  - `const_name`: `LIMIT = 256` bounds the entry contract.
  - `other_state`: `arr` and `idx` transition to state `element(passed_arr, passed_idx)`.
  - `other_machine`: `fetch_element()` forwards the bounded indexing operation.
  - `target_body`: target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`) perform the read.
  In each place, weakening the bound to `idx <= 256` (or `LIMIT = 257`) causes compile-time bounds verification to strictly fail closed.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: cannot prove index ... is within length 256`, confirming that out-of-bounds indexing is strictly prevented across all nine places.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations, erasing mathematical bounds while emitting verified array projection operations with statically validated length claims. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 2048-byte contiguous memory layout (`256 * 8` bytes, 8-byte aligned), while calling plans assign stack homes or indirect destination pointers (`sret`/by-ref) appropriate for 2048-byte aggregate values.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects scaled-index addressing modes (e.g. `[base + idx * 8]` on `x86_64` and `[base, idx, lsl #3]` on `arm64`). Activation frames allocate disjoint stack storage for the 2048-byte array honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under LAYOUT-PLANS: dynamically sized slice views `&[u64]`, multidimensional array indexing layout plans, and non-power-of-two element counts exceeding register spills.
