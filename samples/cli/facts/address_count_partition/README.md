# Address/count interval partition

The fact is that an address base and a count length partition into disjoint
intervals with exactly the original content. The operation is a checked split,
with the requested count bounded by the input length and the input endpoint
bounded by a separately formed address ceiling. No host address width is assumed.

Every place performs the same operation: split a `Region { base: 10, length: 80 }`
at a requested count of `20` under an address `ceiling` of `100`, verify that the
left piece is `{base: 10, length: 20}` and the right piece is `{base: 30,
length: 60}`, and exit 0 only when both hold. Only the transport of the operands
changes.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) with [`build.omg`](build.omg) | — | canonical root witness partitioning base address and count |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the requested count is copied into a local, after an unrelated local store |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded record fields carry the base, length and requested count |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the split machine reads `length` and `ceiling` directly |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a call with a declared result supplies the requested count |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms keep the bounded count, so the joined count splits safely |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `const LENGTH: u64 = 20` provides the named requested count |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives the operands and splits there |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports operands to the split machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies splits the base and count |

## Execution

Each positive place and the root `main.omg` binds `ProgramEntry` for all four
hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`),
declares `Expected exit: 0`, and passes `omega run --both` — Terminal Psi and
native execution agreeing:

```sh
omega run --both samples/cli/facts/address_count_partition/main.omg
omega run --both samples/cli/facts/address_count_partition/local/main.omg
omega run --both samples/cli/facts/address_count_partition/field/main.omg
omega run --both samples/cli/facts/address_count_partition/parameter/main.omg
omega run --both samples/cli/facts/address_count_partition/call_result/main.omg
omega run --both samples/cli/facts/address_count_partition/match_join/main.omg
omega run --both samples/cli/facts/address_count_partition/const_name/main.omg
omega run --both samples/cli/facts/address_count_partition/other_state/main.omg
omega run --both samples/cli/facts/address_count_partition/other_machine/main.omg
omega run --both samples/cli/facts/address_count_partition/target_body/main.omg
```

### The oracle discriminates

Discrimination was verified with two independent probes:
1. Corrupting the requested partition count (`20` -> `25`, so the left piece is no longer `{base: 10, length: 20}`) branches to `bad()`, producing `native signal 5` together with `interp crash: Trap` (host presentation agrees).
2. Corrupting the address `ceiling` (`100` -> `89`, below the interval end `90`) is refused at the call site with `cannot prove requires contract for call split from main ...`.

The oracle therefore strictly discriminates: both engines trap on an incorrect partition at runtime, and the address-ceiling bound is enforced statically at compile time.

## Reject Twins

Each place has an `invalid/` twin that breaks the partition contract by dropping
the `length <= whole.length` bound from the `split` machine's own `requires`
clause; the requested count is then unbounded relative to the input, and the
residual `whole.length - requested` is no longer provably in `u64` range. The
twin differs from its positive only in the construct's own contract — never in a
caller guard. It carries its authored rejection expectation in
`invalid/sample.json` (`rejection_contains`) and `invalid/expected.txt`, rejecting
cleanly during compile-time proof checking:

| Place | Twin | Pinned Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `exact arithmetic` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `exact arithmetic` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `exact arithmetic` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `exact arithmetic` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `exact arithmetic` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `exact arithmetic` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `exact arithmetic` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `exact arithmetic` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `exact arithmetic` |

## Pinned Fixtures

- Positive passes: `tests/omega/pass/facts/address_count_partition_through_<place>_exit` (all 9 places pinned, each recording native exit 0).
- Negative controls: `tests/omega/fail/facts/address_count_partition_through_<place>_rejected` (all 9 places pinned, rejecting with `exact arithmetic`).
- Core regressions:
  - `tests/omega/pass/core/address_interval_partition_unrelated_store_exit`
  - `tests/omega/pass/core/address_interval_content_partition_exit`
  - `tests/omega/fail/core/address_interval_partition_missing_ceiling`
  - `tests/omega/fail/core/address_interval_partition_missing_count_bound`
  - `tests/omega/fail/core/address_interval_partition_overlap`

## Requirements and Evidence

- **Owning requirement:** `wiki/spec/language/counts_and_addresses.md`
- **Parent task:** `BUMP-ALLOCATOR-CANARY` (`TASKS.md`)
- **Reserved ticket:** `3a4e4800ae2040059680d6c228f5683f`

### Specification and Lowering Layers

1. **Establishment:** The `split` machine entry contract (`requires length <= whole.length && embed(whole.base) + embed(whole.length) <= embed(ceiling)`) establishes both the safe count bound and the address region ceiling before any arithmetic is formed. The residual endpoint `whole.base + length` and remaining length `whole.length - length` are proved directly at introduction without any post-hoc caller equality guard (`transition x == 100`). The local place performs an unrelated local store before splitting, confirming that the completed return theorem survives intervening assignments.
2. **Identity and lifetime:** The partitioned address/count relationship remains bound to the exact scalar/view subjects across all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`). The linear `Region` carrier is cleanly retired via `drop(whole)` when `Pieces` are formed, while mathematical content obligations (`Owned::content(old(&whole)) == separate(...)`) remain erased from executable values and ensure disjoint interval preservation.
3. **Composition:** The address/count partition fact composes across all 9 places into the consuming split and subsequent validation of both pieces (`left` and `right`). Each place's positive `<place>/main.omg` vs. its reject twin `<place>/invalid/main.omg` differs only in dropping the construct's partition bound (`length <= whole.length`), cleanly isolating the exact arithmetic requirement under test without caller guard divergence.
4. **Soundness boundaries:** All 9 reject twins (`<place>/invalid/main.omg`) fail closed during typechecking when the partition bound `length <= whole.length` is omitted from `split`'s contract. Residual subtraction `whole.length - requested` underflows unsigned `u64` arithmetic, and proof checking strictly refuses compilation with exit code 1 and diagnostic `error: exact arithmetic in machine split ... may overflow u64`. Unproved partition counts or invalid region bounds cannot escape to runtime.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Provers verify and erase mathematical partition obligations (`IntervalSet` content equality) while address offset arithmetic, count arithmetic, record construction (`Pieces`, `Region`), linear cleanup (`drop`), and state transitions (`done`/`wrong`, `crash Trap`) lower cleanly into canonical `PSITERM\0`. The Terminal Psi interpreter executes every positive place in complete agreement with native execution (`omega run --both` exit 0).
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles uniformly across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Structural records `Region` (`base: addr`, `length: u64`) and `Pieces` (`left: Region`, `right: Region`) evaluate target layout geometry (each `Region` occupies 16 bytes: 8-byte base address + 8-byte length, and `Pieces` occupies 32 bytes). `CallPlan` and `StatePlan` allocate arguments and multi-word aggregate return values (`Pieces`) across ABI registers and stack slots according to target calling conventions.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions (`x86_64` and `arm64`) for 64-bit integer addition (`whole.base + length`), subtraction (`whole.length - length`), register moves, and record field stores. Lowering assigns register homes and disjoint activation stack-frame slots for `Region` and `Pieces` values, inserting standard function entry prologues and epilogues honoring `StatePlan` frame ceilings.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack displacements for `Region` instances and local frames resolve in final binary instruction encodings, basic blocks relax with offset reflow, machine-state footprint evidence validates transitive usage, and direct native PE/ELF/Mach-O emission produces functioning executables in complete agreement with Terminal Psi (`omega run --both` exit 0) across platforms.

### What Remains Open

- Real `Extent` partition: checked custody transfer from parent to children and retirement of the parent under **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS**.
- Aligned request geometry and the counted-residual/tail relationship required by **BUMP-ALLOCATOR-CANARY**.
- Live backing and resident-storage establishment for the allocator itself; this row grants no storage rights.