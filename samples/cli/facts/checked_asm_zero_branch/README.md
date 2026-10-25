# Checked assembly zero branch

The checked seed-2 mov/sub/jz CFG takes its zero edge with eax == 0 and stores output == 0.

Owning requirement: **CHECKED-ASM-LABEL-CONTRACTS**, conditional label edges
against the exact modeled register/ZF version. The first subtraction falls
through `jz done` to `repeat requires eax > 0`; its jump owes the
`loop requires eax > 0` backedge. The second subtraction takes the zero edge
to `done requires eax == 0`. Output starts at 99, so only the final assembly
store can satisfy the exit-0 oracle. No caller guard establishes the label fact.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [false zero-edge fact](local/invalid/main.omg) |
| field | [source](field/main.omg) | [false zero-edge fact](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [false zero-edge fact](parameter/invalid/main.omg) |
| call result | [source](call_result/main.omg) | [false zero-edge fact](call_result/invalid/main.omg) |
| match join | [source](match_join/main.omg) | [false zero-edge fact](match_join/invalid/main.omg) |
| const name | [source](const_name/main.omg) | [false zero-edge fact](const_name/invalid/main.omg) |
| a nested machine | [source](other_state/main.omg) | [false zero-edge fact](other_state/invalid/main.omg) |
| another machine | [source](other_machine/main.omg) | [false zero-edge fact](other_machine/invalid/main.omg) |
| another target body | [source](target_body/main.omg) | [false zero-edge fact](target_body/invalid/main.omg) |

Each twin changes only `done requires eax == 0` to `eax == 1`.
Additional controls pin [false fallthrough/backedge facts](controls/false_backedge/main.omg),
[uninitialized ZF](controls/uninitialized_flag/main.omg),
[omitted clobbers](controls/missing_clobber/main.omg), and
[invented clobbers](controls/invented_clobber/main.omg).
Matching fixtures live under `tests/omega/{pass,fail}/facts/checked_asm_zero_branch_*`.

## Requirement coverage

- **Establishment:** the seed and SUB semantics establish the register and ZF
  facts. Both destination labels remain ordinary proof goals.
- **Identity and lifetime:** the branch consumes the latest immutable ZF version
  and changes no register, flag, or storage. Separate bodies retain no scratch
  input; the uninitialized control checks this boundary.
- **Composition:** exact captured values and versions traverse the same shared
  state edges through all nine places. Both successors and the backedge owe
  their label contracts. The seed and consuming store remain unchanged.
- **Soundness boundaries:** clobbers remain the catalog's exact union; invented
  and omitted changes reject. The branch acquires no authority. This loop makes
  no termination promise; existing ranked-decrease transport stays separate.
- **Psi 05–07:** the shared control model normalizes ZF polarity to ordinary
  checked Boolean guards and transitions. Terminal execution consumes those
  operations, without reparsing assembly text.
- **Omega 00–02:** hosted ProgramEntry selects the x86 body; scalar/record
  carriers use the existing layout and calling plans.
- **Omega 03–06:** ordinary scalar control lowering realizes the normalized graph
  using existing register and stack homes. Storage, clobbers and state ceilings
  retain their existing contracts.
- **Omega 07–09:** normal x86 branch encoding and ELF emission realize the graph;
  native/Terminal agreement checks the authored observable result.

## Scope and evidence

The current baseline is `d6cf554fd1cbf20551983d29bae26f5cf3b81664`.
A fresh optimized baseline rejects `local/main.omg:12:3` with
“asm instruction `jz` creates a hidden control exit”. The source and oracle
were frozen before repair.

The executable reproduction and qualification records retain the frozen source
and oracle, producer identities, actual results and exact refusal diagnostics.
Public acceptance uses normal package preparation followed by
`omega run --both <project>/main.omg`; invalid controls use `omega --check`.
Scratch copies preserve the authored source bytes and keep generated files of
nested projects outside each package's source closure.

This row is x86-64, following the catalog-compatible scope of
`checked_asm_countdown`. Sources bind Linux, Windows and macOS x86-64;
execution is measured only on the available Linux x86-64 host. It establishes
nine-place x86 transport, not four-hosted-target closure or ARM execution.
The parent task remains open for other ISA/register views, unmodeled flag
production and exact linear hardware-effect retirement. Existing flag restore,
ranked-decrease and mutable-input snapshot work remains in place.
