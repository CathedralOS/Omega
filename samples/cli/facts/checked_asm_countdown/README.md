# Checked assembly countdown

The checked seed-2 mov/sub/jnz CFG falls through with `eax == 0` and stores `output == 0`.

Every place copies that sentence and performs the same countdown: initialize the
assembly scratch view `eax` from a seed that is 2, loop `sub eax, 1` on the
declared label `loop requires eax > 0`, fall through the not-taken edge of
`jnz loop` to `done requires eax == 0`, and store `eax` into `output`. The
initial `output` is 99, so only the assembly's own fallthrough store can satisfy
the enclosing transition `output == 0`. Only the transport of the seed changes.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) with [`build.omg`](build.omg) | — | canonical root countdown over the seed literal |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the seed is a local copy |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the seed is a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the seed arrives as an explicit parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a contracted call result supplies the seed |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms execute and supply the seed |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant supplies the seed |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state receives the seed |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine delegates the countdown |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | one target-scoped `countdown` body per realized target |

Every positive place (and the root `main.omg`) declares `Expected exit: 0`. Each
reject twin differs from its positive in exactly one contract: the fallthrough
label's `done requires eax == 0` becomes `done requires eax == 1`. Nothing else
differs — verified per place with `diff <place>/main.omg <place>/invalid/main.omg`.

## Target scope of this fact

The construct is the checked 32-bit GPR cohort (`mov`/`sub`/`jnz` over the
modeled scratch views `eax`/`zf`). Its instruction catalog contracts are
`x86_64`: `sub` decodes as `AsmInstructionShape::Subtract` and `jnz` as
`JumpNonZero`, both catalogued `AsmTargetApplicability::X86_64` in
`omega-rust/psi/foundation/language-core/src/inline_assembly/mod.rs`, and the
modeled scratch viewport itself (`scratch_registers.rs`) is the x86 GPR/flags
set. `AsmTargetApplicability::X86_64` accepts `linux_x86_64`,
`windows_x86_64`, `macos_x86_64` and `uefi_x86_64`; it does not accept
`linux_arm64` or `macos_arm64`, and no AArch64 scratch viewport exists yet.

Measured on base `591336bdc9e0` before this change, the root and all nine places
bound `ProgramEntry` for `macos_arm64` and `linux_arm64` and refused:

```text
error: native artifact provider supply failed: reachable machine MachineId(1)
       has incompatible assembly catalog target requirement for `macos_arm64`
```

and `target_body` additionally refused:

```text
error: machine `main` state `entry` calls `countdown`, which is declared only
       for targets `linux_x86_64`, `windows_x86_64`. This build realizes
       `macos_arm64`, so those bodies are inert siblings and the call has no
       implementation; scope the caller to one of those targets, or declare
       `countdown` for the target being built.
```

Both refusals are correct for this cohort: a machine body that carries an
x86_64 catalog demand cannot be the realization of an arm64 entry, and a
target-scoped callee with no arm64 body has no implementation. The defect was in
the fact source, which declared arm64 entry bindings for a body the catalog
admits only for x86_64. Those bindings are removed; each place now binds
`linux_x86_64`, `windows_x86_64` and `macos_x86_64`, the three x86_64 targets
this construct authors. The fact sentence, the seed, the instruction sequence,
the label contracts and the consuming transition are unchanged in every place —
this is an explicit, documented scope, not a narrowed fact. Realizing this
cohort for arm64 needs an AArch64 scratch viewport and arm64 catalog contracts
for its arithmetic/branch shapes, which the parent item keeps open as
*other ISA/register views*; the sibling `checked_asm_flag_snapshot` fact pins its
x86 scope the same way.

## Execution

Every positive place is an application that exits 0 in both Terminal Psi and
native execution on an x86_64 host:

```sh
omega run --both samples/cli/facts/checked_asm_countdown/main.omg
omega run --both samples/cli/facts/checked_asm_countdown/local/main.omg
omega run --both samples/cli/facts/checked_asm_countdown/field/main.omg
omega run --both samples/cli/facts/checked_asm_countdown/parameter/main.omg
omega run --both samples/cli/facts/checked_asm_countdown/call_result/main.omg
omega run --both samples/cli/facts/checked_asm_countdown/match_join/main.omg
omega run --both samples/cli/facts/checked_asm_countdown/const_name/main.omg
omega run --both samples/cli/facts/checked_asm_countdown/other_state/main.omg
omega run --both samples/cli/facts/checked_asm_countdown/other_machine/main.omg
omega run --both samples/cli/facts/checked_asm_countdown/target_body/main.omg
```

Measured observations (producer built at the recording revision, `opt-level = 1`):

| Leg | Engine | Target | Result |
| --- | --- | --- | --- |
| root + nine places | Intel macOS host, real process launch | `macos_x86_64` (host) | `native exit 0` and `interp exit: 0`, 10/10 |
| root | `linux/amd64` container (docker) on the Intel macOS host | `linux_x86_64` | ELF x86-64 built by `omega --build-dir … --target linux_x86_64`; container exit 0 |
| root + nine places | Terminal Psi interpreter | `macos_x86_64` | `interp exit: 0` |
| Windows | not executable on this host | `windows_x86_64` | bound and source-checked; no native observation claimed |
| arm64 | not realizable for this cohort | `macos_arm64`, `linux_arm64` | refused by the catalog demand (above) |

The Linux ELF is a real native program observation; the macOS arm64 and Linux
arm64 legs are deliberately not claimed because this host cannot execute them
and the cohort has no arm64 realization.

### The oracle discriminates

