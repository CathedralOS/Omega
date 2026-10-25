# Checked assembly arm load

An AArch64 `ldr` destination, place reads the typed place, and the following Omega use reads that recorded value.

Owning item: **CHECKED-ASM-LABEL-CONTRACTS** (`TASKS.md`). Exact requirement: the instruction catalog admits `ldr` as a `MemoryTransfer` Load on `linux_arm64` and `macos_arm64`, so the memory operand is an ordinary Omega place expression and the instruction lowers to the same checked place read an ordinary assignment performs; the destination records that value and the following Omega use reads that recorded value. Every positive place binds `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares `Expected exit: 0`.

Host boundary: this row is measured on Apple Silicon (`macos_arm64`). Unlike the Windows host that first pinned the local place, the native leg of `omega run --both` executes the row's `macos_arm64` program here; `linux_arm64`, `linux_x86_64` and `windows_x86_64` are bound and checked, and no Linux or x86 image is executed on this host. The earlier note ("Native arm64 execution is not available here") described that Windows host, not this one.

## Places

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | the same fact at the row root |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | `ldr` reads a local place; the following use reads the recorded value |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | the read place is a record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | the read place is a machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | the read place starts at a call result |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | the read place converges through a match join |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | the read place is indexed by a named constant |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | the read place arrives as a state parameter |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | the read place crosses another machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | each hosted target's scoped body carries the same `ldr` |

Run each place with `omega run --both <place>/main.omg` (stdin closed). Every
place's reject twin writes the same `ldr` with its memory operand bracketed
(`ldr loaded, [<place>]`). A bracketed operand is not an Omega place
expression, so it refuses before the place-bearing shape applies:

```text
asm instruction `ldr` operand uses bracketed `[...]` memory addressing: spell
authorized access as a typed Omega place such as `self.buffer[index]` — the
place carries the provenance/permission contract raw addresses cannot express
```

`git diff <place>/main.omg <place>/invalid/main.omg` shows only that operand
spelling (and in `target_body`, that spelling once per target body).

## Pinned fixtures

Each place is pinned as a pass fixture (a byte copy of the place with both
execution oracles) and a reject twin fixture (a byte copy of the twin's program
with its authored fragment):

| Place | Positive fixture | Reject twin fixture |
| --- | --- | --- |
| local | [pass/facts/checked_asm_arm_load_through_local_exit](../../../../tests/omega/pass/facts/checked_asm_arm_load_through_local_exit/main.omg) | [fail/facts/checked_asm_arm_load_through_local_rejected](../../../../tests/omega/fail/facts/checked_asm_arm_load_through_local_rejected/main.omg) |
| field | [pass/facts/checked_asm_arm_load_through_field_exit](../../../../tests/omega/pass/facts/checked_asm_arm_load_through_field_exit/main.omg) | [fail/facts/checked_asm_arm_load_through_field_rejected](../../../../tests/omega/fail/facts/checked_asm_arm_load_through_field_rejected/main.omg) |
| parameter | [pass/facts/checked_asm_arm_load_through_parameter_exit](../../../../tests/omega/pass/facts/checked_asm_arm_load_through_parameter_exit/main.omg) | [fail/facts/checked_asm_arm_load_through_parameter_rejected](../../../../tests/omega/fail/facts/checked_asm_arm_load_through_parameter_rejected/main.omg) |
| call result | [pass/facts/checked_asm_arm_load_through_call_result_exit](../../../../tests/omega/pass/facts/checked_asm_arm_load_through_call_result_exit/main.omg) | [fail/facts/checked_asm_arm_load_through_call_result_rejected](../../../../tests/omega/fail/facts/checked_asm_arm_load_through_call_result_rejected/main.omg) |
| match join | [pass/facts/checked_asm_arm_load_through_match_join_exit](../../../../tests/omega/pass/facts/checked_asm_arm_load_through_match_join_exit/main.omg) | [fail/facts/checked_asm_arm_load_through_match_join_rejected](../../../../tests/omega/fail/facts/checked_asm_arm_load_through_match_join_rejected/main.omg) |
| const name | [pass/facts/checked_asm_arm_load_through_const_name_exit](../../../../tests/omega/pass/facts/checked_asm_arm_load_through_const_name_exit/main.omg) | [fail/facts/checked_asm_arm_load_through_const_name_rejected](../../../../tests/omega/fail/facts/checked_asm_arm_load_through_const_name_rejected/main.omg) |
| a nested machine | [pass/facts/checked_asm_arm_load_through_other_state_exit](../../../../tests/omega/pass/facts/checked_asm_arm_load_through_other_state_exit/main.omg) | [fail/facts/checked_asm_arm_load_through_other_state_rejected](../../../../tests/omega/fail/facts/checked_asm_arm_load_through_other_state_rejected/main.omg) |
| another machine | [pass/facts/checked_asm_arm_load_through_other_machine_exit](../../../../tests/omega/pass/facts/checked_asm_arm_load_through_other_machine_exit/main.omg) | [fail/facts/checked_asm_arm_load_through_other_machine_rejected](../../../../tests/omega/fail/facts/checked_asm_arm_load_through_other_machine_rejected/main.omg) |
| another target body | [pass/facts/checked_asm_arm_load_through_target_body_exit](../../../../tests/omega/pass/facts/checked_asm_arm_load_through_target_body_exit/main.omg) | [fail/facts/checked_asm_arm_load_through_target_body_rejected](../../../../tests/omega/fail/facts/checked_asm_arm_load_through_target_body_rejected/main.omg) |

## Requirements and Evidence

- **Establishment:** `asm { ldr loaded, source }` is a catalogued
  `MemoryTransfer` Load whose memory operand is a structured Omega place
  expression. The catalog contract, not an opaque assembler, establishes the
  access: the operand's own provenance, permission and exact-type obligations
  are the ordinary assignment's, and the instruction lowers to that checked
  place read. The local place proves the value transfer (`loaded == 37`) with
  no post-hoc caller equality guard.
- **Identity and lifetime:** the destination names the exact value version the
  place read recorded; the following Omega use reads that recorded value, and
  each other place moves the recorded value through a field, a parameter, a
  call result, a match join, a named constant index, a state parameter,
  another machine and target-scoped bodies without re-establishing it.
- **Composition:** the ten positives compose the same operation across all
  nine transport places plus the row root; the reject twins change only the
  memory-operand spelling, so a place cannot pass by weakening the fact, the
  bound or the operation.
- **Soundness boundaries:** a bracketed `[address]` operand refuses with the
  authored diagnostic on every place and target body; raw addresses grant no
  access authority, and the refusal comes from the shared instruction contract
  rather than a fixture-local recognizer.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** the checked place read
  lowers target-neutrally; both engines agree on every positive (`native exit
  0` / `interp exit: 0` for all ten), so the recorded value is not a
  compile-time fiction.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):**
  every place binds `ProgramEntry` for the four hosted targets; the
  `target_body` place carries one target-scoped body per hosted target, each
  with the same `ldr` place read.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):**
  the backend selects the target realization of the place read (`ldr` on the
  arm64 targets; the same ordinary checked assignment elsewhere) and executes
  the `macos_arm64` image natively on this host.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):**
  the emitted Mach-O image for `macos_arm64` runs to the authored exit and
  agrees with Terminal Psi (`omega run --both`), which is what the pinned
  native exit rows record.

## Regression repair (psi)

`CHECKED-ASM-LABEL-CONTRACTS` carries a current main regression: a register
loaded from a mutable input must preserve that snapshot across a conditional
label edge after the input changes. Reproduced on this host at base
`bd3c83daeef` and still red on `3e4ab71ea0d`, with a labeled body that loads
`eax` from the mutable input, rewrites the input, then reaches `jnz`:

```text
error: state `after` receives `input` as both mutable and read-only
error: state `after` receives `input` as both mutable and read-only
error: cannot prove requires contract for call after from
  main$assembly$portable$5$entry: input == 5 (needs fact
  `asm#scratch#2#eax == 5` here) in state `branch`
