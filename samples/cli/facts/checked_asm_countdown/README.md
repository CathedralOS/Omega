# Checked assembly countdown

The checked seed-2 mov/sub/jnz CFG falls through with eax == 0 and stores output == 0.

Owning requirements: **BUILD-OWNED-TARGET-SELECTION**, ordinary named ISA
implementations with explicit target-keyed Build rows, and
**CHECKED-ASM-LABEL-CONTRACTS**, AArch64 countdown and register-view parity. Every place copies that sentence, keeps seed 2 and output
0, and transports the countdown's result into the same consuming obligation.
`X86Countdown` and `ArmCountdown` are ordinary named implementations of the
portable `Countdown` requirement. Every Build file supplies five explicit catalog
target rows; no countdown declaration has a source target prefix. The x86 body
uses `eax`/`sub`/`jnz`. AArch64 realizes the same u32 operation as
`subs w1, w0, 1; mov w0, w1; b.ne loop`: the destination differs from the left
input, and the value move preserves the arithmetic zero condition. Output starts
at 99, so skipping the assembly's fallthrough store cannot satisfy the oracle.

Every positive, including the root, binds all four hosted targets and retains
the historical `macos_x86_64` binding. Each declares `Expected exit: 0`. Label
`done` requires the counter to be 0 and proves `output == 0` after the store;
the selected countdown body then proves its ordinary `result == 0` contract.
The consuming operation receives that fact before any runtime oracle branch.
The match place also passes its original Boolean to an arm guard: selecting the
wrong arm traps before the unchanged countdown. Both Boolean inputs execute.
The target-body place has separate Build-selected forwarding implementations:
x86 entries expect tag 17 and ARM entries expect tag 29. A wrong selected body
traps before calling the portable countdown requirement.
Transported outputs enter `observe_zero` with `requires value == 0`; parameter
and nested-machine consumers owe the same precondition. The match join publishes
its zero guarantee and both selected results enter that consumer. An equality
branch inside the consumer is only the runtime oracle, after arrival is proved.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| ordinary provider rung | [program](ordinary_provider/main.omg) | [wrong edge](ordinary_provider/invalid/main.omg) | frozen ordinary implementation migration |
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
to counter 1, in both ordinary ISA implementations. The instruction sequence, bound, consuming
operation and runtime oracle are unchanged. The nine places and twins are pinned
as `tests/omega/{pass,fail}/facts/checked_asm_countdown_through_*`.
The positive fixtures also request all four hosted target interpretations from
one unchanged retained Terminal artifact using `expected_terminal_targets.txt`.
The target-body pin additionally requests the existing `compiled-application`
cohort mode, so each installation receives its own actual admitted ProgramEntry
root rather than the artifact-only default entry; the semantic artifact remains
unchanged across that cohort.

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
establish execution. Native execution on Windows and Linux remains unverified; fresh macOS ARM64
and Rosetta x86_64 observations are reported separately below.

The optimized fixed baseline `27aa307651127ca13dd9ac08a2f1a687a2dd5bb0`
wrongly rejects the frozen ordinary-provider rung and every existing countdown
place: the extracted helper cannot prove its `output == 0` normal-exit contract.
The label store and its assertion are already checked. The shared current-value
exit prover excluded both Unit completion and scalar referent parameters, so it
could not consume that completed store through the exact exit origin.

Producer `1273f57db0dfc09fa83edec23dfce9cb698b1abc` supplies the repair:
normal Unit completion has no return operand, and stable scalar referents use
the same store/call-frame invalidation as owned scalar values. Unreadable
write-only referents have no initial observation. No result value or loan is
invented. The nine places, root and frozen ordinary rung were observed with
native exit 0 and Terminal exit 0 on `macos_arm64`; every wrong-edge twin still
rejects for the incoming `done` label obligation. The actual source revision
for the fresh complete row is `1273f57db0dfc09fa83edec23dfce9cb698b1abc`;
its compiler and bundled-library inputs equal the producer's inputs. The
selected-body pin also executes natively and in Terminal on `macos_x86_64`
under Rosetta. Public CLI compilations for Mac ARM64 and x86_64 retain identical
`program.oterm` bytes; separate matching-host image launches return 0.
Explicit `--target` CLI compilation itself is not counted as execution.

The retained build, raw engine streams, source/oracle hashes and product hashes
are in the target-retirement qualification evidence. The focused ordinary
scalar-loan controls also pin explicit and implicit Unit completion, a later
write invalidating the guarantee, and a missing store. These are regression
controls for the shared proof operation, not another countdown fact row.

After resolving the actual main conflict at `75ea436b46adcb9c2e9889e32c4de8c6264768a4`,
producer `7faf25e51b15080d3d7b0f3c5dc7823cab7882f3` repeats the complete
11-positive/11-reject public row, 35 scoped Source controls, ten canonical
macOS ARM64 both-engine runs and the selected-body Rosetta both-engine run.
The original occurrence owner from main composes with optional Unit return
operands. Public ARM64/x86_64 compilations retain identical Terminal bytes and
the x86_64 image also exits 0 under Rosetta. Every scoped Source semantic
observation and all 32 owned golden observations match the earlier complete
row; their original recorded revisions remain intact. These fresh integration
receipts are distinct from the original baseline and producer-1273 records.

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
- Omega 00–02: explicit target-keyed named implementation rows and ProgramEntry join the ordinary
  provider/calling plans. Target installations use one retained Terminal artifact
  with actual target provisions and no source reopening.
- Omega 03–06: ordinary u32 arithmetic and call realization own instruction
  selection, register homes, activation storage and return values. AArch64's
  three-operand input form is established before these shared consumers run.
- Omega 07–09 and emission: native image execution and Terminal agreement retain
  the exit-0 oracle. Target compilation and interpreted target installations are
  reported separately from matching-host native execution.

This row does not close **BUILD-OWNED-TARGET-SELECTION**: remaining library,
layout-policy and product migrations, prefix rejection, and removal of implicit
family parser/IR/codec/dispatch/install machinery stay open. It also does not
close **CHECKED-ASM-LABEL-CONTRACTS**. Other flag families,
linear hardware-effect retirement, transitive catalog plans, privileged port
settlement, supplied-environment interpretation and interrupt/processor canaries
remain open. Fresh focused observations and required independent review are
retained with the qualification rather than replacing the original failure.