The recorded 0 exit is discriminating at runtime. Corrupting only the assembly's
own fallthrough store — `mov output, eax` becomes `mov output, output`, which
still proves `done requires eax == 0` — leaves `output` at its initial 99, so
the enclosing `transition output == 0` routes to `bad()` and `crash Trap;`:

```text
omega run --both <probe>/main.omg    # exit 255
native signal 4
interp crash: Trap (host presentation agrees)
```

Both engines refuse; neither silently accepts a skipped store.

The nine reject twins are discriminated at checking time: at the conditional
fallthrough from `jnz loop`, ZF is set (`eax == 0`), which cannot satisfy
`done requires eax == 1`, so each twin fails closed during checking with

```text
cannot prove requires contract for call done
in state `loop`
```

and exit status 1. Measured 9/9.

### Acceptance gate

```sh
python3 tools/sample_native_gate.py --cli target/debug/omega --facts \
    --filter cli/facts/checked_asm_countdown --jobs 2
```

Measured on the Intel macOS host: `10/10 samples meet their documented oracle on
macos_x86_64 (0 unscored: no oracle or not host-runnable)` and `9/9
intended-invalid controls match their authored rejection` in 455s wall.

## Pinned fixtures

The row is pinned under `tests/omega/{pass,fail}/facts/checked_asm_countdown_*`:
nine `..._exit` positives with `expected_native_exit.txt` (0) and nine
`..._wrong_fallthrough` twins with `expected.txt`. The positives bind the same
three x86_64 targets as the samples. `tests/omega/corpus_native_macos_x86_64.txt`
records their native rows (`built … exit:0`) measured on the Intel macOS host;
`tests/omega/corpus_outcomes.txt` records the check rows and the nine rejections.

## Requirements and Evidence

- **Parent outcome:** `CHECKED-ASM-LABEL-CONTRACTS` (`TASKS.md`), split of
  `STATIC-MACHINE-MIGRATION`, under
  [blocks and assertions](../../../../wiki/spec/language/assembly.md#blocks-and-assertions).
- **Requirement established:** the checked `mov`/`sub`/`jnz` CFG is a basic-block
  graph whose label contracts are ordinary propositions; the seed-2 backedge
  transports the exact immutable register/ZF version to the fallthrough edge, and
  that edge's `requires eax == 0` is proved before `output == 0` is consumed, for
  all nine transport places, with Terminal Psi and real native execution in
  agreement on the fact's x86_64 targets and no post-hoc caller equality guard.

- **Obligations accounted for:**
  - **Establishment:** the entry label's `requires` is proved at the enclosing
    control point (`mov eax, seed` with `seed == 2`), the backedge proves
    `loop requires eax > 0` from `eax - 1`, and the not-taken `jnz` edge proves
    `done requires eax == 0` from the modeled ZF. The post-assembly bound is
    established by the assembly block itself; no caller re-checks `output`.
  - **Identity and lifetime:** the fact names one exact value version — the
    scratch view `eax` after the final `sub` — and one exact storage version,
    `output`. Snapshot versions of the seed are retained across the local copy,
    field projection, parameter, call result, match join, const name, state
    arrival and forwarding machine; only the store at `done` changes `output`,
    and the invalid twins show a changed fallthrough contract invalidates the
    fact rather than being ignored.
  - **Composition:** every place substitutes the same exact seed (2) into the
    same instruction sequence, so the consuming transition `output == 0` reads a
    fact proved on the fallthrough edge of the countdown itself. Place and twin
    differ only in that one label contract, verified by `diff`.
  - **Soundness boundaries:** the CFG proves ownership, storage and termination
    obligations like any other checked body; the loop's backedge is a proved
    edge, the modeled flags are block-local state (no host register is named as
    live), and a fallthrough contract that the modeled ZF cannot satisfy rejects
    at checking (9/9 twins) instead of escaping to native code.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** the labeled CFG lowers to
    ordinary Terminal Psi control flow — each label becomes a checked state with
    its contract as an ordinary call precondition, and the modeled register/flag
    versions become ordinary values carried through those edges. Interpretation
    of the lowered program agrees with native execution (`interp exit: 0` beside
    `native exit 0`) for all ten positive places.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):**
    each place settles `ProgramEntry` for the x86_64 targets it authors and the
    realized host target selection is exact; the `u32` carriers and the seed
    record use their ordinary layout geometry, and calls/returns place arguments
    and results through the host `CallPlan`/`StatePlan`.
  - **Instruction selection, register homes, and activation frame realization
    (`Omega 03–06`):** after normalization the countdown is ordinary scalar
    subtraction, comparison and transitions over assigned register/stack homes,
    with entry prologue and return epilogue honoring the state plan. The
    machine's retained `x86_64` assembly catalog demand is enforced at
    realization, which is why the arm64 bindings refused (measured above).
  - **Form encoding, branch relaxation, relocations, and native image emission
    (`Omega 07–09` + emission):** the countdown's branches and blocks encode into
    x86-64 Mach-O and ELF images with symbolic frame displacements resolved; the
    emitted Mach-O runs on this host (`native exit 0`) and the emitted ELF x86-64
    executable runs to exit 0 in a `linux/amd64` container.

- **What remains open (parent item, not this row):** an AArch64 scratch
  viewport and arm64 catalog contracts for the cohort (the arm64 realization of
  this fact), which is the parent's *other ISA/register views* frontier; plus
  the parent's other open requirements (unmodeled flag production, indexed
  memory/address composition, hardware flag restoration, ranking-witness
  transport for promised termination, checked hardware-effect retirement of the
  exact linear obligation). This row establishes the x86_64 realization and
  transport of the fact; it does not close those.