```

Repair: `omega-rust/psi/compilation/pipeline/03b_typed-trees-to-typed-trees/src/assembly_control/normalization.rs`.
An assembly label edge carries one argument per helper parameter: the mutable
place captures and the register versions. Since `921e9383c083` the edge also
substituted a register version's *recorded initializer* (so a `sub` premise
survives the edge for the ranked-decrease witness). When that initializer reads
a place the same edge already passes as a mutable capture, the substitution
re-reads the place after its write: the edge would hold one place both mutably
and read-only, and the register's snapshot would be lost. The repaired rule
keeps the recorded initializer whenever it is built from register versions and
constants, and substitutes the immutable register version itself when the
initializer reads a mutable capture. The ranked-backedge repair and ordinary
custody checks are unchanged: the false-arrival twin of the reproduction still
rejects with `cannot prove requires contract for call after`, and the pinned
countdown / rank-decrease / flag-snapshot checked-asm fixtures keep their
verdicts.

Fixtures for the repair:

- [pass/asm/scratch_snapshot_survives_input_write_exit](../../../../tests/omega/pass/asm/scratch_snapshot_survives_input_write_exit/main.omg)
  (checks; the arrival contract asks for the preserved snapshot `eax == 5`).
- [fail/asm/scratch_snapshot_after_input_write_wrong_value_rejected](../../../../tests/omega/fail/asm/scratch_snapshot_after_input_write_wrong_value_rejected/main.omg)
  (the same body asks for the rewritten value `eax == 7` and rejects with
  `cannot prove requires contract for call after`).

Both fixtures bind only the two hosted x86_64 targets, because the checked
scratch register/flags viewport is the x86 GPR/flags set; their native rows
belong on an x86_64 host. On this `macos_arm64` host they are checked at
`check-target:linux_x86_64`, which is the reporting their corpus rows carry.

## Run record

Producer: `omega` built from this candidate (`--build-profile` reporting
`{"schema":"omega-compiler-build-profile-v1","opt_level":"1"}`), worktree
revision in the commit that carries this row. Host: `macos_arm64`. Command per
place: `omega run --both <place>/main.omg` with stdin closed; per twin:
`omega --check <place>/invalid/main.omg`.

| Place | Positive (`--both`) | Reject twin |
| --- | --- | --- |
| root | `native exit 0` / `interp exit: 0` | — |
| local | `native exit 0` / `interp exit: 0` | rejected: bracketed `[...]` memory addressing |
| field | `native exit 0` / `interp exit: 0` | rejected: bracketed `[...]` memory addressing |
| parameter | `native exit 0` / `interp exit: 0` | rejected: bracketed `[...]` memory addressing |
| call result | `native exit 0` / `interp exit: 0` | rejected: bracketed `[...]` memory addressing |
| match join | `native exit 0` / `interp exit: 0` | rejected: bracketed `[...]` memory addressing |
| const name | `native exit 0` / `interp exit: 0` | rejected: bracketed `[...]` memory addressing |
| a nested machine | `native exit 0` / `interp exit: 0` | rejected: bracketed `[...]` memory addressing |
| another machine | `native exit 0` / `interp exit: 0` | rejected: bracketed `[...]` memory addressing |
| another target body | `native exit 0` / `interp exit: 0` | rejected: bracketed `[...]` memory addressing |

## Remaining open

Under `CHECKED-ASM-LABEL-CONTRACTS`, unchanged by this row: dynamic
multi-register clobber sets, 64-bit GPR alias bit slices, floating-point/vector
register constraints, arbitrary hardware flag dependencies, and the
ISA/register-view work that keeps an arm64 countdown and unmodeled flag
production unbuilt. The x86 scratch viewport means the scratch-register
fixtures' native rows live on x86_64 hosts; this host contributes their check
rows. `ldr` itself is catalog-admitted on the two arm64 targets only, so
binding the x86_64 hosted targets here is the four-hosted-target form of this
row's program: they are bound and checked, and their realization is the
ordinary place-read assignment rather than an arm64 memory transfer.