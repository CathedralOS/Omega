# Match primitive local origin

A match selects a primitive local, the join records that local as the origin,
and the consumer reads the recorded value.

`choose()` has `ensures result == true`, so the match's selected arm is the
`true` arm, and the join records that arm's origin: the primitive local
`chosen` (initialized to `17`). The consumer call `read_origin(joined)` has
`requires value == 17`, discharged directly from the join's recorded origin —
never from a post-hoc caller equality guard. `observed == 17` is the runtime
oracle only. Every place copies that sentence and transports the fact through
its own name; only the carrier of the selected primitive changes.

The reject twin swaps the two arms (the match selects the other local, value
`5`), so the join records the wrong origin and the consumer's `requires value
== 17` can no longer be proved; it is rejected at compile time with `cannot
prove requires contract for call read_origin`.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) with [`build.omg`](build.omg) | — | canonical root witness |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the selected primitive is a local `let` |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the selected primitive is a record field (`Seed.chosen`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the selected primitive arrives as a parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `select()` returns the joined primitive (`ensures result == 17`) |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | the selected primitive itself arrives from a match join |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | the primitives are named constants (`CHOSEN`, `SPARE`) |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives the primitives and consumes the join |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine supplies the primitives |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies performs the sequence |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four
hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`)
and declares `Expected exit: 0`. `omega run --both` reports Terminal Psi
interpreter and native execution agreement (`native exit 0` and `interp exit: 0`):

```sh
target/debug/omega run --both samples/cli/facts/match_primitive_local_origin/main.omg
target/debug/omega run --both samples/cli/facts/match_primitive_local_origin/local/main.omg
target/debug/omega run --both samples/cli/facts/match_primitive_local_origin/field/main.omg
target/debug/omega run --both samples/cli/facts/match_primitive_local_origin/parameter/main.omg
target/debug/omega run --both samples/cli/facts/match_primitive_local_origin/call_result/main.omg
target/debug/omega run --both samples/cli/facts/match_primitive_local_origin/match_join/main.omg
target/debug/omega run --both samples/cli/facts/match_primitive_local_origin/const_name/main.omg
target/debug/omega run --both samples/cli/facts/match_primitive_local_origin/other_state/main.omg
target/debug/omega run --both samples/cli/facts/match_primitive_local_origin/other_machine/main.omg
target/debug/omega run --both samples/cli/facts/match_primitive_local_origin/target_body/main.omg
python tools/sample_native_gate.py --facts --filter cli/facts/match_primitive_local_origin
python3 tools/corpus_gate.py --filter facts/match_primitive_local_origin
```

### The oracle discriminates

Exit `0` is strictly discriminative. Corrupting the selected primitive's value
(`let chosen: i64 = 17` to `16`) keeps the join's origin recording intact but
now the consumer's `requires value == 17` cannot be discharged — the same
compile-time refusal the twins pin. Corrupting only the runtime comparison
(`observed == 17` to `observed == 5`) instead lets the call compile and makes
the transition false, so `wrong()` executes `crash Trap;` and both engines
abort non-zero: `native signal 5` and `interp crash: Trap (host presentation
agrees)`, exit 255.

## Reject Twins

Each place has an `invalid/` reject twin that swaps only the match arms; the
rest of the program is identical to its positive (verified with `git diff`).
Compile-time verification fails closed with:

| Place | Twin | Pinned Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `cannot prove requires contract for call read_origin` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `cannot prove requires contract for call read_origin` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `cannot prove requires contract for call read_origin` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `cannot prove ensures contract for exit from select` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `cannot prove requires contract for call read_origin` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `cannot prove requires contract for call read_origin` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `cannot prove requires contract for call read_origin` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `cannot prove requires contract for call read_origin` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `cannot prove requires contract for call read_origin` |

## Pinned Fixtures

The row is pinned by `tests/omega/{pass,fail}/facts/match_primitive_local_origin_through_*`
fixtures; every positive records native exit 0 and Terminal agreement, and every
twin records its rejection with the expected fragment. The nine pass fixtures
are byte-identical copies of their place programs, and the nine fail fixtures
are byte-identical copies of their twins, so sample and fixture cannot drift.

## Requirements and Evidence

- **Parent outcome:** `MATCH-SELECTIVE-LOWERING` (`TASKS.md`), whose remaining
  borrowed-join acceptance names exactly this witness: "Still missing: …
  primitive-local origins". The exact requirement this row establishes: a match
  join records the selected primitive local as the origin of its result, and a
  consumer operation reads the recorded value through that origin — carried
  through all nine transport places to native execution.

### Specification and Lowering Layers

1. **Establishment:** The origin fact is established where it originates: the
   single evaluated subject `choose()` fixes the selected arm (`ensures result
   == true`), and each arm names a primitive whose value is known at
   definition (`let chosen: i64 = 17`, a field, a parameter, a call result with
   `ensures result == 17`, a join, or a `const`). The consumer call
   `read_origin(joined)` proves `requires value == 17` directly from the
   recorded origin; no post-hoc caller equality guard is involved.
2. **Identity and lifetime:** The join records the identity of the exact
   selected local/field/parameter version; the twins show that swapping the
   selected arm changes the recorded origin and breaks the consumer's
   obligation. No intervening mutation occurs between the match and the
   consumer read.
3. **Composition:** The fact composes through local, field, parameter, call
   result, match join, const name, successor machine, forwarding machine, and
   target body; `<place>/main.omg` vs `<place>/invalid/main.omg` differ only in
   which arm is selected (the construct's own value).
4. **Soundness boundaries:** All nine twins fail closed during compile-time
   proof checking (`cannot prove requires contract for call read_origin`,
   or the `select` ensures variant) when the join records the `5`-valued
   origin; a wrong origin never discharges `value == 17`. The positives show
   the same obligation discharges exactly when the recorded origin has value
   17.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The match, its arm
   selection, the recorded origin, and the consumer call lower without loss
   into canonical target-neutral Terminal Psi (`PSITERM\0`); the Terminal Psi
   interpreter executes root and all nine positive places in complete
   agreement with native execution (`omega run --both`).
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):**
   Target realization joins Build companion rows (`ProgramEntry`,
   `ProviderPlan`, `CompilerBuiltinExecution`) across all four hosted targets;
   the primitive carrier's `size_of`/`align_of` geometry and the
   `CallPlan`/`StatePlan` register placements for `choose` and `read_origin`
   are evaluated per target.
7. **Instruction selection, register homes, and activation frame realization
   (`Omega 03–06`):** Backend lowering selects target ISA instructions for the
   arm selection, the primitive moves, and the consumer call; register homes
   and disjoint frame slots are assigned under WCSU/bounded activation claims
   with entry prologues and multi-return epilogues.
8. **Form encoding, branch relaxation, relocations, and native image emission
   (`Omega 07–09` + emission):** Final encoding resolves symbolic displacements,
   lays out blocks with short/near branch relaxation, binds relocations, and
   emits direct PE/ELF/Mach-O images that execute with exit code 0 and agree
   with Terminal Psi (`omega run --both`) on this host.

Remaining open scope: the parent item's broader borrowed-join acceptance is
carried by the other rows of the same family; this row closes only the
primitive-local origin recorded on a match join and read by a consumer.
