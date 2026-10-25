# Selected native code layout

The exact selected executable application supplies a nonzero native entry and finite attributed code regions at runtime.

Every place copies that sentence and performs the same
`code_layout<selected<u64, 7>>()` query with the same specialization; only how
the returned `CodeLayout` reaches `inspect` changes. The query is checked
against the nonzero entry, a nonempty attributed region sequence, entry
attribution (`regions[0].base == layout.entry`) and a positive instruction
length. The root `main.omg` probe retains its original body and declares
`Expected exit: 0`. Both `match_join` arms execute, and the const-name place
uses the constant only to select the same application — the metadata remains
runtime data, never a semantic constant.

## Places

| Place | Positive Program | Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| (root) | [main.omg](main.omg) | — | root baseline: direct `code_layout<selected>()` probe |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | layout metadata stored in a local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | layout metadata stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | layout metadata passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | layout metadata returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | layout metadata unified across match branches |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | layout query specialized with named constant parameter |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | layout metadata transitioned to successor machine argument |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | layout metadata forwarded through an intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | layout query executed across target-scoped machine bodies |

## Execution

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64` and `linux_arm64` and declares
`Expected exit: 0`. Run one place directly:

```sh
omega run --both samples/cli/facts/selected_native_code_layout/main.omg
omega run --both samples/cli/facts/selected_native_code_layout/local/main.omg
omega run --both samples/cli/facts/selected_native_code_layout/field/main.omg
omega run --both samples/cli/facts/selected_native_code_layout/parameter/main.omg
omega run --both samples/cli/facts/selected_native_code_layout/call_result/main.omg
omega run --both samples/cli/facts/selected_native_code_layout/match_join/main.omg
omega run --both samples/cli/facts/selected_native_code_layout/const_name/main.omg
omega run --both samples/cli/facts/selected_native_code_layout/other_state/main.omg
omega run --both samples/cli/facts/selected_native_code_layout/other_machine/main.omg
omega run --both samples/cli/facts/selected_native_code_layout/target_body/main.omg
```

`omega run --both` reports the native exit and the Terminal Psi exit;
agreement at zero is the place's runtime oracle. The target-body placement
retains one machine body per hosted target, so checking also validates the
foreign bodies on every host. Pure Terminal interpretation declines this
native query rather than fabricating an address or byte region from an
interpreter ID: under `omega run --both` the interpreter leg reports
`DECLINED (UnsupportedCapability(NativeCodeLayout))` and the native leg
carries the runtime oracle.

### The oracle discriminates

The runtime oracle checks that `layout.entry != 0`, `layout.regions.len > 0`,
`layout.regions[0].base == layout.entry`, and `layout.regions[0].length > 0`.
If the entry address were 0, if regions were empty, if the first region base did
not equal the entry point, or if the code length were 0, `witness()` returns `false`,
and `main()` transitions to `bad()`, triggering `crash Trap;`. Both engines agree on
the resulting failure, verifying that the oracle discriminates against incorrect or
missing metadata.

### Reject twins

Each adjacent `invalid/` twin keeps the same program shape, specialization and
query, and changes only the receiving evaluation stage: the same witness is
consumed by `const OBSERVED: bool = witness()`. Late native metadata cannot
feed a same-product semantic constant, so checking must reject with exit 1:

```
late native code-layout metadata is unavailable to same-product semantic
constant, type, layout, or optimization evaluation
```

An unrelated parse, contract, or lowering error does not count. Check a twin
with `omega --check <place>/invalid/main.omg`.

## Pinned Fixtures

The row is pinned by
`tests/omega/{pass,fail}/facts/selected_native_code_layout_*` fixtures — nine
`*_exit` positives and nine `*_rejected` twins. Every positive records native
exit 0 with Terminal Psi agreement on the hosts that ran it;
`corpus_{both,native}_macos_arm64.txt` carry the nine macOS AArch64 rows.

```sh
python3 tools/corpus_gate.py --filter facts/selected_native_code_layout_through_
python3 tools/corpus_gate.py --both --filter facts/selected_native_code_layout_through_
```

## Production Path

The otherwise uncalled selected specialization must survive into native text.
Metadata comes from final selected instruction spans plus final frame spans,
including branch reflow. The image producer relocates entry and region bases;
no symbol-size subtraction, second compilation, address authority, or lifetime
extension is involved. Runtime copies use the ordinary record/slice ABI after
checking its actual field offsets and element stride.

The focused regressions also cover static type/const/machine/lifetime
forwarding, ordinary declaration shadowing, unavailable/proof-only/incomplete
selections, frame instructions, uncalled boundary-body realization and
portable family progress contracts.

Canonical authority, captured at `7875d3411f26556b6c6711494cd659032e274abe`:
`wiki/spec/language/counts_and_addresses.md`, blob
`348dfb1fa43ef749374a9ff46078b211443fcf6c`:

> `code_layout<SelectedMachine<Arguments>>()` describes the exact selected native
> realization. Like `address_of`, it is a compiler-known core query, not a keyword
> or an invocation of that machine. Its retained use creates the same executable
> demand.

> Regions cover the emitted instruction spans attributed to that realization,
> including its retained entry and any split instruction regions. They do not
> include unrelated intervening bytes, padding, data pools or transitively called
> machines merely because those lie nearby or are needed for execution.

> Lengths are finalized by native lowering, layout and any later size-changing
> realization; bases may remain link/load relocations. This is late artifact
> metadata, not hermetic evaluation of the program being produced. Neither its
> sizes nor its addresses may feed that same program's semantic constants, type
> arguments, layout policies or optimization decisions through a circular query.

## Requirements and Evidence

- **Parent outcome:** `NATIVE-CODE-LAYOUT-QUERIES` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place code-layout query evaluation, metadata preservation, late native artifact reflection, and execution agreement for exact generic machine selections.

Full 8-layer specification and lowering evidence:

- **Establishment:** The native code-layout query `code_layout<selected<u64, 7>>()` creates an executable demand that preserves the otherwise uncalled machine specialization in native code. The query directly establishes the runtime `CodeLayout` property: a nonzero entry address (`layout.entry != 0`), a nonempty sequence of attributed instruction regions (`layout.regions.len > 0`), valid entry attribution (`layout.regions[0].base == layout.entry`), and a strictly positive instruction byte count (`layout.regions[0].length > 0`), without requiring any post-hoc caller equality guard (`transition witness()` directly inspects the layout structure).
- **Identity and lifetime:** The selected machine entry's code-layout fact remains bound to the exact selected realization (`selected<u64, 7>`) across local copy, field projection, parameter passing, call results, match joins, const names, state transitions, forwarding machines, and target bodies. The returned `CodeLayout` record and its `CodeRegion` slice retain exact identity and lifetime through normal runtime value movement; the attributed code spans describe the immutable emitted machine code of that specific realization in the executable image.
- **Composition:** The selected native code-layout fact composes uniformly across all 9 standard places:
  - `local`: layout metadata is bound to a local variable before inspecting.
  - `field`: layout metadata is stored inside an aggregate record field and projected out.
  - `parameter`: layout metadata is passed as an explicit machine parameter to `witness_param`.
  - `call_result`: layout metadata is returned from helper machine `produce_layout`.
  - `match_join`: layout metadata is unified across conditional match branches.
  - `const_name`: a compile-time constant (`SALT: u64 = 7`) specializes the generic parameter of the queried machine.
  - `other_state`: layout metadata is passed as an argument across state machine transition edges.
  - `other_machine`: layout metadata is forwarded through an intermediate machine call chain.
  - `target_body`: layout queries are executed across target-scoped machine bodies for all four hosted targets.
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in attempting to consume `witness()` in a compile-time semantic constant (`const OBSERVED: bool = witness();`), isolating the exact evaluation stage violation.
- **Soundness boundaries:** All 9 reject twins under `<place>/invalid/main.omg` fail closed during semantic analysis. Because native code layout depends on late native realization, frame sizing, and branch relaxation, late native metadata is strictly unavailable during semantic constant evaluation. The compiler rejects every invalid control with `late native code-layout metadata is unavailable to same-product semantic constant, type, layout, or optimization evaluation`, preventing circular layout queries and unsound phase leaks.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Selected native code-layout queries lower into canonical `PSITERM\0` query operations while proof obligations are erased. Terminal Psi models `code_layout` as a native realization query; under `omega run --both`, pure interpretation declines with `DECLINED (UnsupportedCapability(NativeCodeLayout))` while native execution carries the runtime oracle, both agreeing at exit 0.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate the layout geometry of `CodeLayout` (entry pointer of 8 bytes, slice view `regions` with 8-byte base and 8-byte count) and `CodeRegion` (8-byte base and 8-byte length). ABI calling plans (`CallPlan`, `StatePlan`) correctly assign registers and stack slots for passing and returning `CodeLayout` and `bool` flags across function calls.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions (`x86_64` and `arm64`) for pointer comparisons (`layout.entry != 0`, `layout.regions[0].base == layout.entry`), slice length checks (`layout.regions.len > 0`), and unsigned integer comparisons (`layout.regions[0].length > 0`). Backend register allocation assigns register homes and disjoint stack activation frames, inserting standard entry prologues and multi-return restoration epilogues honoring the `StatePlan` frame ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** The image generator attributes final machine code spans to `selected<u64, 7>` after branch relaxation and offset reflow. Symbolic stack displacements and entry relocations resolve into concrete machine addresses. Direct PE, ELF, and Mach-O image emission produces valid native binaries that agree with Terminal Psi (`omega run --both` exit 0) across host targets (`windows_x86_64` and `macos_arm64`).

Remaining open under NATIVE-CODE-LAYOUT-QUERIES: real split/shared native production with one placed fragment and one frame record per application, multiple-fragment refusal, missing or ambiguous native realization refusal, and direct `code_layout` queries of bodyless target-family heads.
