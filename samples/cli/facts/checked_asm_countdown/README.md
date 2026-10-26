# Checked assembly countdown

The checked seed-2 mov/sub/jnz CFG falls through with eax == 0 and stores output == 0.

Owning requirement: **CHECKED-ASM-LABEL-CONTRACTS**, AArch64 countdown and
register-view parity. Every place copies that sentence, keeps seed 2 and output
0, and transports the countdown's result into the same consuming obligation.
The x86 bodies use `eax`/`sub`/`jnz`. AArch64 realizes the same u32 operation as
`subs w1, w0, 1; mov w0, w1; b.ne loop`: the destination differs from the left
input, and the value move preserves the arithmetic zero condition. Output starts
at 99, so skipping the assembly's fallthrough store cannot satisfy the oracle.

Every positive, including the root, binds all four hosted targets and retains
the historical `macos_x86_64` binding. Each declares `Expected exit: 0`. Label
`done` requires the counter to be 0 and proves `output == 0` after the store;
the selected countdown body then proves its ordinary `result == 0` contract.
The consuming operation receives that fact before any runtime oracle branch.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| root | [program](main.omg) | [wrong edge](invalid/main.omg) | canonical countdown |
| local | [program](local/main.omg) | [wrong edge](local/invalid/main.omg) | local result |
| field | [program](field/main.omg) | [wrong edge](field/invalid/main.omg) | record field |
| parameter | [program](parameter/main.omg) | [wrong edge](parameter/invalid/main.omg) | parameter requiring 0 |
| call result | [program](call_result/main.omg) | [wrong edge](call_result/invalid/main.omg) | published result contract |
| match join | [program](match_join/main.omg) | [wrong edge](match_join/invalid/main.omg) | both arms execute |
| const name | [program](const_name/main.omg) | [wrong edge](const_name/invalid/main.omg) | named seed 2 |
| nested machine | [program](other_state/main.omg) | [wrong edge](other_state/invalid/main.omg) | explicit result argument |
| another machine | [program](other_machine/main.omg) | [wrong edge](other_machine/invalid/main.omg) | explicit forwarding call |
| another target body | [program](target_body/main.omg) | [wrong edge](target_body/invalid/main.omg) | selected forwarding body |

Each twin changes only the fallthrough label's own requirement from counter 0
to counter 1, in every target body. The instruction sequence, bound, consuming
operation and runtime oracle are unchanged. The nine places and twins are pinned
as `tests/omega/{pass,fail}/facts/checked_asm_countdown_through_*`.
The positive fixtures also request all four hosted target interpretations from
one unchanged retained Terminal artifact using `expected_terminal_targets.txt`.

## Baseline and evidence

The optimized fixed baseline `16b381462327a3f7d1eb8c7218bdb81114a8a10a`
rejects the frozen four-target seed-2 countdown with `unknown asm instruction
subs`. The authored source and exit-0 oracle were frozen before compiler edits;
qualification receipt `2fedcb8e65e544999b3e4343ca34b775` records exit 200 with
no invalidations. Earlier x86-only programs and observations remain in that
baseline's source and the qualification evidence. They are distinct from fresh
observations of the target-parity implementation.

The execution ledger records producer inputs, optimized executable digests,
actual native and Terminal outcomes, target-installation outcomes and reject
diagnostics. Compiler exit 0 or compile-only target publication alone does not
establish execution. Matching-host native execution on Windows or macOS and
native AArch64 execution remain unverified until their engines observe them.

## Requirements and remaining acceptance

- Establishment: seed 2 justifies the initial positive counter. Modular u32
  subtraction establishes the result's zero condition; each taken/backedge and
  fallthrough proves its own arrival requirement. Goals supply no assumptions.
- Identity and lifetime: `subs` records independent left, right and result
  identities. The uninitialized destination is written, not read; source reads
  still owe initialization. `mov` carries the new counter without changing ZF.
  Separate assembly bodies share no implicit scratch state.
- Composition: `done` proves the output store and the ordinary return guarantee.
  Field extraction, calls, both match arms and explicit nested-machine arguments
  carry the result. The parameter consumer requires 0 directly.
- Soundness: `arm_subs_*` controls pin uninitialized inputs, literal left input,
  mixed ISA registers, unencodable immediates, unsupported wide aliases and
  missing or invented clobbers. Existing assembly rejection controls remain
  regression inputs. No access or hardware authority is granted.
- Psi 05–07: known arithmetic erases to ordinary checked modular scalar
  computations and explicit ZF transitions. The catalog's target demand and
  exact input/result identities survive extraction and Terminal lowering.
- Omega 00–02: the selected target body and its ProgramEntry join the ordinary
  provider/calling plans. Target installations use one retained Terminal artifact
  with actual target provisions and no source reopening.
- Omega 03–06: ordinary u32 arithmetic and call realization own instruction
  selection, register homes, activation storage and return values. AArch64's
  three-operand input form is established before these shared consumers run.
- Omega 07–09 and emission: native image execution and Terminal agreement retain
  the exit-0 oracle. Target compilation and interpreted target installations are
  reported separately from matching-host native execution.

This row does not close **CHECKED-ASM-LABEL-CONTRACTS**. Other flag families,
linear hardware-effect retirement, transitive catalog plans, privileged port
settlement, supplied-environment interpretation and interrupt/processor canaries
remain open. Fresh focused observations and required independent review are
retained with the qualification rather than replacing the original failure.
